#![allow(clippy::unwrap_used, clippy::expect_used)] // integration tests
//! End-to-end tests against a scripted local HTTP server speaking each provider's protocol.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use solar_ai::{AiConfig, AiError, AiHub, Cancel, ImageRequest, JobEvent, ModelRef, ProviderConfig, ProviderKind, Request, Scope, StopReason, ToolSpec, TranslateRequest};
use solar_core::AppKind;

#[derive(Clone)]
struct Reply {
    status: u16,
    content_type: &'static str,
    chunks: Vec<String>,
    delay: Duration,
}

impl Reply {
    fn sse(events: &[&str]) -> Reply {
        Reply { status: 200, content_type: "text/event-stream", chunks: events.iter().map(|e| format!("{e}\n\n")).collect(), delay: Duration::ZERO }
    }
    fn json(status: u16, body: Value) -> Reply {
        Reply { status, content_type: "application/json", chunks: vec![body.to_string()], delay: Duration::ZERO }
    }
}

#[derive(Debug, Clone)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

struct Mock {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

fn mock(replies: Vec<Reply>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::from(replies)));
    let s2 = seen.clone();
    let own_url = url.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let path = request_line.split_whitespace().nth(1).unwrap_or("").to_string();
            let mut headers = Vec::new();
            let mut len = 0usize;
            loop {
                let mut l = String::new();
                reader.read_line(&mut l).unwrap();
                let l = l.trim_end().to_string();
                if l.is_empty() {
                    break;
                }
                if let Some((k, v)) = l.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            s2.lock().unwrap().push(Seen { path, headers, body: serde_json::from_slice(&body).unwrap_or(Value::Null) });
            let reply = queue.lock().unwrap().pop_front().unwrap_or_else(|| Reply::json(500, json!({"error": {"message": "no scripted reply"}})));
            let _ = write!(stream, "HTTP/1.1 {} X\r\nContent-Type: {}\r\nConnection: close\r\n\r\n", reply.status, reply.content_type);
            for c in reply.chunks {
                std::thread::sleep(reply.delay);
                // Replies can point back at the server ("{URL}/result").
                let c = c.replace("{URL}", &own_url);
                if stream.write_all(c.as_bytes()).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
        }
    });
    Mock { url, seen }
}

fn hub_with(kind: ProviderKind, base_url: String, key: Option<&str>) -> AiHub {
    let mut p = ProviderConfig::new(kind);
    p.base_url = Some(base_url);
    p.api_key = key.map(str::to_string);
    let config = AiConfig { providers: vec![p], max_retries: 2, ..AiConfig::default() };
    AiHub::in_memory(config)
}

#[test]
fn lm_studio_streams_text_with_usage() {
    let m = mock(vec![Reply::sse(&[
        r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#,
        r#"data: {"choices":[{"delta":{"content":", Kris"},"finish_reason":"stop"}]}"#,
        r#"data: {"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":3}}"#,
        "data: [DONE]",
    ])]);
    let hub = hub_with(ProviderKind::LmStudio, format!("{}/v1", m.url), None);
    let req = Request::new(Scope::App(AppKind::Word)).user("Say hello").model(ModelRef::new("lmstudio", "qwen3-8b"));
    let mut texts = Vec::new();
    let r = hub.complete(&req, &Cancel::new(), &mut |e| if let JobEvent::Text(t) = e { texts.push(t) }).unwrap();
    assert_eq!(r.text, "Hello, Kris");
    assert_eq!(texts, ["Hello", ", Kris"]);
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (20, 3));
    assert_eq!(r.cost_usd, Some(0.0), "local models are free");
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/chat/completions");
    assert_eq!(seen[0].body["model"], "qwen3-8b");
    let sys = seen[0].body["messages"][0]["content"].as_str().unwrap();
    assert!(sys.starts_with("You are SolarWord's writing assistant"), "the module's instructions come first");
    assert!(seen[0].header("authorization").is_none());
}

#[test]
fn anthropic_tool_loop() {
    let m = mock(vec![
        Reply::sse(&[
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":30,\"output_tokens\":1}}}",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu1\",\"name\":\"sum\",\"input\":{}}}",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"a\\\":2,\\\"b\\\":3}\"}}",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":15}}",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}",
        ]),
        Reply::sse(&[
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":60,\"output_tokens\":1}}}",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"The sum is 5.\"}}",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":6}}",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}",
        ]),
    ]);
    let hub = hub_with(ProviderKind::Anthropic, m.url.clone(), Some("sk-ant-test"));
    let req = Request::new(Scope::App(AppKind::Grid))
        .user("What is 2 + 3?")
        .model(ModelRef::new("anthropic", "claude-sonnet-5-5"))
        .tool(ToolSpec { name: "sum".into(), description: "Add two numbers".into(), parameters: json!({"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}}}) });
    let mut calls = Vec::new();
    let (resp, history) = hub
        .run_tools(req, 4, &Cancel::new(), &mut |c| {
            calls.push(c.clone());
            Ok((c.arguments["a"].as_f64().unwrap() + c.arguments["b"].as_f64().unwrap()).to_string())
        }, &mut |_| {})
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(resp.text, "The sum is 5.");
    assert_eq!(resp.stop, StopReason::EndTurn);
    assert_eq!((resp.usage.input_tokens, resp.usage.output_tokens), (90, 21), "usage adds up over rounds");
    // Priced as Claude Sonnet 5.5 ($2 / $10 per million tokens).
    assert!((resp.cost_usd.unwrap() - (90.0 * 2.0 + 21.0 * 10.0) / 1e6).abs() < 1e-12);
    assert_eq!(history.len(), 4, "user, tool call, tool result, answer");
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/messages");
    assert_eq!(seen[0].header("x-api-key"), Some("sk-ant-test"));
    assert_eq!(seen[0].header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(seen[1].body["messages"][2]["content"][0], json!({"type":"tool_result","tool_use_id":"tu1","content":"5","is_error":false}));
}

#[test]
fn gemini_structured_output() {
    let m = mock(vec![Reply::sse(&[
        r#"data: {"candidates":[{"content":{"parts":[{"text":"{\"title\":\"Sunrise\",\"slides\":[{\"title\":\"Why\","}]}}]}"#,
        r#"data: {"candidates":[{"content":{"parts":[{"text":"\"bullets\":[\"Queues\"]}]}"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":40,"candidatesTokenCount":20}}"#,
    ])]);
    let hub = hub_with(ProviderKind::Gemini, m.url.clone(), Some("g-key"));
    let task = solar_ai::tasks::task("deck.outline").unwrap();
    let req = task.request("A café opening").model(ModelRef::new("gemini", "gemini-3.8-flash"));
    let resp = hub.complete(&req, &Cancel::new(), &mut |_| {}).unwrap();
    let outline: solar_ai::schemas::DeckOutline = resp.json().unwrap();
    assert_eq!(outline.title, "Sunrise");
    assert_eq!(outline.slides[0].bullets, ["Queues"]);
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse");
    assert_eq!(seen[0].header("x-goog-api-key"), Some("g-key"));
    assert_eq!(seen[0].body["generationConfig"]["responseMimeType"], "application/json");
}

#[test]
fn retries_overloads_then_succeeds() {
    let m = mock(vec![
        Reply::json(503, json!({"error": {"message": "busy"}})),
        Reply::sse(&[r#"data: {"choices":[{"delta":{"content":"ok"},"finish_reason":"stop"}]}"#, "data: [DONE]"]),
    ]);
    let hub = hub_with(ProviderKind::Ollama, format!("{}/v1", m.url), None);
    let mut retries = Vec::new();
    let r = hub
        .complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("ollama", "llama3.2:3b")), &Cancel::new(), &mut |e| {
            if let JobEvent::Retrying { attempt, .. } = e {
                retries.push(attempt)
            }
        })
        .unwrap();
    assert_eq!(r.text, "ok");
    assert_eq!(retries, [1]);
}

#[test]
fn auth_errors_are_not_retried_and_keys_are_required() {
    let m = mock(vec![Reply::json(401, json!({"error": {"message": "Incorrect API key provided"}}))]);
    let hub = hub_with(ProviderKind::OpenAi, format!("{}/v1", m.url), Some("bad"));
    let err = hub.complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("openai", "gpt-6.1-sol")), &Cancel::new(), &mut |_| {}).unwrap_err();
    assert!(matches!(&err, AiError::Auth { message, .. } if message == "Incorrect API key provided"), "{err:?}");
    assert_eq!(m.seen.lock().unwrap().len(), 1);
    assert_eq!(m.seen.lock().unwrap()[0].body["max_completion_tokens"], 8192);

    let hub = hub_with(ProviderKind::Anthropic, m.url.clone(), None);
    // A developer's own ANTHROPIC_API_KEY would satisfy the check, so skip it then.
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        let err = hub.complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("anthropic", "x")), &Cancel::new(), &mut |_| {}).unwrap_err();
        assert!(matches!(err, AiError::MissingKey { .. }));
    }
    let err = hub.complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("nope", "x")), &Cancel::new(), &mut |_| {}).unwrap_err();
    assert_eq!(err, AiError::UnknownProvider("nope".into()));
}

#[test]
fn unreachable_local_server_says_so() {
    // A port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let hub = AiHub::in_memory(AiConfig { max_retries: 0, providers: vec![{
        let mut p = ProviderConfig::new(ProviderKind::Ollama);
        p.base_url = Some(format!("http://127.0.0.1:{port}/v1"));
        p
    }], ..AiConfig::default() });
    let err = hub.complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("ollama", "m")), &Cancel::new(), &mut |_| {}).unwrap_err();
    assert!(matches!(&err, AiError::Network { message, .. } if message.contains("is the server running")), "{err}");
}

#[test]
fn jobs_stream_and_cancel() {
    let mut slow = Reply::sse(&[
        r#"data: {"choices":[{"delta":{"content":"one "}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"two "}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"three "}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"four"},"finish_reason":"stop"}]}"#,
        "data: [DONE]",
    ]);
    slow.delay = Duration::from_millis(150);
    let m = mock(vec![slow]);
    let hub = hub_with(ProviderKind::LmStudio, format!("{}/v1", m.url), None);
    let notified = Arc::new(Mutex::new(0));
    let n2 = notified.clone();
    let mut job = hub.start(Request::new(Scope::Home).user("count").model(ModelRef::new("lmstudio", "m")), Some(Arc::new(move || *n2.lock().unwrap() += 1)));
    assert_eq!(job.model, Some(ModelRef::new("lmstudio", "m")));
    // Wait for the first word, then stop.
    let mut text = String::new();
    for _ in 0..100 {
        for e in job.poll() {
            if let JobEvent::Text(t) = e {
                text.push_str(&t);
            }
        }
        if !text.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(text.starts_with("one"));
    job.cancel();
    let result = job.wait();
    assert_eq!(result, Err(AiError::Cancelled));
    assert!(*notified.lock().unwrap() >= 2, "the UI was woken for the text and the end");
}

#[test]
fn model_lists() {
    let m = mock(vec![
        Reply::json(200, json!({"object":"list","data":[{"id":"qwen3-8b"},{"id":"llama-3.3-70b"}]})),
        Reply::json(200, json!({"models":[{"name":"models/gemini-3.8-flash","supportedGenerationMethods":["generateContent"]}]})),
    ]);
    let hub = AiHub::in_memory(AiConfig {
        providers: vec![
            {
                let mut p = ProviderConfig::new(ProviderKind::LmStudio);
                p.base_url = Some(format!("{}/v1", m.url));
                p
            },
            {
                let mut p = ProviderConfig::new(ProviderKind::Gemini);
                p.base_url = Some(m.url.clone());
                p.api_key = Some("k".into());
                p
            },
        ],
        ..AiConfig::default()
    });
    assert_eq!(hub.fetch_models("lmstudio").unwrap(), ["llama-3.3-70b", "qwen3-8b"]);
    assert_eq!(hub.models_offered("lmstudio"), ["llama-3.3-70b", "qwen3-8b"]);
    // With no model chosen, the first usable provider's first model answers.
    assert_eq!(hub.model_for(Scope::Home), Some(ModelRef::new("lmstudio", "llama-3.3-70b")));
    assert_eq!(hub.fetch_models("gemini").unwrap(), ["gemini-3.8-flash"]);
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/models");
    assert_eq!(seen[1].path, "/v1beta/models?pageSize=1000");
}

#[test]
fn deepl_translates_and_reports_usage() {
    let m = mock(vec![
        Reply::json(200, json!({"translations":[{"detected_source_language":"EN","text":"Goedemorgen","billed_characters":12}]})),
        Reply::json(200, json!({"character_count": 12345, "character_limit": 500000})),
        Reply::json(456, json!({"message": "Quota exceeded"})),
    ]);
    let hub = hub_with(ProviderKind::DeepL, m.url.clone(), Some("dl-key:fx"));
    let mut req = TranslateRequest::new("Good morning", "nl");
    req.formality = Some("more".into());
    let t = hub.translate(None, &req).unwrap();
    assert_eq!(t[0].text, "Goedemorgen");
    assert_eq!(t[0].detected_source_language.as_deref(), Some("EN"));
    assert_eq!(hub.fetch_models("deepl").unwrap(), ["quality_optimized", "latency_optimized"]);
    assert_eq!(hub.account_note("deepl").as_deref(), Some("12,345 of 500,000 characters used"));
    let err = hub.translate(Some("deepl"), &req).unwrap_err();
    assert!(matches!(&err, AiError::Http { status: 456, message, .. } if message == "Quota exceeded"), "{err:?}");
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v2/translate");
    assert_eq!(seen[0].header("authorization"), Some("DeepL-Auth-Key dl-key:fx"));
    assert_eq!(seen[0].body["target_lang"], "NL");
    assert_eq!(seen[0].body["formality"], "more");
    assert_eq!(seen[1].path, "/v2/usage");
    // A translation service never answers a chat.
    let err = hub.complete(&Request::new(Scope::Home).user("hi").model(ModelRef::new("deepl", "quality_optimized")), &Cancel::new(), &mut |_| {}).unwrap_err();
    assert!(matches!(err, AiError::WrongService { service: "translation", .. }), "{err:?}");
}

#[test]
fn flux_generates_an_image() {
    let m = mock(vec![
        Reply::json(200, json!({"id": "task-1", "polling_url": "{URL}/v1/get_result?id=task-1", "cost": 3.0})),
        Reply::json(200, json!({"id": "task-1", "status": "Pending"})),
        Reply::json(200, json!({"id": "task-1", "status": "Ready", "result": {"sample": "{URL}/sample.png"}})),
        Reply { status: 200, content_type: "image/png", chunks: vec!["not really a png".into()], delay: Duration::ZERO },
        Reply::json(200, json!({"credits": 1234.0})),
    ]);
    let hub = hub_with(ProviderKind::BlackForestLabs, m.url.clone(), Some("bfl-key"));
    let req = ImageRequest { prompt: "A sunrise over a café".into(), width: Some(1024), height: Some(768), ..ImageRequest::default() };
    let img = hub.generate_image(None, &req, &Cancel::new()).unwrap();
    assert_eq!(img.bytes, b"not really a png");
    assert_eq!(img.media_type, "image/png");
    assert_eq!(img.model, "flux-2-pro", "the provider's recommended model");
    assert_eq!(img.cost_usd, Some(0.03));
    assert_eq!(hub.fetch_models("bfl").unwrap()[0], "flux-2-pro");
    assert_eq!(hub.account_note("bfl").as_deref(), Some("1,234 credits ($12.34)"));
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/flux-2-pro");
    assert_eq!(seen[0].header("x-key"), Some("bfl-key"));
    assert_eq!(seen[0].body["width"], 1024);
    assert_eq!(seen[1].path, "/v1/get_result?id=task-1");
    assert_eq!(seen[1].header("x-key"), Some("bfl-key"));
    assert_eq!(seen[3].path, "/sample.png");
    assert!(seen[3].header("x-key").is_none(), "the signed URL gets no key");
    assert_eq!(seen[4].path, "/v1/credits");
}

#[test]
fn moderated_images_say_so() {
    let m = mock(vec![
        Reply::json(200, json!({"id": "t", "polling_url": "{URL}/v1/get_result?id=t"})),
        Reply::json(200, json!({"id": "t", "status": "Content Moderated"})),
    ]);
    let hub = hub_with(ProviderKind::BlackForestLabs, m.url.clone(), Some("k"));
    let err = hub.generate_image(Some("bfl"), &ImageRequest { prompt: "x".into(), ..ImageRequest::default() }, &Cancel::new()).unwrap_err();
    assert!(err.to_string().contains("content moderation"), "{err}");
}

#[test]
fn kimi_reasoning_is_kept_and_sent_back() {
    let m = mock(vec![
        Reply::sse(&[
            r#"data: {"choices":[{"delta":{"reasoning_content":"The user greets me."}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"Hello!"},"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ]),
        Reply::sse(&[r#"data: {"choices":[{"delta":{"content":"Fine."},"finish_reason":"stop"}]}"#, "data: [DONE]"]),
    ]);
    let hub = hub_with(ProviderKind::Moonshot, format!("{}/v1", m.url), Some("mk"));
    let model = ModelRef::new("moonshot", "kimi-k3.1-pro");
    let mut thinking = String::new();
    let mut req = Request::new(Scope::Home).user("Hi").model(model.clone());
    let r = hub.complete(&req, &Cancel::new(), &mut |e| if let JobEvent::Reasoning(t) = e { thinking.push_str(&t) }).unwrap();
    assert_eq!((r.text.as_str(), r.reasoning.as_str(), thinking.as_str()), ("Hello!", "The user greets me.", "The user greets me."));
    assert!(r.cost_usd.is_some(), "priced from the catalog");
    req.messages.push(r.to_message());
    req = req.user("How are you?");
    hub.complete(&req, &Cancel::new(), &mut |_| {}).unwrap();
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].header("authorization"), Some("Bearer mk"));
    assert!(seen[0].body.get("max_completion_tokens").is_some());
    assert_eq!(seen[1].body["messages"][2], json!({"role":"assistant","content":"Hello!","reasoning_content":"The user greets me."}));
}

#[test]
fn custom_provider_speaks_anthropic() {
    let m = mock(vec![Reply::sse(&[
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hoi\"}}",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}",
    ])]);
    let mut p = ProviderConfig::new(ProviderKind::Custom);
    p.id = "eu-llm".into();
    p.base_url = Some(m.url.clone());
    p.protocol = Some(solar_ai::Protocol::Anthropic);
    p.api_key = Some("ck".into());
    let hub = AiHub::in_memory(AiConfig { providers: vec![p], ..AiConfig::default() });
    let r = hub.complete(&Request::new(Scope::Home).user("Hi").model(ModelRef::new("eu-llm", "big")), &Cancel::new(), &mut |_| {}).unwrap();
    assert_eq!(r.text, "Hoi");
    let seen = m.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/messages");
    assert_eq!(seen[0].header("x-api-key"), Some("ck"));
}
