//! The wire protocols. The three chat protocols each turn a [`Call`] into an HTTP request and
//! parse the streamed reply into [`StreamEvent`]s; [`stream`] and [`list_models`] do the HTTP.
//! [`deepl`] (translation) and [`bfl`] (images) are plain request/response APIs.

pub mod anthropic;
pub mod bfl;
pub mod deepl;
pub mod gemini;
pub mod openai;

use serde_json::Value;

use crate::catalog::ModelInfo;
use crate::config::{Protocol, ProviderConfig};
use crate::error::AiError;
use crate::http::{Cancel, Http, network_error, read_sse, retry_after};
use crate::types::{Request, StreamEvent};

/// Everything a protocol needs to build one request.
pub struct Call<'a> {
    pub provider: &'a ProviderConfig,
    pub model: &'a str,
    pub info: &'a ModelInfo,
    pub req: &'a Request,
    /// The scope's instructions plus the request's own system prompt.
    pub system: Option<String>,
    pub max_tokens: u32,
    pub api_key: Option<String>,
}

/// A built HTTP request.
pub struct Wire {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// A streaming parser: fed each SSE event, it emits events and says whether to keep reading.
pub trait Parser {
    fn feed(&mut self, event: &str, data: &str, sink: &mut dyn FnMut(StreamEvent)) -> Result<bool, AiError>;
    /// Called at the end of the stream (flush anything pending).
    fn finish(&mut self, _sink: &mut dyn FnMut(StreamEvent)) {}
}

pub fn build(call: &Call) -> (Wire, Box<dyn Parser>) {
    match call.provider.protocol() {
        Protocol::Anthropic => (anthropic::build(call), Box::new(anthropic::Stream::default())),
        Protocol::Gemini => (gemini::build(call), Box::new(gemini::Stream::default())),
        // The hub never sends chats to DeepL or BFL; those fall through to the OpenAI shape.
        Protocol::OpenAiChat | Protocol::DeepL | Protocol::Bfl => (openai::build(call), Box::new(openai::Stream::default())),
    }
}

/// Send `call` and stream the reply into `sink`. Returns `Ok(true)` if any output was emitted
/// (callers must not retry after that).
pub fn stream(http: &Http, call: &Call, cancel: &Cancel, sink: &mut dyn FnMut(StreamEvent)) -> Result<(), AiError> {
    let (wire, mut parser) = build(call);
    let pid = &call.provider.id;
    let mut rb = http.agent_for(&wire.url).post(&wire.url).header("content-type", "application/json").header("accept", "text/event-stream");
    for (k, v) in &wire.headers {
        rb = rb.header(k.as_str(), v.as_str());
    }
    let body = serde_json::to_vec(&wire.body).map_err(|e| AiError::BadResponse(e.to_string()))?;
    log::debug!("POST {} ({} bytes) model {}", wire.url, body.len(), call.model);
    let resp = rb.send(&body[..]).map_err(|e| network_error(pid, &wire.url, e))?;
    if cancel.is_cancelled() {
        return Err(AiError::Cancelled);
    }
    let status = resp.status().as_u16();
    if status >= 400 {
        let ra = retry_after(&resp);
        let text = resp.into_body().read_to_string().unwrap_or_default();
        return Err(AiError::from_status(pid, call.model, status, &text, ra));
    }
    let reader = resp.into_body().into_reader();
    read_sse(reader, cancel, |event, data| parser.feed(event, data, sink))?;
    parser.finish(sink);
    Ok(())
}

/// The models a provider's server offers.
pub fn list_models(http: &Http, provider: &ProviderConfig, api_key: Option<&str>) -> Result<Vec<String>, AiError> {
    let (url, headers): (String, Vec<(String, String)>) = match provider.protocol() {
        Protocol::Anthropic => (format!("{}/v1/models?limit=1000", provider.base_url()), anthropic::headers(api_key)),
        Protocol::Gemini => (format!("{}/v1beta/models?pageSize=1000", provider.base_url()), gemini::headers(api_key)),
        Protocol::OpenAiChat => (format!("{}/models", provider.base_url()), openai::headers(api_key, provider)),
        Protocol::DeepL | Protocol::Bfl => return Err(AiError::BadResponse("this service has no model list".into())),
    };
    let mut rb = http.agent_for(&url).get(&url);
    for (k, v) in &headers {
        rb = rb.header(k.as_str(), v.as_str());
    }
    let resp = rb.call().map_err(|e| network_error(&provider.id, &url, e))?;
    let status = resp.status().as_u16();
    let text = resp.into_body().read_to_string().map_err(|e| AiError::BadResponse(e.to_string()))?;
    if status >= 400 {
        return Err(AiError::from_status(&provider.id, "", status, &text, None));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| AiError::BadResponse(format!("model list: {e}")))?;
    let mut ids = match provider.protocol() {
        Protocol::Gemini => gemini::parse_models(&v),
        _ => v.get("data").and_then(|d| d.as_array()).map(|a| a.iter().filter_map(|m| m.get("id")?.as_str().map(str::to_string)).collect()).unwrap_or_default(),
    };
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Send a JSON request (`body: None` for a GET) and read a JSON reply.
pub(crate) fn json_call(http: &Http, provider: &ProviderConfig, url: &str, headers: &[(String, String)], body: Option<&Value>) -> Result<Value, AiError> {
    let agent = http.agent_for(url);
    let resp = match body {
        Some(b) => {
            let mut rb = agent.post(url).header("content-type", "application/json").header("accept", "application/json");
            for (k, v) in headers {
                rb = rb.header(k.as_str(), v.as_str());
            }
            rb.send(&serde_json::to_vec(b).map_err(|e| AiError::BadResponse(e.to_string()))?[..])
        }
        None => {
            let mut rb = agent.get(url).header("accept", "application/json");
            for (k, v) in headers {
                rb = rb.header(k.as_str(), v.as_str());
            }
            rb.call()
        }
    }
    .map_err(|e| network_error(&provider.id, url, e))?;
    let status = resp.status().as_u16();
    let ra = retry_after(&resp);
    let text = resp.into_body().read_to_string().map_err(|e| AiError::BadResponse(e.to_string()))?;
    if status >= 400 {
        return Err(AiError::from_status(&provider.id, "", status, &text, ra));
    }
    serde_json::from_str(&text).map_err(|e| AiError::BadResponse(format!("{url}: {e}")))
}

/// `12,345`.
pub(crate) fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A string field of a JSON object.
pub(crate) fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// `data:` URL for inline content.
pub(crate) fn data_url(media_type: &str, data: &str) -> String {
    format!("data:{media_type};base64,{data}")
}
