//! Black Forest Labs' FLUX API: `POST {base}/v1/<model>` returns a task; poll its
//! `polling_url` until it's `Ready`, then download `result.sample` (a signed URL that expires
//! after about ten minutes). `GET {base}/v1/credits` checks the key for free.

use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::{json_call, s, thousands};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::{Cancel, Http, network_error};
use crate::media::{GeneratedImage, ImageRequest};

/// How long to wait for an image before giving up.
const TIMEOUT: Duration = Duration::from_secs(300);
const POLL: Duration = Duration::from_millis(500);

pub fn headers(api_key: Option<&str>) -> Vec<(String, String)> {
    api_key.map(|k| vec![("x-key".to_string(), k.to_string())]).unwrap_or_default()
}

pub fn body(req: &ImageRequest) -> Value {
    let mut b = Map::new();
    b.insert("prompt".into(), json!(req.prompt));
    if let Some(w) = req.width {
        b.insert("width".into(), json!(w));
    }
    if let Some(h) = req.height {
        b.insert("height".into(), json!(h));
    }
    if let Some(seed) = req.seed {
        b.insert("seed".into(), json!(seed));
    }
    b.insert("output_format".into(), json!(req.output_format.clone().unwrap_or_else(|| "png".into())));
    if let Some(img) = &req.input_image {
        b.insert("input_image".into(), json!(img));
    }
    Value::Object(b)
}

pub fn generate(http: &Http, provider: &ProviderConfig, api_key: Option<&str>, model: &str, req: &ImageRequest, cancel: &Cancel) -> Result<GeneratedImage, AiError> {
    let base = provider.base_url();
    let h = headers(api_key);
    let task = json_call(http, provider, &format!("{base}/v1/{model}"), &h, Some(&body(req)))?;
    let id = s(&task, "id").unwrap_or_default().to_string();
    // Always poll the returned URL (it routes to the region that runs the task).
    let poll_url = s(&task, "polling_url").filter(|u| !u.is_empty()).map(str::to_string).unwrap_or_else(|| format!("{base}/v1/get_result?id={id}"));
    let mut cost = task.get("cost").and_then(Value::as_f64);
    let started = Instant::now();
    let sample = loop {
        if !cancel.sleep(POLL) {
            return Err(AiError::Cancelled);
        }
        let v = json_call(http, provider, &poll_url, &h, None)?;
        if let Some(c) = v.get("cost").and_then(Value::as_f64) {
            cost = Some(c);
        }
        match s(&v, "status").unwrap_or("") {
            "Ready" => break v.pointer("/result/sample").and_then(Value::as_str).map(str::to_string).ok_or_else(|| AiError::BadResponse("the image is ready but has no URL".into()))?,
            "Pending" | "Reasoning" | "Generating" | "Queued" | "" => {}
            "Request Moderated" | "Content Moderated" => return Err(AiError::Http { provider: provider.display_name(), status: 400, message: "the prompt or image was blocked by content moderation".into() }),
            "Task not found" => return Err(AiError::BadResponse(format!("image task {id} not found"))),
            other => return Err(AiError::BadResponse(format!("image generation failed ({other})"))),
        }
        if started.elapsed() > TIMEOUT {
            return Err(AiError::BadResponse("the image took too long; try again".into()));
        }
    };
    let resp = http.agent_for(&sample).get(&sample).call().map_err(|e| network_error(&provider.id, &sample, e))?;
    if resp.status().as_u16() >= 400 {
        return Err(AiError::BadResponse(format!("can't download the image (HTTP {})", resp.status().as_u16())));
    }
    let media_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").to_string();
    let bytes = resp.into_body().with_config().limit(64 * 1024 * 1024).read_to_vec().map_err(|e| AiError::BadResponse(e.to_string()))?;
    let media_type = crate::types::sniff_image(&bytes).map(str::to_string).unwrap_or(media_type);
    // Credits: 1 credit = $0.01.
    Ok(GeneratedImage { bytes, media_type, url: sample, model: model.to_string(), cost_usd: cost.map(|c| c / 100.0) })
}

/// "1,234 credits ($12.34)".
pub fn credits(http: &Http, provider: &ProviderConfig, api_key: Option<&str>) -> Result<String, AiError> {
    let v = json_call(http, provider, &format!("{}/v1/credits", provider.base_url()), &headers(api_key), None)?;
    let c = v.get("credits").and_then(Value::as_f64).unwrap_or(0.0);
    Ok(format!("{} credits (${:.2})", thousands(c.max(0.0).round() as u64), c / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body() {
        let req = ImageRequest { prompt: "A sunrise over a café".into(), width: Some(1024), height: Some(768), ..ImageRequest::default() };
        assert_eq!(body(&req), json!({"prompt":"A sunrise over a café","width":1024,"height":768,"output_format":"png"}));
    }
}
