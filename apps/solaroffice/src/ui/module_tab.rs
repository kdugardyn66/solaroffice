//! A module tab. The editors arrive in later milestones; until then the tab shows the file, offers
//! to open it in the system's default app, and carries the Solar AI pane on its left, the way
//! the editors will.

use dioxus::prelude::*;
use solar_core::fmt;
use solar_core::library::FileMeta;

use super::ask::AiChat;
use crate::files;
use crate::icons::{self, Badge, Icon};
use crate::state::Ctx;

#[component]
pub fn ModuleTab() -> Element {
    let mut ctx = use_context::<Ctx>();
    let active = *ctx.active.read();
    let Some(tab) = ctx.tabs.read().iter().find(|t| Some(t.id) == active).cloned() else {
        return rsx! {};
    };
    let module = tab.kind.map(|k| k.name()).unwrap_or("SolarOffice");
    let meta = tab.path.as_deref().map(FileMeta::read);
    let id = tab.id;
    let pane = *ctx.ai_pane.read();

    rsx! {
        if pane {
            // One conversation per tab, in that module's scope.
            aside { class: "w-[360px] shrink-0 bg-side border-r border-line", AiChat { key: "{id}", chat_id: id, target: tab.kind, compact: true } }
        }
        div { class: "flex-1 min-w-0 flex flex-col bg-app",
            div { class: "flex items-center h-10 px-3 border-b border-line shrink-0",
                button {
                    class: if pane { "flex items-center gap-1.5 px-2.5 h-7 rounded-md bg-accent-soft text-accent font-medium" } else { "flex items-center gap-1.5 px-2.5 h-7 rounded-md text-ink2 hover:bg-hover" },
                    title: "Show or hide the Solar AI pane",
                    onclick: move |_| ctx.ai_pane.set(!pane),
                    Icon { svg: icons::SPARKLES, size: 14 }
                    "Solar AI"
                }
            }
            div { class: "flex-1 flex items-center justify-center overflow-y-auto p-6",
                div { class: "w-[540px] max-w-full p-8 rounded-2xl border border-line bg-card text-center flex flex-col items-center",
                    Badge { kind: tab.kind, size: 56 }
                    div { class: "mt-4 text-[17px] font-semibold break-all", "{tab.title}" }
                    match (&tab.path, meta) {
                        (Some(p), Some(m)) => rsx! {
                            div { class: "mt-1 text-[12px] text-muted break-all", "{p.display()}" }
                            if m.exists {
                                div { class: "mt-1 text-[12px] text-ink2",
                                    "{fmt::size(m.size)}"
                                    if let Some(ms) = m.modified_ms { " · modified {fmt::when_local(ms)}" }
                                }
                            }
                        },
                        _ => rsx! {
                            div { class: "mt-1 text-[12px] text-muted break-all", "New file · will be saved in {tab.folder.display()}" }
                        },
                    }
                    p { class: "mt-5 text-ink2 leading-relaxed",
                        "The {module} editor arrives in a later milestone. This tab already behaves like one: ⌘W closes it, ⌘⇧T reopens it, and Solar AI answers in {module}'s scope."
                    }
                    div { class: "mt-6 flex gap-2 whitespace-nowrap",
                        if let Some(p) = tab.path.clone() {
                            {
                                let p2 = p.clone();
                                rsx! {
                                    button { class: "flex items-center gap-1.5 px-3.5 h-8 rounded-lg bg-accent text-on-accent font-medium hover:brightness-110",
                                        onclick: move |_| files::open_external(&p),
                                        Icon { svg: icons::EXTERNAL, size: 14 }
                                        "Open in default app"
                                    }
                                    button { class: "flex items-center gap-1.5 px-3.5 h-8 rounded-lg border border-line hover:bg-hover",
                                        onclick: move |_| files::reveal(&p2),
                                        Icon { svg: icons::FOLDER_OPEN, size: 14 }
                                        if cfg!(target_os = "macos") { "Reveal in Finder" } else { "Show in folder" }
                                    }
                                }
                            }
                        }
                        button { class: "px-3.5 h-8 rounded-lg border border-line hover:bg-hover", onclick: move |_| ctx.close_tab(id), "Close tab" }
                    }
                }
            }
        }
    }
}
