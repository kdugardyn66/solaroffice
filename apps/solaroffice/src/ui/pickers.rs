//! A drop-down list with rich rows (provider captions, model names with tags, category icons and
//! prices), used for the model pickers in Settings and in the chat composer.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use solar_ai::{AiConfig, AiHub, Category, ModelRef};

use crate::ai_state::{TestState, by_region, flag_emoji, offered, provider_fallback};
use crate::icons::{self, Icon};

#[derive(Clone, PartialEq)]
pub enum Entry {
    /// A provider caption: "🇩🇪 Mistral AI".
    Header { country: String, text: String },
    Item { key: String, name: String, tag: Option<&'static str>, cats: Vec<Category>, price: String, selected: bool, tooltip: String },
}

impl Entry {
    /// A plain row (no price, no categories).
    pub fn plain(key: impl Into<String>, name: impl Into<String>, selected: bool) -> Entry {
        Entry::Item { key: key.into(), name: name.into(), tag: None, cats: Vec::new(), price: String::new(), selected, tooltip: String::new() }
    }
}

fn price_text(info: &solar_ai::ModelInfo) -> String {
    match info.price_tier() {
        "" => info.price_label(),
        tier => format!("{tier}  {}", info.price_label().replace(" per 1M tokens", " /1M")),
    }
}

fn tooltip(info: &solar_ai::ModelInfo) -> String {
    let good = if info.categories.is_empty() { String::new() } else { format!(" · Good at: {}", info.categories.iter().map(|c| c.label()).collect::<Vec<_>>().join(", ")) };
    format!("{}{good}", info.price_label())
}

/// The rows of a model picker: every usable chat provider's models under its caption.
/// Key `""` is the "none" row (`none_label`); other keys are `provider:model`.
pub fn model_entries(hub: &AiHub, config: &AiConfig, tests: &HashMap<String, TestState>, value: Option<&ModelRef>, none_label: Option<&str>) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(l) = none_label {
        out.push(Entry::plain("", l, value.is_none()));
    }
    for p in by_region(&config.providers).into_iter().filter(|p| p.usable_for_chat()) {
        let models = offered(hub, tests, p);
        if models.is_empty() {
            continue;
        }
        let provider_default = p.default_model.clone().or_else(|| provider_fallback(hub, tests, p));
        out.push(Entry::Header { country: p.country(), text: p.display_name() });
        for m in models {
            let r = ModelRef::new(p.id.clone(), m.clone());
            let info = hub.model_info(&r);
            out.push(Entry::Item {
                key: r.to_string(),
                name: info.name.clone(),
                tag: (provider_default.as_deref() == Some(m.as_str())).then_some("default"),
                cats: info.categories.clone(),
                price: price_text(&info),
                selected: value == Some(&r),
                tooltip: tooltip(&info),
            });
        }
    }
    out
}

/// The rows of a provider's "Default model" picker; key `""` is Automatic.
pub fn provider_model_entries(hub: &AiHub, tests: &HashMap<String, TestState>, p: &solar_ai::ProviderConfig) -> Vec<Entry> {
    let fallback = provider_fallback(hub, tests, p);
    let info_of = |m: &str| hub.catalog().info(p.kind, m);
    let auto = fallback.as_deref().map(|m| format!("Automatic ({})", info_of(m).name)).unwrap_or_else(|| "Automatic".into());
    let mut out = vec![Entry::plain("", auto, p.default_model.is_none())];
    for m in offered(hub, tests, p) {
        let info = info_of(&m);
        out.push(Entry::Item {
            key: m.clone(),
            name: info.name.clone(),
            tag: info.recommended.then_some("recommended"),
            cats: info.categories.clone(),
            price: price_text(&info),
            selected: p.default_model.as_deref() == Some(m.as_str()),
            tooltip: tooltip(&info),
        });
    }
    out
}

/// A button showing `label` that opens the list; picking a row calls `on_pick` with its key.
#[component]
pub fn Dropdown(label: String, entries: Vec<Entry>, on_pick: EventHandler<String>, #[props(default = 380)] width: u32, #[props(default)] disabled: bool) -> Element {
    let mut open = use_signal(|| None::<(f64, f64, bool)>);
    let mut anchor = use_signal(|| None::<Rc<MountedData>>);
    let list_h = 340.0;

    rsx! {
        button {
            class: "flex items-center gap-2 h-9 pl-3 pr-2 rounded-lg border border-line bg-app text-left hover:border-[color:var(--so-line-strong)] disabled:opacity-50 max-w-full",
            style: "width:{width}px",
            disabled,
            onmounted: move |e| anchor.set(Some(e.data())),
            onclick: move |_| {
                if open.peek().is_some() {
                    open.set(None);
                    return;
                }
                let Some(a) = anchor.peek().clone() else { return };
                spawn(async move {
                    if let Ok(r) = a.get_client_rect().await {
                        let win = dioxus::desktop::window();
                        let h = win.window.inner_size().to_logical::<f64>(win.window.scale_factor()).height;
                        let up = r.max_y() + list_h + 12.0 > h && r.min_y() > list_h;
                        open.set(Some((r.min_x(), if up { r.min_y() - 4.0 } else { r.max_y() + 4.0 }, up)));
                    }
                });
            },
            span { class: "flex-1 truncate", "{label}" }
            Icon { svg: icons::CHEVRON_DOWN, size: 14, class: "text-muted" }
        }
        if let Some((x, y, up)) = *open.read() {
            {
                let win = dioxus::desktop::window();
                let w = win.window.inner_size().to_logical::<f64>(win.window.scale_factor()).width;
                let left = x.min(w - 560.0).max(8.0);
                let pos = if up { format!("left:{left}px;bottom:{}px", win.window.inner_size().to_logical::<f64>(win.window.scale_factor()).height - y) } else { format!("left:{left}px;top:{y}px") };
                rsx! {
                    div { class: "fixed inset-0 z-[65]", onclick: move |_| open.set(None) }
                    div {
                        class: "fixed z-[70] w-[540px] max-w-[calc(100vw-16px)] max-h-[340px] overflow-y-auto py-1 rounded-xl border border-line bg-card shadow-[0_12px_32px_-8px_rgba(0,0,0,.28)]",
                        style: "{pos}",
                        for (i, entry) in entries.iter().enumerate() {
                            match entry.clone() {
                                Entry::Header { country, text } => rsx! {
                                    div { key: "h{i}", class: "flex items-center gap-1.5 px-3 pt-2 pb-1 text-[11.5px] font-medium text-muted border-t border-line first:border-t-0",
                                        span { "{flag_emoji(&country)}" }
                                        "{text}"
                                    }
                                },
                                Entry::Item { key, name, tag, cats, price, selected, tooltip } => {
                                    let k = key.clone();
                                    let cat_names = cats.iter().map(|c| c.label()).collect::<Vec<_>>().join(", ");
                                    let look = if selected { "bg-accent-soft" } else { "hover:bg-hover" };
                                    rsx! {
                                        button {
                                            key: "{key}",
                                            class: "w-full flex items-center gap-2 px-3 h-[30px] text-left {look}",
                                            title: "{tooltip}",
                                            onclick: move |_| {
                                                open.set(None);
                                                on_pick.call(k.clone());
                                            },
                                            span { class: "truncate", "{name}" }
                                            if let Some(t) = tag {
                                                span { class: "shrink-0 px-1.5 rounded text-[11px] leading-[16px] font-medium bg-accent-soft text-accent", "{t}" }
                                            }
                                            span { class: "flex-1" }
                                            if !cats.is_empty() {
                                                span { class: "shrink-0 text-[12px] tracking-wide", title: "{cat_names}",
                                                    for c in cats.clone() { "{c.emoji()}" }
                                                }
                                            }
                                            if !price.is_empty() {
                                                span { class: "shrink-0 text-[12px] text-muted tabular-nums pl-2", "{price}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
