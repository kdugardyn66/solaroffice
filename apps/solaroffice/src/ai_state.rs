//! The state behind Settings › AI providers / AI models: the edited copy of the configuration
//! (saved on Done), the screen shown inside the section, connection tests and the model form.
//! No UI code, so it is unit-tested.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use solar_ai::{AiConfig, AiError, AiHub, Category, KeySource, ModelInfo, ModelRef, ProviderConfig, ProviderKind};

/// The sections of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    /// AI providers: the connected services and local servers.
    Providers,
    /// AI models: which model each part of SolarOffice uses, and models you added.
    Models,
}

/// The screen shown inside the AI sections.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AiScreen {
    /// The list (providers, or models).
    #[default]
    List,
    /// Editing the provider with this id.
    Edit(String),
    /// Adding a provider, step 1: choose the kind.
    AddKind,
    /// Adding a provider, step 2: its details.
    AddForm(Box<ProviderConfig>),
    /// Adding a model by hand.
    AddModel(Box<ModelForm>),
}

/// The "Add a model" form.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelForm {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub context: String,
    pub max_output: String,
    pub input: String,
    pub output: String,
    pub vision: bool,
    pub pdf: bool,
    pub tools: bool,
    /// What it's good at (at most five).
    pub categories: Vec<Category>,
    pub error: Option<String>,
}

/// What a connection test found: the models, and an account note (DeepL usage, BFL credits).
pub type TestResult = Result<(Vec<String>, Option<String>), AiError>;

/// A "Test connection" in progress or done, per provider id.
pub enum TestState {
    Running(Receiver<TestResult>),
    Ok(Vec<String>),
    Err(String),
}

/// Edits in the AI sections, applied on Done.
#[derive(Default)]
pub struct AiSettingsUi {
    /// The configuration being edited; `None` until an AI section is opened.
    pub draft: Option<AiConfig>,
    pub tests: HashMap<String, TestState>,
    /// Account notes from finished tests, by provider id.
    pub notes: HashMap<String, String>,
    pub error: Option<String>,
    pub screen: AiScreen,
}

impl AiSettingsUi {
    /// The draft, started from the saved configuration when there is none yet.
    pub fn draft_mut(&mut self, hub: &AiHub) -> &mut AiConfig {
        self.draft.get_or_insert_with(|| hub.config())
    }

    pub fn any_running(&self) -> bool {
        self.tests.values().any(|t| matches!(t, TestState::Running(_)))
    }

    /// Take in finished connection tests; returns whether anything changed.
    pub fn poll_tests(&mut self) -> bool {
        let mut changed = false;
        for (id, t) in self.tests.iter_mut() {
            if let TestState::Running(rx) = t
                && let Ok(r) = rx.try_recv()
            {
                changed = true;
                *t = match r {
                    Ok((list, note)) => {
                        match note {
                            Some(n) => self.notes.insert(id.clone(), n),
                            None => self.notes.remove(id),
                        };
                        TestState::Ok(list)
                    }
                    Err(e) => TestState::Err(e.to_string()),
                };
            }
        }
        changed
    }

    /// Start a connection test for `p` (with the settings as edited, not as saved).
    pub fn start_test(&mut self, p: &ProviderConfig) {
        let draft = self.draft.clone().unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        let probe = AiHub::in_memory(AiConfig { providers: vec![p.clone()], ..draft });
        let id = p.id.clone();
        std::thread::spawn(move || {
            let r = probe.fetch_models(&id).map(|list| (list, probe.account_note(&id)));
            let _ = tx.send(r);
        });
        self.tests.insert(p.id.clone(), TestState::Running(rx));
    }
}

/// The models to offer for `p`: its own list, the catalog's, a test's result and the hub's
/// fetched list.
pub fn offered(hub: &AiHub, tests: &HashMap<String, TestState>, p: &ProviderConfig) -> Vec<String> {
    let mut out = p.models.clone();
    let mut push = |m: &str| {
        if !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    };
    for m in hub.catalog().for_kind(p.kind) {
        push(&m.id);
    }
    if let Some(TestState::Ok(list)) = tests.get(&p.id) {
        for m in list {
            push(m);
        }
    }
    for m in hub.models_offered(&p.id) {
        push(&m);
    }
    out
}

/// The model a provider uses when none is picked: its first listed model, else the first offered.
pub fn provider_fallback(hub: &AiHub, tests: &HashMap<String, TestState>, p: &ProviderConfig) -> Option<String> {
    p.models.first().cloned().or_else(|| offered(hub, tests, p).into_iter().next())
}

/// Providers in region order (Europe, Asia-Pacific, North America, local & custom), keeping
/// their order within a region.
pub fn by_region(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    let mut v: Vec<&ProviderConfig> = providers.iter().collect();
    v.sort_by_key(|p| p.region());
    v
}

/// "Claude Sonnet 5.5 · Anthropic · $$$".
pub fn model_summary(hub: &AiHub, config: &AiConfig, m: &ModelRef) -> String {
    let info = hub.model_info(m);
    let provider = config.provider(&m.provider).map(|p| p.display_name()).unwrap_or_default();
    match info.price_tier() {
        "" if info.input == Some(0.0) => format!("{} · {provider} · free", info.name),
        "" => format!("{} · {provider}", info.name),
        t => format!("{} · {provider} · {t}", info.name),
    }
}

/// "Ready", "Needs an API key", "Key from $ANTHROPIC_API_KEY", "Local", "Off" and its tone.
pub fn status(p: &ProviderConfig) -> (String, Tone) {
    if !p.enabled {
        return ("Off".into(), Tone::Muted);
    }
    if p.base_url().is_empty() {
        return ("Needs an endpoint".into(), Tone::Bad);
    }
    match p.key_source() {
        KeySource::None if p.kind.needs_key() => ("Needs an API key".into(), Tone::Bad),
        KeySource::Env(v) => (format!("Key from ${v}"), Tone::Good),
        KeySource::Stored => ("Ready".into(), Tone::Good),
        KeySource::None if p.kind.is_local() => ("Local · no key needed".into(), Tone::Good),
        KeySource::None => ("Ready".into(), Tone::Good),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
    Muted,
}

/// The catalog entry for the "Add a model" form, or what's wrong with it.
pub fn model_from_form(f: &ModelForm, kind: Option<ProviderKind>) -> Result<ModelInfo, String> {
    let kind = kind.ok_or("Choose a provider.")?;
    let id = f.id.trim();
    if id.is_empty() {
        return Err("Enter the model id the provider uses.".into());
    }
    let num = |s: &str, what: &str| -> Result<Option<f64>, String> {
        let s = s.trim().trim_start_matches('$').replace(',', "");
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<f64>().map(Some).ok().filter(|v| v.is_some_and(|v| v >= 0.0)).ok_or(format!("{what} must be a number, like 2 or 0.75."))
    };
    let mut info = ModelInfo::unknown(kind, id);
    if !f.name.trim().is_empty() {
        info.name = f.name.trim().to_string();
    }
    if let Some(v) = num(&f.input, "The input price")? {
        info.input = Some(v);
    }
    if let Some(v) = num(&f.output, "The output price")? {
        info.output = Some(v);
    }
    if info.input.is_some() != info.output.is_some() {
        return Err("Enter both prices, or neither.".into());
    }
    if let Some(v) = num(&f.context, "The context window")? {
        info.context = v as u64;
    }
    if let Some(v) = num(&f.max_output, "The max output")? {
        info.max_output = v as u32;
    }
    info.vision = f.vision;
    info.pdf = f.pdf;
    info.tools = f.tools;
    if f.categories.len() > Category::MAX_PER_MODEL {
        return Err(format!("Choose at most {} categories.", Category::MAX_PER_MODEL));
    }
    info.categories = f.categories.clone();
    Ok(info)
}

/// Remove provider `id` from `draft`, with the models that pointed at it.
pub fn remove_provider(draft: &mut AiConfig, id: &str) {
    draft.providers.retain(|p| p.id != id);
    draft.models.retain(|_, m| m.provider != id);
    if draft.default_model.as_ref().is_some_and(|m| m.provider == id) {
        draft.default_model = None;
    }
}

/// Add the model from a validated form to `draft`.
pub fn add_model(draft: &mut AiConfig, provider: &str, info: ModelInfo) {
    if let Some(p) = draft.provider_mut(provider)
        && !p.models.contains(&info.id)
    {
        p.models.push(info.id.clone());
    }
    draft.catalog.retain(|c| !(c.provider == info.provider && c.id == info.id));
    draft.catalog.push(info);
}

/// Remove an added model from `draft`.
pub fn remove_model(draft: &mut AiConfig, provider: &str, model: &str) {
    let kind = draft.provider(provider).map(|p| p.kind);
    if let Some(p) = draft.provider_mut(provider) {
        p.models.retain(|x| x != model);
        if p.default_model.as_deref() == Some(model) {
            p.default_model = None;
        }
    }
    draft.catalog.retain(|c| !(Some(c.provider) == kind && c.id == model));
}

/// The two-letter flag emoji for a country code ("NL" → 🇳🇱), or empty.
pub fn flag_emoji(code: &str) -> String {
    let code = code.trim();
    if code.len() != 2 || !code.is_ascii() {
        return String::new();
    }
    code.to_ascii_uppercase().chars().filter_map(|c| char::from_u32(0x1F1E6 + (c as u32 - 'A' as u32))).collect()
}

/// A provider kind's badge: colour and initials.
pub fn kind_badge(kind: ProviderKind) -> (&'static str, &'static str) {
    match kind {
        ProviderKind::AlephAlpha => ("#1F2937", "AA"),
        ProviderKind::BlackForestLabs => ("#111111", "BF"),
        ProviderKind::DeepL => ("#0F2B46", "DL"),
        ProviderKind::Mistral => ("#FA520F", "M"),
        ProviderKind::Alibaba => ("#FF6A00", "Qw"),
        ProviderKind::DeepSeek => ("#4D6BFE", "DS"),
        ProviderKind::Moonshot => ("#16162A", "K"),
        ProviderKind::Sakana => ("#C81E3A", "S"),
        ProviderKind::Zai => ("#2D3A8C", "Z"),
        ProviderKind::Anthropic => ("#D97757", "A"),
        ProviderKind::Cohere => ("#39594D", "Co"),
        ProviderKind::Gemini => ("#4285F4", "G"),
        ProviderKind::Meta => ("#0467DF", "M"),
        ProviderKind::Microsoft => ("#0078D4", "MS"),
        ProviderKind::OpenAi => ("#10A37F", "O"),
        ProviderKind::SpaceXAi => ("#000000", "X"),
        ProviderKind::LmStudio => ("#6B46C1", "LM"),
        ProviderKind::Ollama => ("#4B5563", "Ol"),
        ProviderKind::OpenAiCompatible => ("#64748B", "</>"),
        ProviderKind::Custom => ("#64748B", "+"),
    }
}

/// The card title on the Add screen: the company, or "Custom provider".
pub fn kind_title(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Custom => "Custom provider",
        k => k.vendor(),
    }
}

/// "Translation" / "Images" for the non-chat services.
pub fn service_tag(s: solar_ai::Service) -> Option<&'static str> {
    match s {
        solar_ai::Service::Chat => None,
        solar_ai::Service::Translate => Some("Translation"),
        solar_ai::Service::Image => Some("Images"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags() {
        assert_eq!(flag_emoji("NL"), "🇳🇱");
        assert_eq!(flag_emoji("us"), "🇺🇸");
        assert_eq!(flag_emoji(""), "");
        assert_eq!(flag_emoji("USA"), "");
    }

    #[test]
    fn model_form_validation() {
        let kind = Some(ProviderKind::OpenAi);
        let mut f = ModelForm { provider: "openai".into(), tools: true, ..ModelForm::default() };
        assert!(model_from_form(&f, kind).is_err(), "id is required");
        f.id = "gpt-x".into();
        f.input = "$2".into();
        assert!(model_from_form(&f, kind).unwrap_err().contains("both prices"));
        f.output = "10".into();
        f.context = "128,000".into();
        let info = model_from_form(&f, kind).unwrap();
        assert_eq!((info.input, info.output, info.context), (Some(2.0), Some(10.0), 128_000));
        assert!(info.tools);
        f.output = "abc".into();
        assert!(model_from_form(&f, kind).unwrap_err().contains("must be a number"));
        f.output = "10".into();
        f.categories = vec![Category::Chat; 6];
        assert!(model_from_form(&f, kind).is_err());
        assert!(model_from_form(&f, None).is_err());
    }

    #[test]
    fn add_and_remove_models_and_providers() {
        let mut draft = AiConfig::default();
        let kind = ProviderKind::Ollama;
        let info = ModelInfo::unknown(kind, "llama-x");
        add_model(&mut draft, "ollama", info);
        assert!(draft.provider("ollama").unwrap().models.contains(&"llama-x".to_string()));
        assert_eq!(draft.catalog.len(), 1);
        draft.models.insert("word".into(), ModelRef::new("ollama", "llama-x"));
        draft.default_model = Some(ModelRef::new("ollama", "llama-x"));
        remove_model(&mut draft, "ollama", "llama-x");
        assert!(draft.catalog.is_empty());
        assert!(draft.provider("ollama").unwrap().models.is_empty());
        remove_provider(&mut draft, "ollama");
        assert!(draft.provider("ollama").is_none());
        assert!(draft.models.is_empty() && draft.default_model.is_none());
    }

    #[test]
    fn status_of_providers() {
        let mut p = ProviderConfig::new(ProviderKind::Anthropic);
        p.api_key = Some("k".into());
        assert_eq!(status(&p).1, Tone::Good);
        p.enabled = false;
        assert_eq!(status(&p), ("Off".into(), Tone::Muted));
        let local = ProviderConfig::new(ProviderKind::Ollama);
        assert_eq!(status(&local).1, Tone::Good);
        let custom = ProviderConfig::new(ProviderKind::Custom);
        assert_eq!(status(&custom).1, Tone::Bad, "a custom provider needs an endpoint");
    }

    #[test]
    fn regions_sort_stably() {
        let c = AiConfig::default();
        let order: Vec<_> = by_region(&c.providers).iter().map(|p| p.region()).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn tests_get_polled() {
        let mut ui = AiSettingsUi::default();
        let (tx, rx) = std::sync::mpsc::channel();
        ui.tests.insert("a".into(), TestState::Running(rx));
        assert!(ui.any_running() && !ui.poll_tests());
        tx.send(Ok((vec!["m1".into()], Some("12 credits".into())))).unwrap();
        assert!(ui.poll_tests());
        assert!(matches!(ui.tests.get("a"), Some(TestState::Ok(l)) if l == &["m1".to_string()]));
        assert_eq!(ui.notes.get("a").map(String::as_str), Some("12 credits"));
        assert!(!ui.any_running());
    }
}
