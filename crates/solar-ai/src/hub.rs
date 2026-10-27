//! [`AiHub`]: the one object the shell and every module use to talk to models.
//!
//! ```no_run
//! use solar_ai::{AiHub, Scope, Request};
//! use solar_core::AppKind;
//!
//! let hub = AiHub::load(solar_core::settings::config_dir());
//! // Blocking (call from a worker thread):
//! let reply = hub.scoped(Scope::App(AppKind::Word)).ask("Write a two-line toast for a retirement party").unwrap();
//! // Streaming, for the UI: poll the job every frame.
//! let mut job = hub.start(Request::new(Scope::Home).user("Hello"), None);
//! for event in job.poll() { /* JobEvent::Text(t) => append t, JobEvent::Done(result) => ... */ }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::catalog::{Catalog, ModelInfo};
use crate::config::{AiConfig, Protocol, ProviderConfig, Service};
use crate::media::{GeneratedImage, ImageRequest, TranslateRequest, Translation};
use crate::error::AiError;
use crate::http::{Cancel, Http};
use crate::providers::{self, Call};
use crate::scope::Scope;
use crate::types::{Message, ModelRef, Part, Request, Response, Role, StreamEvent, ToolCall, Usage};

/// Called from the worker thread whenever a job has news (e.g. `egui::Context::request_repaint`).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// What a running job reports.
#[derive(Debug, Clone, PartialEq)]
pub enum JobEvent {
    Text(String),
    /// The model is reasoning (shown as "Thinking…"; the text is in [`Response::reasoning`]).
    Reasoning(String),
    ToolCall(ToolCall),
    /// A rate limit, overload or network error; the request is sent again after `wait`.
    Retrying { attempt: u32, wait: Duration, reason: String },
    Done(Result<Response, AiError>),
}

/// A request running on a worker thread.
pub struct Job {
    rx: Receiver<JobEvent>,
    cancel: Cancel,
    /// The model answering, when it could be resolved.
    pub model: Option<ModelRef>,
    finished: bool,
}

impl Job {
    /// Events since the last poll (never blocks).
    pub fn poll(&mut self) -> Vec<JobEvent> {
        let events: Vec<JobEvent> = self.rx.try_iter().collect();
        if events.iter().any(|e| matches!(e, JobEvent::Done(_))) {
            self.finished = true;
        }
        events
    }

    /// Stop the request; a `Done(Err(Cancelled))` follows.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Block until the job ends.
    pub fn wait(self) -> Result<Response, AiError> {
        for e in self.rx.iter() {
            if let JobEvent::Done(r) = e {
                return r;
            }
        }
        Err(AiError::Cancelled)
    }
}

struct State {
    config: AiConfig,
    catalog: Catalog,
    dir: Option<PathBuf>,
    http: Http,
    /// Model lists fetched from servers, by provider id.
    fetched: HashMap<String, Vec<String>>,
    /// Account status from the last check ("1,234 credits ($12.34)"), by provider id.
    notes: HashMap<String, String>,
}

/// The AI layer. Cheap to clone; clones share configuration.
#[derive(Clone)]
pub struct AiHub {
    state: Arc<RwLock<State>>,
}

impl AiHub {
    /// The hub with `ai.toml` from `dir` (created on the first save).
    pub fn load(dir: PathBuf) -> AiHub {
        let config = AiConfig::load(&dir);
        AiHub::build(config, Some(dir))
    }

    /// A hub that never touches the disk (tests, previews).
    pub fn in_memory(config: AiConfig) -> AiHub {
        AiHub::build(config, None)
    }

    fn build(config: AiConfig, dir: Option<PathBuf>) -> AiHub {
        let catalog = Catalog::builtin().merged(&config.catalog);
        let http = Http::new(Duration::from_secs(config.first_byte_timeout_secs.max(5)));
        AiHub { state: Arc::new(RwLock::new(State { config, catalog, dir, http, fetched: HashMap::new(), notes: HashMap::new() })) }
    }

    pub fn config(&self) -> AiConfig {
        self.state.read().expect("ai state").config.clone()
    }

    /// Replace the configuration and save it.
    pub fn set_config(&self, config: AiConfig) -> Result<(), AiError> {
        let mut st = self.state.write().expect("ai state");
        if let Some(dir) = &st.dir {
            config.save(dir)?;
        }
        st.catalog = Catalog::builtin().merged(&config.catalog);
        if config.first_byte_timeout_secs != st.config.first_byte_timeout_secs {
            st.http = Http::new(Duration::from_secs(config.first_byte_timeout_secs.max(5)));
        }
        st.config = config;
        Ok(())
    }

    pub fn catalog(&self) -> Catalog {
        self.state.read().expect("ai state").catalog.clone()
    }

    /// The model a scope uses by default, if any provider is usable.
    pub fn model_for(&self, scope: Scope) -> Option<ModelRef> {
        let st = self.state.read().expect("ai state");
        st.config.model_for(scope, |p| first_model(&st, p))
    }

    pub fn model_info(&self, m: &ModelRef) -> ModelInfo {
        let st = self.state.read().expect("ai state");
        let kind = st.config.provider(&m.provider).map(|p| p.kind).unwrap_or(crate::config::ProviderKind::OpenAiCompatible);
        st.catalog.info(kind, &m.model)
    }

    /// Models to offer for a provider: its own list, the server's (once fetched) and the catalog's.
    pub fn models_offered(&self, provider_id: &str) -> Vec<String> {
        let st = self.state.read().expect("ai state");
        let Some(p) = st.config.provider(provider_id) else { return Vec::new() };
        let mut out: Vec<String> = p.models.clone();
        let mut push = |m: &str| {
            if !out.iter().any(|x| x == m) {
                out.push(m.to_string());
            }
        };
        for m in st.catalog.for_kind(p.kind) {
            push(&m.id);
        }
        for m in st.fetched.get(provider_id).into_iter().flatten() {
            push(m);
        }
        out
    }

    /// Ask the provider's server for its models (blocking) and remember them. For DeepL and
    /// Black Forest Labs, which have no model list, this checks the account instead (see
    /// [`AiHub::account_note`]) and returns the catalog's models.
    pub fn fetch_models(&self, provider_id: &str) -> Result<Vec<String>, AiError> {
        let (http, provider, catalog) = {
            let st = self.state.read().expect("ai state");
            let p = st.config.provider(provider_id).cloned().ok_or_else(|| AiError::UnknownProvider(provider_id.into()))?;
            let catalog: Vec<String> = st.catalog.for_kind(p.kind).iter().map(|m| m.id.clone()).collect();
            (st.http.clone(), p, catalog)
        };
        let key = check_key(&provider)?;
        let note = match provider.protocol() {
            Protocol::DeepL => Some(providers::deepl::usage(&http, &provider, key.as_deref())?),
            Protocol::Bfl => Some(providers::bfl::credits(&http, &provider, key.as_deref())?),
            _ => None,
        };
        let ids = match note {
            Some(n) => {
                self.state.write().expect("ai state").notes.insert(provider_id.into(), n);
                return Ok(catalog);
            }
            None => providers::list_models(&http, &provider, key.as_deref())?,
        };
        self.state.write().expect("ai state").fetched.insert(provider_id.into(), ids.clone());
        Ok(ids)
    }

    /// What the last check said about the account (DeepL usage, BFL credits).
    pub fn account_note(&self, provider_id: &str) -> Option<String> {
        self.state.read().expect("ai state").notes.get(provider_id).cloned()
    }

    /// The first usable provider offering `service`, or the one named.
    fn service_provider(&self, service: Service, provider_id: Option<&str>) -> Result<(Http, ProviderConfig, Option<String>), AiError> {
        let st = self.state.read().expect("ai state");
        let p = match provider_id {
            Some(id) => st.config.provider(id).cloned().ok_or_else(|| AiError::UnknownProvider(id.into()))?,
            None => st.config.providers.iter().find(|p| p.service() == service && p.usable()).or_else(|| st.config.providers.iter().find(|p| p.service() == service)).cloned().ok_or(AiError::NotConfigured)?,
        };
        if p.service() != service {
            return Err(AiError::WrongService { provider: p.display_name(), service: service_name(p.service()) });
        }
        if !p.enabled {
            return Err(AiError::ProviderDisabled { provider: p.display_name() });
        }
        let key = check_key(&p)?;
        Ok((st.http.clone(), p, key))
    }

    /// Translate with a translation service (DeepL); blocking. `provider_id: None` uses the
    /// first connected one. The request's `model` defaults to the provider's default model.
    pub fn translate(&self, provider_id: Option<&str>, req: &TranslateRequest) -> Result<Vec<Translation>, AiError> {
        let (http, p, key) = self.service_provider(Service::Translate, provider_id)?;
        let mut req = req.clone();
        if req.model.is_none() {
            req.model = p.default_model.clone().filter(|m| !m.trim().is_empty());
        }
        providers::deepl::translate(&http, &p, key.as_deref(), &req)
    }

    /// Generate an image with an image service (Black Forest Labs' FLUX); blocking, polls until
    /// the image is ready (cancel with `cancel`). `provider_id: None` uses the first connected one.
    pub fn generate_image(&self, provider_id: Option<&str>, req: &ImageRequest, cancel: &Cancel) -> Result<GeneratedImage, AiError> {
        let (http, p, key) = self.service_provider(Service::Image, provider_id)?;
        let model = req
            .model
            .clone()
            .or_else(|| p.default_model.clone())
            .filter(|m| !m.trim().is_empty())
            .or_else(|| self.catalog().for_kind(p.kind).first().map(|m| m.id.clone()))
            .ok_or_else(|| AiError::BadResponse("no image model".into()))?;
        providers::bfl::generate(&http, &p, key.as_deref(), &model, req, cancel)
    }

    /// The handle a module uses: requests carry its scope.
    pub fn scoped(&self, scope: Scope) -> ScopedAi {
        ScopedAi { hub: self.clone(), scope }
    }

    /// Run `req` on a worker thread.
    pub fn start(&self, req: Request, notify: Option<Notify>) -> Job {
        let (tx, rx) = channel();
        let cancel = Cancel::new();
        let model = req.model.clone().or_else(|| self.model_for(req.scope));
        let hub = self.clone();
        let c = cancel.clone();
        std::thread::Builder::new()
            .name("solar-ai".into())
            .spawn(move || {
                let send = |tx: &Sender<JobEvent>, e: JobEvent| {
                    let _ = tx.send(e);
                    if let Some(n) = &notify {
                        n();
                    }
                };
                let result = hub.complete(&req, &c, &mut |e| send(&tx, e));
                send(&tx, JobEvent::Done(result));
            })
            .expect("spawn ai thread");
        Job { rx, cancel, model, finished: false }
    }

    /// Run `req` on this thread, reporting progress to `on` (text, tool calls, retries).
    pub fn complete(&self, req: &Request, cancel: &Cancel, on: &mut dyn FnMut(JobEvent)) -> Result<Response, AiError> {
        let (http, provider, model, info, retries) = {
            let st = self.state.read().expect("ai state");
            let model = req.model.clone().or_else(|| st.config.model_for(req.scope, |p| first_model(&st, p))).ok_or(AiError::NotConfigured)?;
            let provider = st.config.provider(&model.provider).cloned().ok_or_else(|| AiError::UnknownProvider(model.provider.clone()))?;
            let info = st.catalog.info(provider.kind, &model.model);
            (st.http.clone(), provider, model, info, st.config.max_retries)
        };
        if !provider.enabled {
            return Err(AiError::ProviderDisabled { provider: provider.display_name() });
        }
        if provider.service() != Service::Chat {
            return Err(AiError::WrongService { provider: provider.display_name(), service: service_name(provider.service()) });
        }
        let api_key = check_key(&provider)?;
        let system = system_prompt(req, provider.protocol());
        let max_tokens = req.max_tokens.unwrap_or_else(|| info.max_output.min(8192)).max(1);
        let call = Call { provider: &provider, model: &model.model, info: &info, req, system, max_tokens, api_key };

        let mut attempt = 0;
        loop {
            if cancel.is_cancelled() {
                return Err(AiError::Cancelled);
            }
            let mut resp = Response { model: Some(model.clone()), ..Response::default() };
            let mut emitted = false;
            let result = providers::stream(&http, &call, cancel, &mut |ev| match ev {
                StreamEvent::Text(t) => {
                    emitted = true;
                    resp.text.push_str(&t);
                    on(JobEvent::Text(t));
                }
                StreamEvent::Reasoning(t) => {
                    emitted = true;
                    resp.reasoning.push_str(&t);
                    on(JobEvent::Reasoning(t));
                }
                StreamEvent::ToolCall(c) => {
                    emitted = true;
                    resp.tool_calls.push(c.clone());
                    on(JobEvent::ToolCall(c));
                }
                StreamEvent::Usage(u) => merge_usage(&mut resp.usage, u),
                StreamEvent::Stop(s) => resp.stop = s,
            });
            match result {
                Ok(()) => {
                    resp.cost_usd = info.cost(resp.usage);
                    if cancel.is_cancelled() {
                        return Err(AiError::Cancelled);
                    }
                    return Ok(resp);
                }
                Err(e) if e.is_retryable() && !emitted && attempt < retries => {
                    attempt += 1;
                    let wait = match &e {
                        AiError::RateLimited { retry_after: Some(d), .. } => (*d).min(Duration::from_secs(60)),
                        _ => Duration::from_secs(1 << attempt.min(5)),
                    };
                    log::info!("{e}; retry {attempt}/{retries} in {wait:?}");
                    on(JobEvent::Retrying { attempt, wait, reason: e.to_string() });
                    if !cancel.sleep(wait) {
                        return Err(AiError::Cancelled);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Run `req`, answering tool calls with `handler` and sending the results back, until the
    /// model stops calling tools (or `max_rounds` is reached). Returns the final reply and the
    /// whole conversation. Modules use this to let a model edit their documents through tools.
    pub fn run_tools(
        &self,
        mut req: Request,
        max_rounds: u32,
        cancel: &Cancel,
        handler: &mut dyn FnMut(&ToolCall) -> Result<String, String>,
        on: &mut dyn FnMut(JobEvent),
    ) -> Result<(Response, Vec<Message>), AiError> {
        let mut total = Usage::default();
        let mut cost: Option<f64> = Some(0.0);
        for _ in 0..max_rounds.max(1) {
            let mut resp = self.complete(&req, cancel, on)?;
            total.add(resp.usage);
            cost = cost.zip(resp.cost_usd).map(|(a, b)| a + b);
            req.messages.push(resp.to_message());
            if resp.tool_calls.is_empty() {
                resp.usage = total;
                resp.cost_usd = cost;
                return Ok((resp, req.messages));
            }
            let results: Vec<Part> = resp
                .tool_calls
                .iter()
                .map(|c| match handler(c) {
                    Ok(content) => Part::ToolResult { call_id: c.id.clone(), content, is_error: false },
                    Err(content) => Part::ToolResult { call_id: c.id.clone(), content, is_error: true },
                })
                .collect();
            req.messages.push(Message { role: Role::User, parts: results });
            // Later rounds may answer in text.
            req.tool_choice = crate::types::ToolChoice::Auto;
        }
        Err(AiError::BadResponse(format!("still calling tools after {max_rounds} rounds")))
    }
}

/// A module's view of the hub: every request carries the module's scope (instructions and
/// default model).
#[derive(Clone)]
pub struct ScopedAi {
    hub: AiHub,
    pub scope: Scope,
}

impl ScopedAi {
    pub fn hub(&self) -> &AiHub {
        &self.hub
    }

    /// A new request in this scope.
    pub fn request(&self) -> Request {
        Request::new(self.scope)
    }

    /// The model this scope uses.
    pub fn model(&self) -> Option<ModelRef> {
        self.hub.model_for(self.scope)
    }

    /// Stream a chat on a worker thread.
    pub fn chat(&self, messages: Vec<Message>, notify: Option<Notify>) -> Job {
        let mut r = self.request();
        r.messages = messages;
        self.hub.start(r, notify)
    }

    /// One prompt, one text answer (blocking).
    pub fn ask(&self, prompt: &str) -> Result<String, AiError> {
        self.hub.complete(&self.request().user(prompt), &Cancel::new(), &mut |_| {}).map(|r| r.text)
    }

    /// One prompt, a typed JSON answer matching `schema` (blocking).
    pub fn json<T: serde::de::DeserializeOwned>(&self, prompt: &str, name: &str, schema: serde_json::Value) -> Result<T, AiError> {
        self.hub.complete(&self.request().user(prompt).json(name, schema), &Cancel::new(), &mut |_| {})?.json()
    }
}

/// The system prompt sent: the scope's instructions, the request's own, and (for APIs without a
/// forced tool) a reminder to answer in JSON.
fn system_prompt(req: &Request, protocol: Protocol) -> Option<String> {
    let mut parts = vec![req.scope.system_prompt().to_string()];
    if let Some(s) = req.system.as_deref().filter(|s| !s.trim().is_empty()) {
        parts.push(s.to_string());
    }
    if let Some(j) = &req.json
        && protocol != Protocol::Anthropic
    {
        parts.push(format!("Reply with a single JSON value only, no prose or code fences, matching this JSON Schema:\n{}", j.schema));
    }
    Some(parts.join("\n\n"))
}

fn merge_usage(total: &mut Usage, u: Usage) {
    if u.input_tokens > 0 {
        total.input_tokens = u.input_tokens;
    }
    if u.output_tokens > 0 {
        total.output_tokens = u.output_tokens;
    }
}

fn first_model(st: &State, p: &ProviderConfig) -> Option<String> {
    st.catalog.for_kind(p.kind).first().map(|m| m.id.clone()).or_else(|| st.fetched.get(&p.id).and_then(|v| v.first().cloned()))
}

fn service_name(s: Service) -> &'static str {
    match s {
        Service::Chat => "chat",
        Service::Translate => "translation",
        Service::Image => "image",
    }
}

/// The provider's key, or the error explaining it's missing (or that it has no endpoint).
fn check_key(p: &ProviderConfig) -> Result<Option<String>, AiError> {
    if p.base_url().is_empty() {
        return Err(AiError::MissingEndpoint { provider: p.display_name() });
    }
    let key = p.api_key();
    if key.is_none() && p.kind.needs_key() {
        return Err(AiError::MissingKey { provider: p.display_name(), env: p.kind.key_env().first().copied().unwrap_or("an API key").into() });
    }
    Ok(key)
}
