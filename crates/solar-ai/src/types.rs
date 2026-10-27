//! The provider-neutral request and response types every module talks in.

use std::fmt;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scope::Scope;

/// Who said a message. System instructions go in [`Request::system`], not in a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A tool call the model asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The provider's call id (Gemini has none for older models: the tool name is used).
    pub id: String,
    pub name: String,
    /// The arguments, as JSON.
    pub arguments: Value,
    /// Gemini's thought signature, which must be sent back with the call in later turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// One piece of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    /// An image (PNG, JPEG, GIF, WebP), base64-encoded.
    Image { media_type: String, data: String },
    /// A document (PDF), base64-encoded.
    Document { media_type: String, data: String, name: Option<String> },
    /// The assistant called a tool (keep these in the history when sending tool results back).
    ToolUse { call: ToolCall },
    /// The result of a tool call, sent back in a user message.
    ToolResult { call_id: String, content: String, is_error: bool },
    /// The model's visible reasoning (`reasoning_content`), kept with its turn: some APIs
    /// (Kimi K3, GLM) need it sent back; the others ignore it.
    Reasoning { text: String },
}

impl Part {
    pub fn text(t: impl Into<String>) -> Part {
        Part::Text { text: t.into() }
    }

    /// An image from raw bytes; the media type is sniffed from the bytes.
    pub fn image(bytes: &[u8]) -> Option<Part> {
        let media_type = sniff_image(bytes)?;
        Some(Part::Image { media_type: media_type.into(), data: base64::engine::general_purpose::STANDARD.encode(bytes) })
    }

    /// A PDF from raw bytes.
    pub fn pdf(bytes: &[u8], name: Option<String>) -> Part {
        Part::Document { media_type: "application/pdf".into(), data: base64::engine::general_purpose::STANDARD.encode(bytes), name }
    }
}

/// The media type of an image from its first bytes.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// A chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Message {
        Message { role: Role::User, parts: vec![Part::text(text)] }
    }

    pub fn assistant(text: impl Into<String>) -> Message {
        Message { role: Role::Assistant, parts: vec![Part::text(text)] }
    }

    /// The text parts joined.
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|p| match p {
                Part::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// A tool the model may call. `parameters` is a JSON Schema object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum ToolChoice {
    /// The model decides.
    #[default]
    Auto,
    /// The model must call some tool.
    Required,
    /// The model must call this tool.
    Tool(String),
    /// No tool calls.
    None,
}

/// Ask for a JSON reply matching `schema` (a JSON Schema object).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonOutput {
    /// A short identifier for the schema (letters, digits, `_`).
    pub name: String,
    pub schema: Value,
}

/// `provider:model`, e.g. `anthropic:claude-sonnet-5-5` or `ollama:llama3.2:3b`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModelRef {
    /// The configured provider's id (see `ProviderConfig::id`).
    pub provider: String,
    pub model: String,
}

impl ModelRef {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> ModelRef {
        ModelRef { provider: provider.into(), model: model.into() }
    }

    /// Parse `provider:model`; the model may itself contain `:` (Ollama tags).
    pub fn parse(s: &str) -> Option<ModelRef> {
        let (p, m) = s.trim().split_once(':')?;
        (!p.is_empty() && !m.is_empty()).then(|| ModelRef::new(p, m))
    }
}

impl fmt::Display for ModelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.model)
    }
}

impl TryFrom<String> for ModelRef {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        ModelRef::parse(&s).ok_or_else(|| format!("'{s}' is not provider:model"))
    }
}

impl From<ModelRef> for String {
    fn from(m: ModelRef) -> String {
        m.to_string()
    }
}

/// A request to any model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Request {
    /// The model; `None` uses the scope's default (see `AiConfig::model_for`).
    pub model: Option<ModelRef>,
    /// Who is asking: the home screen or a module. Picks the default model and adds the
    /// module's instructions in front of `system`.
    pub scope: Scope,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub tool_choice: ToolChoice,
    /// Defaults to the model's limit, capped at 8192.
    pub max_tokens: Option<u32>,
    /// Sent only when set (some reasoning models reject it).
    pub temperature: Option<f32>,
    pub json: Option<JsonOutput>,
}

impl Request {
    pub fn new(scope: Scope) -> Request {
        Request { scope, ..Request::default() }
    }

    pub fn user(mut self, text: impl Into<String>) -> Request {
        self.messages.push(Message::user(text));
        self
    }

    pub fn system(mut self, text: impl Into<String>) -> Request {
        self.system = Some(text.into());
        self
    }

    pub fn model(mut self, m: ModelRef) -> Request {
        self.model = Some(m);
        self
    }

    pub fn json(mut self, name: &str, schema: Value) -> Request {
        self.json = Some(JsonOutput { name: name.into(), schema });
        self
    }

    pub fn tool(mut self, t: ToolSpec) -> Request {
        self.tools.push(t);
        self
    }

    /// The tool name a [`Part::ToolResult`] answers, from the matching [`Part::ToolUse`] earlier
    /// in the conversation (Gemini needs names, not ids).
    pub fn tool_name_for(&self, call_id: &str) -> Option<&str> {
        self.messages.iter().flat_map(|m| &m.parts).find_map(|p| match p {
            Part::ToolUse { call } if call.id == call_id => Some(call.name.as_str()),
            _ => None,
        })
    }
}

/// Token counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
    }
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StopReason {
    #[default]
    EndTurn,
    MaxTokens,
    ToolUse,
    ContentFilter,
    Other(String),
}

/// What a provider emits while streaming.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Text(String),
    /// Reasoning text, before the answer.
    Reasoning(String),
    ToolCall(ToolCall),
    /// Counts so far; later events replace earlier ones field by field when non-zero.
    Usage(Usage),
    Stop(StopReason),
}

/// A finished reply.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Response {
    pub model: Option<ModelRef>,
    pub text: String,
    /// The model's reasoning, when the API streams it (`reasoning_content`).
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    pub stop: StopReason,
    /// Estimated cost in US dollars, when the model's price is known.
    pub cost_usd: Option<f64>,
}

impl Response {
    /// The reply as JSON: a forced tool call's arguments, else the text (tolerating code fences
    /// and prose around the JSON).
    pub fn json_value(&self) -> Option<Value> {
        if let Some(c) = self.tool_calls.first() {
            return Some(c.arguments.clone());
        }
        extract_json(&self.text)
    }

    /// The JSON reply parsed into `T`.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, crate::AiError> {
        let v = self.json_value().ok_or_else(|| crate::AiError::BadResponse("the reply contains no JSON".into()))?;
        serde_json::from_value(v).map_err(|e| crate::AiError::BadResponse(format!("the JSON reply doesn't match: {e}")))
    }

    /// The assistant message to append to the history (text and tool calls).
    pub fn to_message(&self) -> Message {
        let mut parts = Vec::new();
        if !self.reasoning.is_empty() {
            parts.push(Part::Reasoning { text: self.reasoning.clone() });
        }
        if !self.text.is_empty() {
            parts.push(Part::text(self.text.clone()));
        }
        parts.extend(self.tool_calls.iter().cloned().map(|call| Part::ToolUse { call }));
        Message { role: Role::Assistant, parts }
    }
}

/// The first JSON object or array in `text` (bare, or inside a ```json fence).
pub fn extract_json(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str(t) {
        return Some(v);
    }
    if let Some(start) = t.find("```") {
        let rest = &t[start + 3..];
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        if let Some(end) = rest.find("```")
            && let Ok(v) = serde_json::from_str(rest[..end].trim())
        {
            return Some(v);
        }
    }
    // The widest balanced {...} or [...] span that parses.
    for (open, close) in [('{', '}'), ('[', ']')] {
        if let (Some(a), Some(b)) = (t.find(open), t.rfind(close))
            && a < b
            && let Ok(v) = serde_json::from_str(&t[a..=b])
        {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_refs() {
        let m = ModelRef::parse("ollama:llama3.2:3b").unwrap();
        assert_eq!((m.provider.as_str(), m.model.as_str()), ("ollama", "llama3.2:3b"));
        assert_eq!(m.to_string(), "ollama:llama3.2:3b");
        assert!(ModelRef::parse("nomodel").is_none());
        assert!(ModelRef::parse(":x").is_none());
        let s: ModelRef = serde_json::from_value(json!("anthropic:claude-sonnet-5-5")).unwrap();
        assert_eq!(s.provider, "anthropic");
    }

    #[test]
    fn json_extraction() {
        assert_eq!(extract_json(r#"{"a":1}"#), Some(json!({"a":1})));
        assert_eq!(extract_json("Sure!\n```json\n{\"a\": [1,2]}\n```\nDone."), Some(json!({"a":[1,2]})));
        assert_eq!(extract_json("Here: {\"ok\": true} — enjoy"), Some(json!({"ok": true})));
        assert_eq!(extract_json("[1, 2]"), Some(json!([1, 2])));
        assert_eq!(extract_json("no json here"), None);
    }

    #[test]
    fn image_sniffing() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_image(b"hello"), None);
        assert!(Part::image(b"hello").is_none());
    }

    #[test]
    fn tool_names_for_results() {
        let mut r = Request::default();
        r.messages.push(Message {
            role: Role::Assistant,
            parts: vec![Part::ToolUse { call: ToolCall { id: "c1".into(), name: "insert_row".into(), arguments: json!({}), signature: None } }],
        });
        assert_eq!(r.tool_name_for("c1"), Some("insert_row"));
        assert_eq!(r.tool_name_for("zz"), None);
    }
}
