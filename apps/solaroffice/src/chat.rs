//! The AI chat behind Ask AI and the AI side pane: the conversation, the running job and the
//! request it builds. No UI code, so it is unit-tested.

use std::path::PathBuf;

use solar_ai::{AiError, AiHub, Job, JobEvent, Message, ModelRef, Part, Request, Role, Scope, StopReason};
use solar_core::AppKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub speaker: Speaker,
    pub text: String,
    /// The model that answered (assistant messages).
    pub model: Option<ModelRef>,
    /// The request failed; `text` is the error.
    pub error: bool,
    /// "1,234 tokens · $0.0021" once finished.
    pub footer: Option<String>,
    /// The model's reasoning (sent back to APIs that need it).
    pub reasoning: String,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> ChatMessage {
        ChatMessage { speaker: Speaker::User, text: text.into(), model: None, error: false, footer: None, reasoning: String::new() }
    }
}

/// One conversation with its composer.
#[derive(Default)]
pub struct ChatState {
    pub draft: String,
    pub messages: Vec<ChatMessage>,
    /// The module the answer is for; `None` is general chat.
    pub target: Option<AppKind>,
    /// A model picked in the composer, overriding Settings › AI for this chat.
    pub model: Option<ModelRef>,
    /// Use the starred files as context (Ask AI on Home).
    pub use_starred: bool,
    pub job: Option<Job>,
    /// "Retrying in 4 s…" or "Thinking…" while a request waits.
    pub status: Option<String>,
}

impl ChatState {
    pub fn new(target: Option<AppKind>, use_starred: bool) -> ChatState {
        ChatState { target, use_starred, ..ChatState::default() }
    }

    pub fn scope(&self) -> Scope {
        self.target.map(Scope::App).unwrap_or(Scope::Home)
    }

    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    /// The conversation so far as a request (failed turns left out).
    pub fn request(&self, context: Option<String>) -> Request {
        let mut r = Request::new(self.scope());
        r.model = self.model.clone();
        r.system = context;
        for m in self.messages.iter().filter(|m| !m.error && !m.text.is_empty()) {
            r.messages.push(match m.speaker {
                Speaker::User => Message::user(m.text.clone()),
                Speaker::Assistant if m.reasoning.is_empty() => Message::assistant(m.text.clone()),
                Speaker::Assistant => Message { role: Role::Assistant, parts: vec![Part::Reasoning { text: m.reasoning.clone() }, Part::text(m.text.clone())] },
            });
        }
        r
    }

    /// Send the draft; returns whether a request started.
    pub fn send(&mut self, hub: &AiHub, context: Option<String>) -> bool {
        let text = self.draft.trim().to_string();
        if text.is_empty() || self.running() {
            return false;
        }
        self.draft.clear();
        self.messages.push(ChatMessage::user(text));
        let job = hub.start(self.request(context), None);
        self.messages.push(ChatMessage { speaker: Speaker::Assistant, text: String::new(), model: job.model.clone(), error: false, footer: None, reasoning: String::new() });
        self.job = Some(job);
        true
    }

    /// Take in the running job's news; returns true if anything changed.
    pub fn pump(&mut self) -> bool {
        let Some(job) = self.job.as_mut() else { return false };
        let events = job.poll();
        if events.is_empty() {
            return false;
        }
        let mut done = false;
        for e in events {
            let last = self.messages.last_mut().filter(|m| m.speaker == Speaker::Assistant);
            match e {
                JobEvent::Text(t) => {
                    self.status = None;
                    if let Some(m) = last {
                        m.text.push_str(&t);
                    }
                }
                JobEvent::Reasoning(_) => {
                    if last.is_some_and(|m| m.text.is_empty()) {
                        self.status = Some("Thinking…".into());
                    }
                }
                JobEvent::ToolCall(_) => {}
                JobEvent::Retrying { wait, reason, .. } => self.status = Some(format!("{reason} — retrying in {} s…", wait.as_secs().max(1))),
                JobEvent::Done(result) => {
                    done = true;
                    self.status = None;
                    if let Some(m) = last {
                        match result {
                            Ok(r) => {
                                if m.text.is_empty() {
                                    m.text = r.text.clone();
                                }
                                m.reasoning = r.reasoning.clone();
                                let tokens = r.usage.input_tokens + r.usage.output_tokens;
                                let mut footer = format!("{} tokens", group(tokens));
                                if let Some(c) = r.cost_usd.filter(|c| *c > 0.0) {
                                    footer.push_str(&format!(" · ${c:.4}"));
                                }
                                if r.stop == StopReason::MaxTokens {
                                    footer.push_str(" · cut off at the length limit");
                                }
                                m.footer = Some(footer);
                            }
                            Err(AiError::Cancelled) => {
                                m.footer = Some("Stopped".into());
                                if m.text.is_empty() {
                                    m.error = true;
                                    m.text = "Stopped before the answer started.".into();
                                }
                            }
                            Err(e) => {
                                m.error = true;
                                m.text = if m.text.is_empty() { e.to_string() } else { format!("{}\n\n{}", m.text, e) };
                            }
                        }
                    }
                }
            }
        }
        if done {
            self.job = None;
        }
        true
    }

    pub fn stop(&self) {
        if let Some(j) = &self.job {
            j.cancel();
        }
    }

    pub fn clear(&mut self) {
        self.stop();
        self.job = None;
        self.messages.clear();
        self.status = None;
    }
}

/// 12345 → "12,345".
pub fn group(n: u64) -> String {
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

/// The most text of the starred files sent with a question.
pub const CONTEXT_BUDGET: usize = 60_000;

/// File types whose text can be read as is.
fn readable(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "txt" | "html" | "htm" | "csv" | "drawio" | "dio" | "svg"))
}

/// The instructions that carry the starred files: text files are included (up to `budget`
/// characters in all); other types are listed by name only.
pub fn starred_context(paths: &[PathBuf], budget: usize) -> Option<String> {
    let mut existing: Vec<&PathBuf> = paths.iter().filter(|p| p.is_file()).collect();
    existing.dedup();
    if existing.is_empty() {
        return None;
    }
    let mut out = String::from(
        "The user starred these files as their knowledge base. Answer from them when the question concerns them, and name the file you used. Say so when the answer isn't in them.\n",
    );
    let mut left = budget;
    let mut names_only: Vec<String> = Vec::new();
    for p in existing {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if readable(p)
            && left > 200
            && let Ok(text) = std::fs::read_to_string(p)
        {
            let take: String = text.chars().take(left.saturating_sub(100)).collect();
            let cut = take.chars().count() < text.chars().count();
            left = left.saturating_sub(take.chars().count() + 60);
            out.push_str(&format!("\n=== {name} ===\n{take}{}\n", if cut { "\n[…cut]" } else { "" }));
        } else {
            names_only.push(name);
        }
    }
    if !names_only.is_empty() {
        out.push_str(&format!("\nAlso starred (contents not available to you): {}.\n", names_only.join(", ")));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(group(7), "7");
        assert_eq!(group(12_345), "12,345");
        assert_eq!(group(1_234_567), "1,234,567");
    }

    #[test]
    fn request_skips_failed_and_empty_turns() {
        let mut c = ChatState::new(Some(AppKind::Grid), false);
        c.messages.push(ChatMessage::user("a"));
        let mut bad = ChatMessage::user("");
        bad.speaker = Speaker::Assistant;
        bad.error = true;
        bad.text = "boom".into();
        c.messages.push(bad);
        c.messages.push(ChatMessage::user("b"));
        let r = c.request(Some("ctx".into()));
        assert_eq!(r.messages.len(), 2);
        assert_eq!(r.scope, Scope::App(AppKind::Grid));
        assert_eq!(r.system.as_deref(), Some("ctx"));
    }

    #[test]
    fn send_needs_text_and_no_running_job() {
        let hub = AiHub::in_memory(solar_ai::AiConfig { providers: vec![], ..Default::default() });
        let mut c = ChatState::default();
        assert!(!c.send(&hub, None), "empty draft");
        c.draft = "  hi ".into();
        assert!(c.send(&hub, None));
        assert_eq!(c.messages[0].text, "hi");
        assert!(c.running());
        c.draft = "again".into();
        assert!(!c.send(&hub, None), "one request at a time");
        // No provider is configured: the job fails and the error lands in the answer.
        for _ in 0..100 {
            c.pump();
            if !c.running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!c.running());
        let last = c.messages.last().unwrap();
        assert!(last.error && !last.text.is_empty());
        c.clear();
        assert!(c.messages.is_empty());
    }

    #[test]
    fn starred_context_reads_text_and_lists_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let md = d.path().join("Notes.md");
        std::fs::write(&md, "Launch is on 3 March.").unwrap();
        let docx = d.path().join("Plan.docx");
        std::fs::write(&docx, "PK").unwrap();
        let missing = d.path().join("Gone.md");
        let ctx = starred_context(&[md, docx, missing], CONTEXT_BUDGET).unwrap();
        assert!(ctx.contains("=== Notes.md ===") && ctx.contains("Launch is on 3 March."));
        assert!(ctx.contains("Plan.docx") && !ctx.contains("PK"));
        assert!(!ctx.contains("Gone.md"));
        assert!(starred_context(&[], CONTEXT_BUDGET).is_none());
    }

    #[test]
    fn starred_context_respects_the_budget() {
        let d = tempfile::tempdir().unwrap();
        let big = d.path().join("Big.md");
        std::fs::write(&big, "x".repeat(10_000)).unwrap();
        let ctx = starred_context(&[big], 1_000).unwrap();
        assert!(ctx.len() < 1_600 && ctx.contains("[…cut]"));
    }
}
