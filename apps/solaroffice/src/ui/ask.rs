//! The AI chat: Ask AI on Home (full page) and the AI pane next to module tabs (compact). Both
//! are [`AiChat`]; each conversation lives in `Ctx::chats` under its chat id, so it survives
//! switching tabs. Streaming answers, a model picker, "for module", the starred files as
//! context, stop, copy and clear.

use dioxus::prelude::*;
use solar_ai::{AiHub, ModelRef};
use solar_core::AppKind;

use super::pickers::{Dropdown, Entry, model_entries};
use crate::ai_state::{SettingsTab, model_summary};
use crate::chat::{ChatMessage, ChatState, Speaker, starred_context, CONTEXT_BUDGET};
use crate::files;
use crate::icons::{self, Badge, Icon};
use crate::state::{Ctx, HOME_CHAT};

const SUGGESTIONS: [(AppKind, &str); 4] = [
    (AppKind::Word, "Draft a one-page project proposal"),
    (AppKind::Grid, "Build a monthly household budget"),
    (AppKind::Deck, "Outline a 10-slide product pitch"),
    (AppKind::Markdown, "Write release notes from my changelog"),
];

/// Ask AI on Home.
#[component]
pub fn AskAi() -> Element {
    rsx! {
        AiChat { chat_id: HOME_CHAT, target: None, compact: false }
    }
}

/// Scroll the message list to the bottom when it is already near the bottom.
fn stick_to_bottom(chat_id: u64) {
    let js = format!("let e=document.getElementById('chat-scroll-{chat_id}'); if(e && e.scrollHeight-e.scrollTop-e.clientHeight<200) e.scrollTop=e.scrollHeight;");
    let _ = document::eval(&js);
}

/// Put `text` in the composer (the textarea keeps its own text while typing).
fn set_composer(chat_id: u64, text: &str) {
    let js = format!("let e=document.getElementById('chat-input-{chat_id}'); if(e){{e.value={}; e.focus();}}", serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into()));
    let _ = document::eval(&js);
}

#[component]
pub fn AiChat(chat_id: u64, target: Option<AppKind>, compact: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    let hub: AiHub = ctx.ai.read().clone();

    // Stream the running job into the conversation.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            let running = ctx.chats.peek().get(&chat_id).is_some_and(|c| c.running());
            if running {
                let changed = ctx.with_chat(chat_id, target, |c| c.pump());
                if changed {
                    stick_to_bottom(chat_id);
                }
            }
        }
    });

    let chats = ctx.chats.read();
    let default_chat;
    let chat = match chats.get(&chat_id) {
        Some(c) => c,
        None => {
            default_chat = ChatState::new(target, chat_id == HOME_CHAT);
            &default_chat
        }
    };
    let messages: Vec<ChatMessage> = chat.messages.clone();
    let running = chat.running();
    let status = chat.status.clone();
    let draft = chat.draft.clone();
    let use_starred = chat.use_starred;
    let chat_target = chat.target;
    let picked = chat.model.clone();
    let scope = chat.scope();
    drop(chats);

    let model = picked.clone().or_else(|| hub.model_for(scope));
    let config = hub.config();
    let model_label = match &model {
        Some(m) => model_summary(&hub, &config, m),
        None => "No model ready".to_string(),
    };
    let entries = model_entries(&hub, &config, &Default::default(), picked.as_ref(), Some("Default for this module"));
    let starred_n = ctx.home.read().library.starred.len();
    let empty = messages.is_empty();
    let col = if compact { "px-3" } else { "max-w-[780px] mx-auto px-8" };

    let mut send = move || {
        let hub = ctx.ai.peek().clone();
        let context = if ctx.chats.peek().get(&chat_id).is_some_and(|c| c.use_starred) {
            let paths: Vec<_> = ctx.home.peek().library.starred.iter().map(|s| s.path.clone()).collect();
            starred_context(&paths, CONTEXT_BUDGET)
        } else {
            None
        };
        if ctx.with_chat(chat_id, target, |c| c.send(&hub, context)) {
            set_composer(chat_id, "");
            stick_to_bottom(chat_id);
        }
    };

    rsx! {
        div { class: "flex flex-col h-full min-h-0",
            if compact {
                div { class: "flex items-center gap-2 px-3 h-10 border-b border-line shrink-0",
                    Icon { svg: icons::SPARKLES, size: 15, class: "text-accent" }
                    span { class: "font-semibold flex-1", "Solar AI" }
                    if !empty {
                        button {
                            class: "p-1 rounded text-muted hover:text-ink hover:bg-hover",
                            title: "New conversation",
                            onclick: move |_| ctx.with_chat(chat_id, target, |c| c.clear()),
                            Icon { svg: icons::BROOM, size: 14 }
                        }
                    }
                }
            }
            div { id: "chat-scroll-{chat_id}", class: "flex-1 min-h-0 overflow-y-auto",
                div { class: "{col} py-6",
                    if empty {
                        div { class: if compact { "pt-6 flex flex-col items-center text-center" } else { "pt-[8vh] flex flex-col items-center text-center" },
                            span { class: "w-12 h-12 rounded-2xl bg-accent-soft text-accent flex items-center justify-center", Icon { svg: icons::SPARKLES, size: 22 } }
                            div { class: if compact { "mt-3 text-[17px] font-semibold" } else { "mt-3 text-[24px] font-semibold tracking-tight" }, if compact { "How can I help?" } else { "Ask AI" } }
                            div { class: "mt-1 text-ink2 max-w-[440px]",
                                if compact { "Ask about this file, or describe what to write." } else { "Ask a question, or describe a document, workbook or deck and SolarOffice drafts it." }
                            }
                        }
                        if model.is_none() {
                            div { class: "mt-6 p-4 rounded-xl border border-line bg-card",
                                div { class: "font-medium", "No AI provider is ready yet" }
                                div { class: "mt-1 text-[12.5px] text-ink2", "Add an API key for Anthropic, OpenAI, Gemini or another provider — or connect a local Ollama / LM Studio server — in Settings." }
                                button {
                                    class: "mt-3 px-3.5 h-8 rounded-lg bg-accent text-on-accent font-medium hover:brightness-110",
                                    onclick: move |_| ctx.open_settings(SettingsTab::Providers),
                                    "Set up AI providers"
                                }
                            }
                        }
                        if !compact {
                            div { class: "mt-6 grid grid-cols-2 gap-2.5",
                                for (kind , text) in SUGGESTIONS {
                                    button {
                                        key: "{text}",
                                        class: "flex items-center gap-3 p-3 rounded-xl border border-line bg-card hover:bg-hover text-left",
                                        onclick: move |_| {
                                            ctx.with_chat(chat_id, target, |c| {
                                                c.draft = text.to_string();
                                                c.target = Some(kind);
                                            });
                                            set_composer(chat_id, text);
                                        },
                                        Badge { kind: Some(kind), size: 26 }
                                        span { "{text}" }
                                    }
                                }
                            }
                        }
                    } else {
                        for (i , m) in messages.iter().enumerate() {
                            Bubble { key: "{i}", message: m.clone(), streaming: running && i + 1 == messages.len() }
                        }
                        if let Some(s) = status {
                            div { class: "text-[12.5px] text-muted", "{s}" }
                        }
                    }
                }
            }

            // Composer
            div { class: "{col} pb-5 pt-1 shrink-0",
                div { class: "rounded-2xl border border-line bg-card focus-within:border-accent",
                    textarea {
                        id: "chat-input-{chat_id}",
                        class: "w-full resize-none bg-transparent outline-none px-4 pt-3 pb-1 placeholder:text-muted select-text text-[14px]",
                        rows: if compact { "2" } else { "3" },
                        placeholder: "Ask anything…  (Enter to send, Shift+Enter for a new line)",
                        initial_value: "{draft}",
                        oninput: move |e| ctx.with_chat(chat_id, target, |c| c.draft = e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter && !e.modifiers().contains(Modifiers::SHIFT) {
                                e.prevent_default();
                                send();
                            }
                        },
                    }
                    div { class: "flex items-center gap-1.5 px-2.5 pb-2.5 flex-wrap",
                        if !compact {
                            Dropdown {
                                label: chat_target.map(|k| format!("For {}", k.name())).unwrap_or_else(|| "General chat".into()),
                                width: 170,
                                entries: {
                                    let mut v = vec![Entry::plain("", "General chat", chat_target.is_none())];
                                    v.extend(AppKind::ALL.into_iter().map(|k| Entry::plain(k.id(), k.name(), chat_target == Some(k))));
                                    v
                                },
                                on_pick: move |key: String| ctx.with_chat(chat_id, target, |c| c.target = AppKind::ALL.into_iter().find(|k| k.id() == key)),
                            }
                        }
                        Dropdown {
                            label: model_label,
                            width: if compact { 200 } else { 300 },
                            entries,
                            on_pick: move |key: String| ctx.with_chat(chat_id, target, |c| c.model = ModelRef::parse(&key)),
                        }
                        if chat_id == HOME_CHAT {
                            button {
                                class: if use_starred { "flex items-center gap-1.5 px-2.5 h-9 rounded-lg border border-accent bg-accent-soft text-accent" } else { "flex items-center gap-1.5 px-2.5 h-9 rounded-lg border border-line text-ink2 hover:bg-hover" },
                                title: "Answer from your starred files",
                                onclick: move |_| ctx.with_chat(chat_id, target, |c| c.use_starred = !c.use_starred),
                                Icon { svg: icons::STAR, size: 14, filled: use_starred }
                                "Starred files ({starred_n})"
                            }
                        }
                        div { class: "flex-1" }
                        if !compact && !empty {
                            button {
                                class: "flex items-center gap-1.5 px-2.5 h-9 rounded-lg text-ink2 hover:bg-hover",
                                onclick: move |_| ctx.with_chat(chat_id, target, |c| c.clear()),
                                Icon { svg: icons::BROOM, size: 14 }
                                "New chat"
                            }
                        }
                        if running {
                            button {
                                class: "flex items-center justify-center w-9 h-9 rounded-lg bg-ink text-app hover:opacity-90",
                                title: "Stop",
                                onclick: move |_| ctx.with_chat(chat_id, target, |c| c.stop()),
                                Icon { svg: icons::STOP, size: 14, filled: true }
                            }
                        } else {
                            button {
                                class: "flex items-center justify-center w-9 h-9 rounded-lg bg-accent text-on-accent hover:brightness-110 disabled:opacity-40",
                                title: "Send",
                                disabled: draft.trim().is_empty(),
                                onclick: move |_| send(),
                                Icon { svg: icons::SEND, size: 15 }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Bubble(message: ChatMessage, streaming: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    match message.speaker {
        Speaker::User => rsx! {
            div { class: "flex justify-end mb-4",
                div { class: "max-w-[85%] px-4 py-2.5 rounded-2xl bg-sel whitespace-pre-wrap break-words select-text", "{message.text}" }
            }
        },
        Speaker::Assistant => {
            let model = message.model.as_ref().map(|m| m.model.clone()).unwrap_or_default();
            let text = message.text.clone();
            let waiting = streaming && message.text.is_empty();
            rsx! {
                div { class: "mb-5 group",
                    div { class: "flex items-center gap-1.5 mb-1 text-[11.5px] text-muted",
                        Icon { svg: icons::SPARKLES, size: 12, class: "text-accent" }
                        span { "{model}" }
                    }
                    if waiting {
                        div { class: "text-muted", "Thinking…" }
                    } else if message.error {
                        div { class: "px-3 py-2 rounded-lg bg-[#FEE4E2] text-[#B42318] dark:bg-[#4A1D1A] dark:text-[#FF9A90] whitespace-pre-wrap break-words select-text", "{message.text}" }
                    } else {
                        div { class: "whitespace-pre-wrap break-words leading-relaxed select-text", "{message.text}" }
                    }
                    if !streaming {
                        div { class: "mt-1.5 flex items-center gap-3 text-[11.5px] text-muted",
                            if let Some(f) = &message.footer {
                                span { "{f}" }
                            }
                            if !message.error {
                                button {
                                    class: "flex items-center gap-1 opacity-0 group-hover:opacity-100 hover:text-ink",
                                    onclick: move |_| {
                                        if files::copy_text(&text) {
                                            ctx.toast("Answer copied.");
                                        }
                                    },
                                    Icon { svg: icons::COPY, size: 12 }
                                    "Copy"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
