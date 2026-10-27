//! One error type for every provider.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum AiError {
    /// No provider is enabled and usable.
    #[error("no AI model is set up: add a provider in Settings › AI providers")]
    NotConfigured,
    #[error("there is no provider '{0}' in Settings › AI")]
    UnknownProvider(String),
    #[error("{provider} needs an API key: add it in Settings › AI providers or set {env}")]
    MissingKey { provider: String, env: String },
    #[error("{provider} needs its endpoint (URL): add it in Settings › AI providers")]
    MissingEndpoint { provider: String },
    #[error("{provider} can't do this: it's a {service} service")]
    WrongService { provider: String, service: &'static str },
    #[error("{provider} is turned off in Settings › AI")]
    ProviderDisabled { provider: String },
    #[error("the API key for {provider} was refused ({message})")]
    Auth { provider: String, message: String },
    #[error("{provider} has no model '{model}' ({message})")]
    ModelNotFound { provider: String, model: String, message: String },
    #[error("{provider} is rate-limiting requests; try again shortly")]
    RateLimited { provider: String, retry_after: Option<Duration> },
    #[error("{provider} is overloaded or unavailable (HTTP {status}); try again shortly")]
    Unavailable { provider: String, status: u16 },
    #[error("{provider} rejected the request (HTTP {status}): {message}")]
    Http { provider: String, status: u16, message: String },
    #[error("can't reach {provider} at {url}: {message}")]
    Network { provider: String, url: String, message: String },
    #[error("unexpected reply: {0}")]
    BadResponse(String),
    #[error("stopped")]
    Cancelled,
    #[error("can't save the AI settings: {0}")]
    Config(String),
}

impl AiError {
    /// Worth retrying the same request (before any output arrived).
    pub fn is_retryable(&self) -> bool {
        matches!(self, AiError::RateLimited { .. } | AiError::Unavailable { .. } | AiError::Network { .. })
    }

    /// Classify an HTTP error response.
    pub fn from_status(provider: &str, model: &str, status: u16, body: &str, retry_after: Option<Duration>) -> AiError {
        let message = error_message(body);
        let provider = provider.to_string();
        match status {
            401 | 403 => AiError::Auth { provider, message },
            404 => AiError::ModelNotFound { provider, model: model.to_string(), message },
            429 => AiError::RateLimited { provider, retry_after },
            500 | 502 | 503 | 504 | 529 => AiError::Unavailable { provider, status },
            _ => AiError::Http { provider, status, message },
        }
    }
}

/// The human part of an error body: `{"error": {"message": …}}` (all three APIs), else the text.
pub fn error_message(body: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let msg = parsed.as_ref().and_then(|v| {
        v.pointer("/error/message")
            .or_else(|| v.get("message"))
            .or_else(|| v.get("error").filter(|e| e.is_string()))
            .and_then(|m| m.as_str())
            .map(str::to_string)
    });
    let msg = msg.unwrap_or_else(|| body.trim().to_string());
    let mut msg: String = msg.chars().take(300).collect();
    if msg.is_empty() {
        msg = "no details".into();
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_statuses() {
        let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(AiError::from_status("anthropic", "m", 401, body, None), AiError::Auth { provider: "anthropic".into(), message: "invalid x-api-key".into() });
        assert!(AiError::from_status("p", "m", 529, "", None).is_retryable());
        assert!(AiError::from_status("p", "m", 429, "", None).is_retryable());
        assert!(!AiError::from_status("p", "m", 400, r#"{"error":{"message":"bad"}}"#, None).is_retryable());
        assert!(matches!(AiError::from_status("p", "m", 404, "", None), AiError::ModelNotFound { .. }));
    }

    #[test]
    fn error_messages() {
        assert_eq!(error_message(r#"{"error":{"code":400,"message":"API key not valid","status":"INVALID_ARGUMENT"}}"#), "API key not valid");
        assert_eq!(error_message(r#"{"error":"model 'x' not found"}"#), "model 'x' not found");
        assert_eq!(error_message("  plain text  "), "plain text");
        assert_eq!(error_message(""), "no details");
    }
}
