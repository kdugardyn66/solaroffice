//! DeepL's translation API (`POST {base}/v2/translate`, `GET {base}/v2/usage`).
//!
//! Free keys end in `:fx` and use `api-free.deepl.com` (see [`ProviderConfig::base_url`]).

use serde_json::{Map, Value, json};

use super::{json_call, s, thousands};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::Http;
use crate::media::{TranslateRequest, Translation};

pub fn headers(api_key: Option<&str>) -> Vec<(String, String)> {
    api_key.map(|k| vec![("authorization".to_string(), format!("DeepL-Auth-Key {k}"))]).unwrap_or_default()
}

pub fn body(req: &TranslateRequest) -> Value {
    let mut b = Map::new();
    b.insert("text".into(), json!(req.texts));
    b.insert("target_lang".into(), json!(req.target_lang.to_ascii_uppercase()));
    let mut opt = |k: &str, v: &Option<String>| {
        if let Some(v) = v.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            b.insert(k.into(), json!(v));
        }
    };
    opt("source_lang", &req.source_lang.as_ref().map(|l| l.to_ascii_uppercase()));
    opt("formality", &req.formality);
    opt("model_type", &req.model);
    opt("context", &req.context);
    b.insert("show_billed_characters".into(), json!(true));
    Value::Object(b)
}

pub fn parse(v: &Value) -> Result<Vec<Translation>, AiError> {
    let list = v.get("translations").and_then(Value::as_array).ok_or_else(|| AiError::BadResponse("no translations in the reply".into()))?;
    Ok(list
        .iter()
        .map(|t| Translation {
            text: s(t, "text").unwrap_or_default().to_string(),
            detected_source_language: s(t, "detected_source_language").map(str::to_string),
            billed_characters: t.get("billed_characters").and_then(Value::as_u64),
        })
        .collect())
}

pub fn translate(http: &Http, provider: &ProviderConfig, api_key: Option<&str>, req: &TranslateRequest) -> Result<Vec<Translation>, AiError> {
    if req.texts.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{}/v2/translate", provider.base_url());
    parse(&json_call(http, provider, &url, &headers(api_key), Some(&body(req)))?)
}

/// "12,345 of 500,000 characters used" (free to call; also checks the key).
pub fn usage(http: &Http, provider: &ProviderConfig, api_key: Option<&str>) -> Result<String, AiError> {
    let url = format!("{}/v2/usage", provider.base_url());
    let v = json_call(http, provider, &url, &headers(api_key), None)?;
    let n = |k: &str| v.get(k).and_then(Value::as_u64);
    Ok(match (n("character_count"), n("character_limit")) {
        (Some(c), Some(l)) if l > 0 && l < 1_000_000_000_000 => format!("{} of {} characters used", thousands(c), thousands(l)),
        (Some(c), _) => format!("{} characters used", thousands(c)),
        _ => "Connected".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_reply() {
        let req = TranslateRequest { texts: vec!["Hello".into()], target_lang: "nl".into(), source_lang: Some("en".into()), formality: Some("more".into()), model: Some("quality_optimized".into()), context: None };
        let b = body(&req);
        assert_eq!(b, json!({"text":["Hello"],"target_lang":"NL","source_lang":"EN","formality":"more","model_type":"quality_optimized","show_billed_characters":true}));
        let t = parse(&json!({"translations":[{"detected_source_language":"EN","text":"Hallo","billed_characters":5}]})).unwrap();
        assert_eq!(t, [Translation { text: "Hallo".into(), detected_source_language: Some("EN".into()), billed_characters: Some(5) }]);
    }
}
