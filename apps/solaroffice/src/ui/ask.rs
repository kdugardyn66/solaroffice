//! Ask AI: questions answered from the starred files (GenOffice's knowledge-base panel). The
//! answering side comes with the AI layer; the page already shows the corpus.

use dioxus::prelude::*;
use solar_core::library::display_name;
use solar_core::AppKind;

use crate::icons::{self, Badge, Icon};
use crate::state::{Ctx, Page};

#[component]
pub fn AskAi() -> Element {
    let mut ctx = use_context::<Ctx>();
    let starred: Vec<_> = ctx.home.read().library.starred.iter().map(|s| s.path.clone()).collect();
    let n = starred.len();
    let mut draft = use_signal(String::new);

    rsx! {
        div { class: "max-w-[820px] mx-auto px-10 pt-12 pb-16",
            div { class: "flex items-center gap-3",
                span { class: "w-10 h-10 rounded-xl bg-accent-soft text-accent flex items-center justify-center", Icon { svg: icons::SPARKLES, size: 20 } }
                div {
                    h1 { class: "text-[22px] font-semibold tracking-tight", "Ask AI" }
                    p { class: "text-ink2", "Answers from your starred files, with citations." }
                }
            }

            div { class: "mt-7 rounded-2xl border border-line bg-card p-3",
                textarea {
                    class: "w-full h-24 resize-none bg-transparent outline-none p-2 placeholder:text-muted select-text",
                    placeholder: "Ask a question about your starred files…",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                }
                div { class: "flex items-center gap-2 px-1",
                    span { class: "text-[12px] text-muted flex-1",
                        if n == 0 { "Knowledge base is empty — star some files first." } else if n == 1 { "Knowledge base: 1 starred file" } else { "Knowledge base: {n} starred files" }
                    }
                    button {
                        class: "flex items-center gap-1.5 px-3 h-8 rounded-lg bg-accent text-on-accent font-medium disabled:opacity-40",
                        disabled: true,
                        title: "Answering comes with the AI layer (next milestone).",
                        Icon { svg: icons::SEND, size: 14 }
                        "Ask"
                    }
                }
            }
            p { class: "mt-2 text-[12px] text-muted", "AI providers and models are set up in the AI milestone (Anthropic, Mistral, OpenAI, local models, …)." }

            if n > 0 {
                div { class: "mt-8 text-[12px] font-medium text-muted", "Starred files" }
                div { class: "mt-2 flex flex-wrap gap-2",
                    for p in starred {
                        {
                            let kind = AppKind::for_path(&p);
                            let name = display_name(&p);
                            rsx! {
                                button { key: "{p.display()}", class: "flex items-center gap-2 pl-1.5 pr-3 h-8 rounded-lg border border-line bg-card hover:bg-hover",
                                    onclick: move |_| ctx.open_path(&p),
                                    Badge { kind, size: 20 }
                                    span { class: "max-w-[220px] truncate", "{name}" }
                                }
                            }
                        }
                    }
                }
            } else {
                button { class: "mt-6 text-accent hover:underline", onclick: move |_| ctx.go(Page::Recent), "Go to Recent to star files →" }
            }
        }
    }
}
