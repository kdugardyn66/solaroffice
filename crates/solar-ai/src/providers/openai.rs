//! OpenAI Chat Completions (`POST {base}/chat/completions`, streamed). Also used for Ollama,
//! LM Studio and every other OpenAI-compatible server.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::{Call, Parser, Wire, data_url, s};
use crate::config::{Auth, ProviderConfig};
use crate::error::AiError;
use crate::types::{Part, Role, StopReason, StreamEvent, ToolCall, ToolChoice, Usage};

pub fn headers(api_key: Option<&str>, provider: &ProviderConfig) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = provider.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if let Some(k) = api_key {
        h.push(match provider.kind.auth() {
            Auth::Bearer => ("authorization".into(), format!("Bearer {k}")),
            Auth::Header(name) => (name.into(), k.into()),
            Auth::DeepL => ("authorization".into(), format!("DeepL-Auth-Key {k}")),
        });
    }
    h
}

fn messages(call: &Call) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(sys) = &call.system {
        out.push(json!({"role": "system", "content": sys}));
    }
    for m in &call.req.messages {
        match m.role {
            Role::User => {
                // Tool results are their own `tool` messages, before the rest of the turn.
                for p in &m.parts {
                    if let Part::ToolResult { call_id, content, .. } = p {
                        out.push(json!({"role": "tool", "tool_call_id": call_id, "content": content}));
                    }
                }
                let rest: Vec<&Part> = m.parts.iter().filter(|p| !matches!(p, Part::ToolResult { .. })).collect();
                if rest.is_empty() {
                    continue;
                }
                if rest.iter().all(|p| matches!(p, Part::Text { .. })) {
                    // Plain string content: understood by every compatible server.
                    let text: String = rest.iter().filter_map(|p| if let Part::Text { text } = p { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n");
                    out.push(json!({"role": "user", "content": text}));
                } else {
                    let content: Vec<Value> = rest
                        .iter()
                        .filter_map(|p| match p {
                            Part::Text { text } => Some(json!({"type": "text", "text": text})),
                            Part::Image { media_type, data } => Some(json!({"type": "image_url", "image_url": {"url": data_url(media_type, data)}})),
                            Part::Document { media_type, data, name } => Some(json!({"type": "file", "file": {"filename": name.clone().unwrap_or_else(|| "document.pdf".into()), "file_data": data_url(media_type, data)}})),
                            _ => None,
                        })
                        .collect();
                    out.push(json!({"role": "user", "content": content}));
                }
            }
            Role::Assistant => {
                let text = m.text();
                let reasoning: String = m.parts.iter().filter_map(|p| if let Part::Reasoning { text } = p { Some(text.as_str()) } else { None }).collect();
                let calls: Vec<Value> = m
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::ToolUse { call } => Some(json!({"id": call.id, "type": "function", "function": {"name": call.name, "arguments": call.arguments.to_string()}})),
                        _ => None,
                    })
                    .collect();
                let mut v = json!({"role": "assistant", "content": if text.is_empty() && !calls.is_empty() { Value::Null } else { json!(text) }});
                if !calls.is_empty() {
                    v["tool_calls"] = json!(calls);
                }
                if call.provider.kind.quirks().send_reasoning && !reasoning.is_empty() {
                    v["reasoning_content"] = json!(reasoning);
                }
                out.push(v);
            }
        }
    }
    out
}

pub fn build(call: &Call) -> Wire {
    let req = call.req;
    let q = call.provider.kind.quirks();
    let mut body = Map::new();
    body.insert("model".into(), json!(call.model));
    body.insert("messages".into(), json!(messages(call)));
    body.insert("stream".into(), json!(true));
    if q.stream_usage {
        body.insert("stream_options".into(), json!({"include_usage": true}));
    }
    // OpenAI's current models (and Qwen, Kimi, Foundry) take max_completion_tokens; most
    // compatible servers still expect max_tokens.
    body.insert(if q.max_completion_tokens { "max_completion_tokens" } else { "max_tokens" }.into(), json!(call.max_tokens));
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    if !req.tools.is_empty() {
        let tools: Vec<Value> = req.tools.iter().map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}})).collect();
        body.insert("tools".into(), json!(tools));
        body.insert(
            "tool_choice".into(),
            match &req.tool_choice {
                ToolChoice::None => json!("none"),
                ToolChoice::Required if q.forced_tools => json!("required"),
                ToolChoice::Tool(n) if q.forced_tools => json!({"type": "function", "function": {"name": n}}),
                // Qwen and GLM only take "auto"; the prompt asks for the tool instead.
                _ => json!("auto"),
            },
        );
    }
    if let Some(j) = &req.json {
        // Where json_schema isn't supported, json_object plus the schema in the system prompt.
        let format = if q.json_schema { json!({"type": "json_schema", "json_schema": {"name": j.name, "schema": j.schema, "strict": false}}) } else { json!({"type": "json_object"}) };
        body.insert("response_format".into(), format);
    }
    Wire { url: format!("{}/chat/completions", call.provider.base_url()), headers: headers(call.api_key.as_deref(), call.provider), body: Value::Object(body) }
}

#[derive(Default)]
pub struct Stream {
    /// Tool calls being assembled, by index: (id, name, arguments).
    tools: BTreeMap<u64, (String, String, String)>,
}

impl Stream {
    fn flush_tools(&mut self, sink: &mut dyn FnMut(StreamEvent)) -> Result<(), AiError> {
        for (i, (id, name, args)) in std::mem::take(&mut self.tools) {
            let arguments = if args.trim().is_empty() { json!({}) } else { serde_json::from_str(&args).map_err(|e| AiError::BadResponse(format!("tool arguments: {e}")))? };
            let id = if id.is_empty() { format!("call_{i}") } else { id };
            sink(StreamEvent::ToolCall(ToolCall { id, name, arguments, signature: None }));
        }
        Ok(())
    }
}

impl Parser for Stream {
    fn feed(&mut self, _event: &str, data: &str, sink: &mut dyn FnMut(StreamEvent)) -> Result<bool, AiError> {
        if data.trim() == "[DONE]" {
            self.flush_tools(sink)?;
            return Ok(false);
        }
        let v: Value = serde_json::from_str(data).map_err(|e| AiError::BadResponse(format!("bad chunk: {e}")))?;
        if let Some(e) = v.get("error") {
            return Err(AiError::BadResponse(s(e, "message").or(e.as_str()).unwrap_or("stream error").into()));
        }
        if let Some(choice) = v.pointer("/choices/0") {
            // DeepSeek, Kimi, GLM and Qwen stream their thinking first; LM Studio and others say `reasoning`.
            if let Some(t) = choice.pointer("/delta/reasoning_content").or_else(|| choice.pointer("/delta/reasoning")).and_then(Value::as_str).filter(|t| !t.is_empty()) {
                sink(StreamEvent::Reasoning(t.into()));
            }
            if let Some(t) = choice.pointer("/delta/content").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                sink(StreamEvent::Text(t.into()));
            }
            if let Some(calls) = choice.pointer("/delta/tool_calls").and_then(Value::as_array) {
                for (pos, c) in calls.iter().enumerate() {
                    let idx = c.get("index").and_then(Value::as_u64).unwrap_or(pos as u64);
                    let e = self.tools.entry(idx).or_default();
                    if let Some(id) = s(c, "id") {
                        e.0 = id.into();
                    }
                    if let Some(n) = c.pointer("/function/name").and_then(Value::as_str) {
                        e.1.push_str(n);
                    }
                    if let Some(a) = c.pointer("/function/arguments") {
                        match a {
                            Value::String(a) => e.2.push_str(a),
                            other => e.2.push_str(&other.to_string()), // some servers send an object
                        }
                    }
                }
            }
            if let Some(r) = choice.get("finish_reason").and_then(Value::as_str) {
                self.flush_tools(sink)?;
                sink(StreamEvent::Stop(match r {
                    "stop" => StopReason::EndTurn,
                    "length" => StopReason::MaxTokens,
                    "tool_calls" | "function_call" => StopReason::ToolUse,
                    "content_filter" => StopReason::ContentFilter,
                    other => StopReason::Other(other.into()),
                }));
            }
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            sink(StreamEvent::Usage(Usage { input_tokens: n("prompt_tokens"), output_tokens: n("completion_tokens") }));
        }
        Ok(true)
    }

    fn finish(&mut self, sink: &mut dyn FnMut(StreamEvent)) {
        // Servers that end the stream without [DONE] or a finish_reason.
        let _ = self.flush_tools(sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ModelInfo;
    use crate::config::ProviderKind;
    use crate::types::{JsonOutput, Message, Request, ToolSpec};

    fn wire(kind: ProviderKind, req: &Request) -> Wire {
        let p = ProviderConfig::new(kind);
        let info = ModelInfo::unknown(kind, "m");
        let key = kind.needs_key().then(|| "sk".to_string());
        build(&Call { provider: &p, model: "m", info: &info, req, system: Some("sys".into()), max_tokens: 256, api_key: key })
    }

    #[test]
    fn request_bodies() {
        let mut req = Request::default().user("Hi");
        req.messages.push(Message { role: Role::Assistant, parts: vec![Part::ToolUse { call: ToolCall { id: "c1".into(), name: "f".into(), arguments: json!({"x":1}), signature: None } }] });
        req.messages.push(Message { role: Role::User, parts: vec![Part::ToolResult { call_id: "c1".into(), content: "ok".into(), is_error: false }, Part::Image { media_type: "image/png".into(), data: "AA".into() }] });
        req.tools.push(ToolSpec { name: "f".into(), description: "F".into(), parameters: json!({"type":"object"}) });
        req.json = Some(JsonOutput { name: "out".into(), schema: json!({"type":"object"}) });
        let w = wire(ProviderKind::OpenAi, &req);
        assert_eq!(w.url, "https://api.openai.com/v1/chat/completions");
        assert!(w.headers.contains(&("authorization".into(), "Bearer sk".into())));
        let m = &w.body["messages"];
        assert_eq!(m[0], json!({"role":"system","content":"sys"}));
        assert_eq!(m[1], json!({"role":"user","content":"Hi"}));
        assert_eq!(m[2]["tool_calls"][0]["function"]["arguments"], "{\"x\":1}");
        assert_eq!(m[2]["content"], Value::Null);
        assert_eq!(m[3], json!({"role":"tool","tool_call_id":"c1","content":"ok"}));
        assert_eq!(m[4]["content"][0]["image_url"]["url"], "data:image/png;base64,AA");
        assert_eq!(w.body["max_completion_tokens"], 256);
        assert_eq!(w.body["response_format"]["type"], "json_schema");

        let w = wire(ProviderKind::Ollama, &Request::default().user("Hi"));
        assert_eq!(w.url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(w.body["max_tokens"], 256);
        assert!(w.headers.is_empty(), "no key, no auth header");
        let w = wire(ProviderKind::LmStudio, &Request::default().user("Hi"));
        assert_eq!(w.url, "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn provider_quirks() {
        let mut req = Request::default().user("Hi");
        req.messages.push(Message { role: Role::Assistant, parts: vec![Part::Reasoning { text: "think".into() }, Part::text("Hello")] });
        req.messages.push(Message::user("More"));
        req.tools.push(ToolSpec { name: "f".into(), description: "F".into(), parameters: json!({"type":"object"}) });
        req.tool_choice = ToolChoice::Tool("f".into());
        req.json = Some(JsonOutput { name: "out".into(), schema: json!({"type":"object"}) });

        let w = wire(ProviderKind::Moonshot, &req);
        assert_eq!(w.url, "https://api.moonshot.ai/v1/chat/completions");
        assert_eq!(w.body["messages"][2]["reasoning_content"], "think", "Kimi needs its reasoning back");
        assert_eq!(w.body["max_completion_tokens"], 256);

        let w = wire(ProviderKind::Zai, &req);
        assert_eq!(w.body["tool_choice"], "auto");
        assert_eq!(w.body["response_format"], json!({"type":"json_object"}));
        assert_eq!(w.body["max_tokens"], 256);

        let w = wire(ProviderKind::Mistral, &req);
        assert!(w.body["messages"][2].get("reasoning_content").is_none());
        assert_eq!(w.body["tool_choice"]["function"]["name"], "f");
        assert_eq!(w.body["response_format"]["type"], "json_schema");

        let w = wire(ProviderKind::Cohere, &req);
        assert!(w.body.get("stream_options").is_none());

        let mut p = ProviderConfig::new(ProviderKind::Microsoft);
        p.base_url = Some("https://res.openai.azure.com/openai/v1".into());
        assert_eq!(headers(Some("k"), &p), [("api-key".to_string(), "k".to_string())]);
    }

    #[test]
    fn reasoning_streams() {
        let mut p = Stream::default();
        let mut out = Vec::new();
        p.feed("", r#"{"choices":[{"delta":{"reasoning_content":"Let me think"}}]}"#, &mut |e| out.push(e)).unwrap();
        p.feed("", r#"{"choices":[{"delta":{"content":"Yes"}}]}"#, &mut |e| out.push(e)).unwrap();
        assert_eq!(out, [StreamEvent::Reasoning("Let me think".into()), StreamEvent::Text("Yes".into())]);
    }

    #[test]
    fn stream_parsing() {
        let mut p = Stream::default();
        let mut out = Vec::new();
        let chunks = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"}}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"lo"}}]}"#,
            r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"f","arguments":"{\"x\""}}]}}]}"#,
            r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":":2}"}}]}}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":9,"completion_tokens":4,"total_tokens":13}}"#,
        ];
        for c in chunks {
            assert!(p.feed("", c, &mut |e| out.push(e)).unwrap());
        }
        assert!(!p.feed("", "[DONE]", &mut |e| out.push(e)).unwrap());
        assert_eq!(out[0], StreamEvent::Text("Hel".into()));
        assert_eq!(out[2], StreamEvent::ToolCall(ToolCall { id: "call_a".into(), name: "f".into(), arguments: json!({"x":2}), signature: None }));
        assert_eq!(out[3], StreamEvent::Stop(StopReason::ToolUse));
        assert_eq!(out[4], StreamEvent::Usage(Usage { input_tokens: 9, output_tokens: 4 }));
    }

    #[test]
    fn ollama_style_whole_tool_call_without_id() {
        let mut p = Stream::default();
        let mut out = Vec::new();
        p.feed("", r#"{"choices":[{"delta":{"tool_calls":[{"function":{"name":"f","arguments":{"x":1}}}]}}]}"#, &mut |e| out.push(e)).unwrap();
        p.finish(&mut |e| out.push(e));
        assert_eq!(out, [StreamEvent::ToolCall(ToolCall { id: "call_0".into(), name: "f".into(), arguments: json!({"x":1}), signature: None })]);
    }
}
