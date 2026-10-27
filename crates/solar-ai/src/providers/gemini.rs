//! Gemini API (`POST /v1beta/models/{model}:streamGenerateContent?alt=sse`).

use serde_json::{Map, Value, json};

use super::{Call, Parser, Wire, s};
use crate::error::AiError;
use crate::types::{Part, Request, Role, StopReason, StreamEvent, ToolCall, ToolChoice, Usage};

pub fn headers(api_key: Option<&str>) -> Vec<(String, String)> {
    api_key.map(|k| vec![("x-goog-api-key".to_string(), k.to_string())]).unwrap_or_default()
}

fn part(p: &Part, req: &Request) -> Value {
    match p {
        Part::Text { text } => json!({"text": text}),
        Part::Image { media_type, data } | Part::Document { media_type, data, .. } => json!({"inlineData": {"mimeType": media_type, "data": data}}),
        Part::ToolUse { call } => {
            let mut v = json!({"functionCall": {"name": call.name, "args": call.arguments}});
            if call.id != call.name {
                v["functionCall"]["id"] = json!(call.id);
            }
            if let Some(sig) = &call.signature {
                v["thoughtSignature"] = json!(sig);
            }
            v
        }
        Part::ToolResult { call_id, content, is_error } => {
            let name = req.tool_name_for(call_id).unwrap_or(call_id);
            let response = match serde_json::from_str::<Value>(content) {
                Ok(v @ Value::Object(_)) => v,
                _ if *is_error => json!({"error": content}),
                _ => json!({"content": content}),
            };
            let mut v = json!({"functionResponse": {"name": name, "response": response}});
            if call_id != name {
                v["functionResponse"]["id"] = json!(call_id);
            }
            v
        }
        // Filtered out in `build`.
        Part::Reasoning { .. } => Value::Null,
    }
}

pub fn build(call: &Call) -> Wire {
    let req = call.req;
    let contents: Vec<Value> = req
        .messages
        .iter()
        .map(|m| json!({"role": if m.role == Role::User { "user" } else { "model" }, "parts": m.parts.iter().filter(|p| !matches!(p, Part::Reasoning { .. })).map(|p| part(p, req)).collect::<Vec<_>>()}))
        .collect();
    let mut body = Map::new();
    body.insert("contents".into(), json!(contents));
    if let Some(sys) = &call.system {
        body.insert("systemInstruction".into(), json!({"parts": [{"text": sys}]}));
    }
    let mut gen_cfg = Map::new();
    gen_cfg.insert("maxOutputTokens".into(), json!(call.max_tokens));
    if let Some(t) = req.temperature {
        gen_cfg.insert("temperature".into(), json!(t));
    }
    if let Some(j) = &req.json {
        gen_cfg.insert("responseMimeType".into(), json!("application/json"));
        gen_cfg.insert("responseJsonSchema".into(), j.schema.clone());
    }
    body.insert("generationConfig".into(), Value::Object(gen_cfg));
    if !req.tools.is_empty() {
        let decls: Vec<Value> = req.tools.iter().map(|t| json!({"name": t.name, "description": t.description, "parametersJsonSchema": t.parameters})).collect();
        body.insert("tools".into(), json!([{"functionDeclarations": decls}]));
        let cfg = match &req.tool_choice {
            ToolChoice::Auto => json!({"mode": "AUTO"}),
            ToolChoice::Required => json!({"mode": "ANY"}),
            ToolChoice::Tool(n) => json!({"mode": "ANY", "allowedFunctionNames": [n]}),
            ToolChoice::None => json!({"mode": "NONE"}),
        };
        body.insert("toolConfig".into(), json!({"functionCallingConfig": cfg}));
    }
    let model = call.model.strip_prefix("models/").unwrap_or(call.model);
    Wire {
        url: format!("{}/v1beta/models/{}:streamGenerateContent?alt=sse", call.provider.base_url(), model),
        headers: headers(call.api_key.as_deref()),
        body: Value::Object(body),
    }
}

/// Model ids from `GET /v1beta/models`, keeping those that can generate content.
pub fn parse_models(v: &Value) -> Vec<String> {
    v.get("models")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|m| m.get("supportedGenerationMethods").and_then(Value::as_array).is_none_or(|g| g.iter().any(|x| x == "generateContent")))
                .filter_map(|m| s(m, "name").map(|n| n.strip_prefix("models/").unwrap_or(n).to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
pub struct Stream {
    saw_tool: bool,
    calls: u32,
}

impl Parser for Stream {
    fn feed(&mut self, _event: &str, data: &str, sink: &mut dyn FnMut(StreamEvent)) -> Result<bool, AiError> {
        let v: Value = serde_json::from_str(data).map_err(|e| AiError::BadResponse(format!("bad chunk: {e}")))?;
        if let Some(e) = v.get("error") {
            return Err(AiError::BadResponse(s(e, "message").unwrap_or("stream error").into()));
        }
        if let Some(reason) = v.pointer("/promptFeedback/blockReason").and_then(Value::as_str) {
            log::warn!("gemini blocked the prompt: {reason}");
            sink(StreamEvent::Stop(StopReason::ContentFilter));
            return Ok(false);
        }
        if let Some(parts) = v.pointer("/candidates/0/content/parts").and_then(Value::as_array) {
            for p in parts {
                if p.get("thought").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if let Some(t) = s(p, "text").filter(|t| !t.is_empty()) {
                    sink(StreamEvent::Text(t.into()));
                }
                if let Some(fc) = p.get("functionCall") {
                    self.saw_tool = true;
                    self.calls += 1;
                    let name = s(fc, "name").unwrap_or_default().to_string();
                    let id = s(fc, "id").map(str::to_string).unwrap_or_else(|| name.clone());
                    sink(StreamEvent::ToolCall(ToolCall {
                        id,
                        name,
                        arguments: fc.get("args").cloned().unwrap_or_else(|| json!({})),
                        signature: s(p, "thoughtSignature").map(str::to_string),
                    }));
                }
            }
        }
        if let Some(u) = v.get("usageMetadata") {
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            sink(StreamEvent::Usage(Usage { input_tokens: n("promptTokenCount"), output_tokens: n("candidatesTokenCount") + n("thoughtsTokenCount") }));
        }
        if let Some(r) = v.pointer("/candidates/0/finishReason").and_then(Value::as_str) {
            sink(StreamEvent::Stop(match r {
                "STOP" if self.saw_tool => StopReason::ToolUse,
                "STOP" => StopReason::EndTurn,
                "MAX_TOKENS" => StopReason::MaxTokens,
                "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY" => StopReason::ContentFilter,
                other => StopReason::Other(other.into()),
            }));
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ModelInfo;
    use crate::config::{ProviderConfig, ProviderKind};
    use crate::types::{JsonOutput, Message, ToolSpec};

    #[test]
    fn request_body() {
        let p = ProviderConfig::new(ProviderKind::Gemini);
        let info = ModelInfo::unknown(ProviderKind::Gemini, "m");
        let mut req = Request::default().user("Make a chart");
        req.messages.push(Message { role: Role::Assistant, parts: vec![Part::ToolUse { call: ToolCall { id: "add".into(), name: "add".into(), arguments: json!({"a":1}), signature: Some("sig".into()) } }] });
        req.messages.push(Message { role: Role::User, parts: vec![Part::ToolResult { call_id: "add".into(), content: "3".into(), is_error: false }] });
        req.tools.push(ToolSpec { name: "add".into(), description: "Add".into(), parameters: json!({"type":"object"}) });
        req.tool_choice = ToolChoice::Tool("add".into());
        req.json = Some(JsonOutput { name: "x".into(), schema: json!({"type":"object"}) });
        let c = Call { provider: &p, model: "models/gemini-3.8-flash", info: &info, req: &req, system: Some("sys".into()), max_tokens: 500, api_key: Some("k".into()) };
        let w = build(&c);
        assert_eq!(w.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse");
        assert_eq!(w.headers, [("x-goog-api-key".to_string(), "k".to_string())]);
        assert_eq!(w.body["systemInstruction"]["parts"][0]["text"], "sys");
        assert_eq!(w.body["contents"][1]["role"], "model");
        assert_eq!(w.body["contents"][1]["parts"][0]["thoughtSignature"], "sig");
        assert_eq!(w.body["contents"][2]["parts"][0]["functionResponse"], json!({"name":"add","response":{"content":"3"}}));
        assert_eq!(w.body["generationConfig"]["maxOutputTokens"], 500);
        assert_eq!(w.body["generationConfig"]["responseMimeType"], "application/json");
        assert_eq!(w.body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0], "add");
    }

    #[test]
    fn stream_parsing() {
        let mut p = Stream::default();
        let mut out = Vec::new();
        let chunks = [
            r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Thinking","thought":true},{"text":"Hi "}]}}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"add","args":{"a":2}},"thoughtSignature":"s1"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":7,"candidatesTokenCount":5,"thoughtsTokenCount":3}}"#,
        ];
        for c in chunks {
            assert!(p.feed("", c, &mut |e| out.push(e)).unwrap());
        }
        assert_eq!(out[0], StreamEvent::Text("Hi ".into()));
        assert_eq!(out[1], StreamEvent::ToolCall(ToolCall { id: "add".into(), name: "add".into(), arguments: json!({"a":2}), signature: Some("s1".into()) }));
        assert!(out.contains(&StreamEvent::Usage(Usage { input_tokens: 7, output_tokens: 8 })));
        assert!(out.contains(&StreamEvent::Stop(StopReason::ToolUse)));
        let err = Stream::default().feed("", r#"{"error":{"code":400,"message":"API key not valid"}}"#, &mut |_| {}).unwrap_err();
        assert_eq!(err, AiError::BadResponse("API key not valid".into()));
    }

    #[test]
    fn model_list() {
        let v = json!({"models":[
            {"name":"models/gemini-3.8-flash","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/text-embedding-5","supportedGenerationMethods":["embedContent"]}]});
        assert_eq!(parse_models(&v), ["gemini-3.8-flash"]);
    }
}
