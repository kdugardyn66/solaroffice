//! Anthropic Messages API (`POST /v1/messages`, streamed).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::{Call, Parser, Wire, s};
use crate::error::AiError;
use crate::types::{Part, Role, StopReason, StreamEvent, ToolCall, ToolChoice, Usage};

pub const VERSION: &str = "2023-06-01";
/// The tool used to get JSON output: the model is forced to call it with the answer.
pub const JSON_TOOL: &str = "json_output";

pub fn headers(api_key: Option<&str>) -> Vec<(String, String)> {
    let mut h = vec![("anthropic-version".to_string(), VERSION.to_string())];
    if let Some(k) = api_key {
        h.push(("x-api-key".into(), k.into()));
    }
    h
}

fn part(p: &Part) -> Value {
    match p {
        Part::Text { text } => json!({"type": "text", "text": text}),
        Part::Image { media_type, data } => json!({"type": "image", "source": {"type": "base64", "media_type": media_type, "data": data}}),
        Part::Document { media_type, data, name } => {
            let mut v = json!({"type": "document", "source": {"type": "base64", "media_type": media_type, "data": data}});
            if let Some(n) = name {
                v["title"] = json!(n);
            }
            v
        }
        Part::ToolUse { call } => json!({"type": "tool_use", "id": call.id, "name": call.name, "input": call.arguments}),
        Part::ToolResult { call_id, content, is_error } => json!({"type": "tool_result", "tool_use_id": call_id, "content": content, "is_error": is_error}),
        // Filtered out in `build`: other APIs' reasoning can't be replayed here.
        Part::Reasoning { .. } => Value::Null,
    }
}

pub fn build(call: &Call) -> Wire {
    let req = call.req;
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| json!({"role": if m.role == Role::User { "user" } else { "assistant" }, "content": m.parts.iter().filter(|p| !matches!(p, Part::Reasoning { .. })).map(part).collect::<Vec<_>>()}))
        .collect();
    let mut body = Map::new();
    body.insert("model".into(), json!(call.model));
    body.insert("max_tokens".into(), json!(call.max_tokens));
    body.insert("stream".into(), json!(true));
    if let Some(sys) = &call.system {
        body.insert("system".into(), json!(sys));
    }
    body.insert("messages".into(), json!(messages));
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    let mut tools: Vec<Value> = req.tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.parameters})).collect();
    let mut choice = req.tool_choice.clone();
    if let Some(j) = &req.json {
        tools.push(json!({"name": JSON_TOOL, "description": format!("Return the answer ({}) as structured data.", j.name), "input_schema": j.schema}));
        choice = ToolChoice::Tool(JSON_TOOL.into());
    }
    if !tools.is_empty() {
        body.insert("tools".into(), json!(tools));
        body.insert(
            "tool_choice".into(),
            match choice {
                ToolChoice::Auto => json!({"type": "auto"}),
                ToolChoice::Required => json!({"type": "any"}),
                ToolChoice::Tool(n) => json!({"type": "tool", "name": n}),
                ToolChoice::None => json!({"type": "none"}),
            },
        );
    }
    Wire { url: format!("{}/v1/messages", call.provider.base_url()), headers: headers(call.api_key.as_deref()), body: Value::Object(body) }
}

/// Streaming state: tool-use blocks being assembled, by block index.
#[derive(Default)]
pub struct Stream {
    tools: BTreeMap<u64, (String, String, String)>,
    usage: Usage,
}

impl Parser for Stream {
    fn feed(&mut self, event: &str, data: &str, sink: &mut dyn FnMut(StreamEvent)) -> Result<bool, AiError> {
        let v: Value = serde_json::from_str(data).map_err(|e| AiError::BadResponse(format!("bad event: {e}")))?;
        let kind = if event.is_empty() { s(&v, "type").unwrap_or("") } else { event };
        match kind {
            "message_start" => {
                if let Some(u) = v.pointer("/message/usage") {
                    self.usage.input_tokens = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                        + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                        + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0);
                    self.usage.output_tokens = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
                    sink(StreamEvent::Usage(self.usage));
                }
            }
            "content_block_start" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(b) = v.get("content_block")
                    && s(b, "type") == Some("tool_use")
                {
                    self.tools.insert(idx, (s(b, "id").unwrap_or_default().into(), s(b, "name").unwrap_or_default().into(), String::new()));
                }
            }
            "content_block_delta" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                let d = v.get("delta").cloned().unwrap_or_default();
                match s(&d, "type") {
                    Some("text_delta") => {
                        if let Some(t) = s(&d, "text").filter(|t| !t.is_empty()) {
                            sink(StreamEvent::Text(t.into()));
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(t) = self.tools.get_mut(&idx) {
                            t.2.push_str(s(&d, "partial_json").unwrap_or(""));
                        }
                    }
                    _ => {} // thinking, signatures, citations
                }
            }
            "content_block_stop" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some((id, name, args)) = self.tools.remove(&idx) {
                    let arguments = if args.trim().is_empty() { json!({}) } else { serde_json::from_str(&args).map_err(|e| AiError::BadResponse(format!("tool arguments: {e}")))? };
                    sink(StreamEvent::ToolCall(ToolCall { id, name, arguments, signature: None }));
                }
            }
            "message_delta" => {
                if let Some(n) = v.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    self.usage.output_tokens = n;
                    sink(StreamEvent::Usage(self.usage));
                }
                if let Some(r) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    sink(StreamEvent::Stop(match r {
                        "end_turn" | "stop_sequence" => StopReason::EndTurn,
                        "max_tokens" | "model_context_window_exceeded" => StopReason::MaxTokens,
                        "tool_use" => StopReason::ToolUse,
                        "refusal" => StopReason::ContentFilter,
                        other => StopReason::Other(other.into()),
                    }));
                }
            }
            "message_stop" => return Ok(false),
            "error" => {
                let t = v.pointer("/error/type").and_then(Value::as_str).unwrap_or("");
                let m = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("stream error").to_string();
                return Err(match t {
                    "overloaded_error" => AiError::Unavailable { provider: "anthropic".into(), status: 529 },
                    "rate_limit_error" => AiError::RateLimited { provider: "anthropic".into(), retry_after: None },
                    _ => AiError::BadResponse(m),
                });
            }
            _ => {} // ping
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ModelInfo;
    use crate::config::{ProviderConfig, ProviderKind};
    use crate::types::{JsonOutput, Message, Request, ToolSpec};

    fn call<'a>(p: &'a ProviderConfig, info: &'a ModelInfo, req: &'a Request) -> Call<'a> {
        Call { provider: p, model: "claude-sonnet-5-5", info, req, system: Some("Be brief.".into()), max_tokens: 1024, api_key: Some("sk".into()) }
    }

    #[test]
    fn request_body() {
        let p = ProviderConfig::new(ProviderKind::Anthropic);
        let info = ModelInfo::unknown(ProviderKind::Anthropic, "m");
        let mut req = Request::default().user("Hi");
        req.messages[0].parts.push(Part::Image { media_type: "image/png".into(), data: "AAA".into() });
        req.tools.push(ToolSpec { name: "add_row".into(), description: "Add a row".into(), parameters: json!({"type":"object"}) });
        let w = build(&call(&p, &info, &req));
        assert_eq!(w.url, "https://api.anthropic.com/v1/messages");
        assert!(w.headers.contains(&("x-api-key".into(), "sk".into())));
        assert_eq!(w.body["system"], "Be brief.");
        assert_eq!(w.body["max_tokens"], 1024);
        assert_eq!(w.body["messages"][0]["content"][1]["source"]["media_type"], "image/png");
        assert_eq!(w.body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(w.body["tool_choice"]["type"], "auto");
        assert!(w.body.get("temperature").is_none());

        let mut jr = Request::default().user("List 3 colours");
        jr.json = Some(JsonOutput { name: "colours".into(), schema: json!({"type":"object"}) });
        let w = build(&call(&p, &info, &jr));
        assert_eq!(w.body["tool_choice"], json!({"type":"tool","name": JSON_TOOL}));
        let _ = Message::user("x");
    }

    #[test]
    fn stream_parsing() {
        let events = [
            ("message_start", r#"{"type":"message_start","message":{"usage":{"input_tokens":12,"output_tokens":1}}}"#),
            ("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#),
            ("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}"#),
            ("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"lo"}}"#),
            ("content_block_start", r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_1","name":"add_row","input":{}}}"#),
            ("content_block_delta", r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"n\":"}}"#),
            ("content_block_delta", r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"3}"}}"#),
            ("content_block_stop", r#"{"type":"content_block_stop","index":1}"#),
            ("ping", r#"{"type":"ping"}"#),
            ("message_delta", r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":40}}"#),
        ];
        let mut p = Stream::default();
        let mut out = Vec::new();
        for (e, d) in events {
            assert!(p.feed(e, d, &mut |ev| out.push(ev)).unwrap());
        }
        assert!(!p.feed("message_stop", r#"{"type":"message_stop"}"#, &mut |ev| out.push(ev)).unwrap());
        let text: String = out.iter().filter_map(|e| if let StreamEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
        assert_eq!(text, "Hello");
        assert!(out.contains(&StreamEvent::ToolCall(ToolCall { id: "tu_1".into(), name: "add_row".into(), arguments: json!({"n":3}), signature: None })));
        assert!(out.contains(&StreamEvent::Usage(Usage { input_tokens: 12, output_tokens: 40 })));
        assert!(out.contains(&StreamEvent::Stop(StopReason::ToolUse)));
    }

    #[test]
    fn stream_errors() {
        let mut p = Stream::default();
        let e = p.feed("error", r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#, &mut |_| {}).unwrap_err();
        assert!(e.is_retryable());
    }
}
