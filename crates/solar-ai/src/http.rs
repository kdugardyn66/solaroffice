//! HTTP plumbing: agents, Server-Sent Events, cancellation.

use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::error::AiError;

/// Stops a running request. Cheap to clone; all clones share the flag.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    /// Sleep up to `d`, waking early when cancelled. Returns `false` if cancelled.
    pub fn sleep(&self, d: Duration) -> bool {
        let step = Duration::from_millis(50);
        let mut left = d;
        while !left.is_zero() {
            if self.is_cancelled() {
                return false;
            }
            let s = left.min(step);
            std::thread::sleep(s);
            left -= s;
        }
        !self.is_cancelled()
    }
}

/// Two agents: one for hosted APIs (system proxy settings, the OS certificate store) and one for
/// servers on this machine or network, which never go through a proxy.
#[derive(Clone)]
pub struct Http {
    remote: ureq::Agent,
    local: ureq::Agent,
}

impl Http {
    pub fn new(first_byte_timeout: Duration) -> Http {
        let base = || {
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_connect(Some(Duration::from_secs(15)))
                .timeout_recv_response(Some(first_byte_timeout))
                .user_agent(format!("SolarOffice/{}", solar_core::VERSION))
        };
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        let remote = base().tls_config(tls).build().new_agent();
        let local = base().proxy(None).build().new_agent();
        Http { remote, local }
    }

    pub fn agent_for(&self, url: &str) -> &ureq::Agent {
        if is_local_url(url) { &self.local } else { &self.remote }
    }
}

/// `localhost`, loopback, private-network and `.local` addresses.
pub fn is_local_url(url: &str) -> bool {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host_port = rest.split('/').next().unwrap_or("");
    let host = if let Some(h) = host_port.strip_prefix('[') { h.split(']').next().unwrap_or("") } else { host_port.rsplit_once(':').map(|(h, _)| h).unwrap_or(host_port) };
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".local") || host.ends_with(".localhost") || host == "::1" {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return ip.is_loopback() || ip.is_private() || ip.is_link_local();
    }
    false
}

/// A `Retry-After` header in seconds.
pub fn retry_after(resp: &ureq::http::Response<ureq::Body>) -> Option<Duration> {
    resp.headers().get("retry-after")?.to_str().ok()?.trim().parse::<f64>().ok().filter(|s| *s >= 0.0).map(|s| Duration::from_secs_f64(s.min(120.0)))
}

/// Map a transport error.
pub fn network_error(provider: &str, url: &str, e: ureq::Error) -> AiError {
    let message = match &e {
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) if is_local_url(url) => format!("{e} — is the server running?"),
        _ => e.to_string(),
    };
    AiError::Network { provider: provider.into(), url: url.into(), message }
}

/// Reads Server-Sent Events, calling `on_event(event_name, data)` for each. Stops early (with
/// [`AiError::Cancelled`]) when `cancel` is set, or when `on_event` returns `false`.
pub fn read_sse(reader: impl Read, cancel: &Cancel, mut on_event: impl FnMut(&str, &str) -> Result<bool, AiError>) -> Result<(), AiError> {
    let mut lines = BufReader::new(reader);
    let mut event = String::new();
    let mut data = String::new();
    let mut line = String::new();
    loop {
        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        line.clear();
        let n = lines.read_line(&mut line).map_err(|e| AiError::BadResponse(format!("stream interrupted: {e}")))?;
        let eof = n == 0;
        let l = line.trim_end_matches(['\r', '\n']);
        if eof || l.is_empty() {
            if !data.is_empty() || !event.is_empty() {
                let keep_going = on_event(&event, &data)?;
                event.clear();
                data.clear();
                if !keep_going {
                    return Ok(());
                }
            }
            if eof {
                return Ok(());
            }
            continue;
        }
        if l.starts_with(':') {
            continue; // comment / keep-alive
        }
        let (field, value) = match l.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (l, ""),
        };
        match field {
            "event" => event = value.to_string(),
            "data" => {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_urls() {
        for u in ["http://localhost:11434/v1", "http://127.0.0.1:1234/v1", "http://192.168.1.20:8000/v1", "http://[::1]:8080", "http://studio.local:1234/v1", "10.0.0.5:8000"] {
            assert!(is_local_url(u), "{u}");
        }
        for u in ["https://api.anthropic.com", "https://openrouter.ai/api/v1", "https://8.8.8.8/v1"] {
            assert!(!is_local_url(u), "{u}");
        }
    }

    #[test]
    fn sse_parsing() {
        let stream = "event: a\ndata: {\"x\":1}\n\n: keep-alive\n\ndata: line1\ndata: line2\n\ndata: [DONE]\n\ndata: never";
        let mut got = Vec::new();
        read_sse(stream.as_bytes(), &Cancel::new(), |e, d| {
            got.push((e.to_string(), d.to_string()));
            Ok(d != "[DONE]")
        })
        .unwrap();
        assert_eq!(got, [("a".into(), "{\"x\":1}".into()), ("".into(), "line1\nline2".into()), ("".into(), "[DONE]".into())]);
    }

    #[test]
    fn sse_handles_crlf_and_trailing_event_and_cancel() {
        let mut got = Vec::new();
        read_sse("data: 1\r\n\r\ndata: 2".as_bytes(), &Cancel::new(), |_, d| {
            got.push(d.to_string());
            Ok(true)
        })
        .unwrap();
        assert_eq!(got, ["1", "2"]);
        let c = Cancel::new();
        c.cancel();
        assert_eq!(read_sse("data: 1\n\n".as_bytes(), &c, |_, _| Ok(true)), Err(AiError::Cancelled));
        assert!(!c.sleep(Duration::from_secs(5)), "cancelled sleep returns at once");
    }
}
