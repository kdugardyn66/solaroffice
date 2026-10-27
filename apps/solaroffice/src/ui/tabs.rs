//! The tab strip, which is also the window's title bar: Home, the module tabs, + and the theme
//! switch. Dragging the empty strip moves the window; a double click zooms it.

use dioxus::prelude::*;

use crate::icons::{self, Badge, Icon};
use crate::state::{Ctx, Menu, MenuKind};

/// Room for the macOS traffic lights.
const TRAFFIC_LIGHTS: u32 = if cfg!(target_os = "macos") { 78 } else { 8 };

#[component]
pub fn TabStrip() -> Element {
    let mut ctx = use_context::<Ctx>();
    let tabs = ctx.tabs.read().clone();
    let active = *ctx.active.read();
    let dark = ctx.dark();

    rsx! {
        div {
            class: "flex items-end h-10 shrink-0 bg-strip border-b border-line pr-2 gap-0.5",
            style: "padding-left:{TRAFFIC_LIGHTS}px",
            onmousedown: move |e| {
                if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                    dioxus::desktop::window().drag();
                }
            },
            ondoubleclick: move |_| dioxus::desktop::window().toggle_maximized(),

            // Home
            TabButton {
                active: active.is_none(),
                onclick: move |_| ctx.active.set(None),
                Icon { svg: icons::HOME, size: 15 }
                span { class: "truncate", "Home" }
            }

            for tab in tabs {
                TabButton {
                    key: "{tab.id}",
                    active: active == Some(tab.id),
                    title: tab.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| format!("New file — saved to {}", tab.folder.display())),
                    onclick: move |_| ctx.active.set(Some(tab.id)),
                    onmiddle: move |_| ctx.close_tab(tab.id),
                    Badge { kind: tab.kind, size: 16 }
                    span { class: "truncate flex-1", "{tab.title}" }
                    button {
                        class: "ml-1 -mr-1 p-0.5 rounded text-muted hover:text-ink hover:bg-hover opacity-0 group-hover:opacity-100 group-[.is-active]:opacity-100",
                        title: "Close tab (⌘W)",
                        onmousedown: |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            ctx.close_tab(tab.id);
                        },
                        Icon { svg: icons::X, size: 13 }
                    }
                }
            }

            button {
                class: "mb-1.5 ml-1 p-1.5 rounded-md text-ink2 hover:bg-hover",
                title: "New file (⌘T)",
                onmousedown: |e| e.stop_propagation(),
                onclick: move |e| {
                    e.stop_propagation();
                    let p = e.client_coordinates();
                    ctx.menu.set(Some(Menu { kind: MenuKind::NewFile, x: p.x - 12.0, y: 40.0 }));
                },
                Icon { svg: icons::PLUS, size: 16 }
            }

            div { class: "flex-1 self-stretch" }

            button {
                class: "mb-1.5 p-1.5 rounded-md text-ink2 hover:bg-hover",
                title: if dark { "Switch to light theme" } else { "Switch to dark theme" },
                onmousedown: |e| e.stop_propagation(),
                ondoubleclick: |e| e.stop_propagation(),
                onclick: move |_| ctx.toggle_theme(),
                Icon { svg: if dark { icons::SUN } else { icons::MOON }, size: 16 }
            }
        }
    }
}

#[component]
fn TabButton(
    active: bool,
    #[props(default)] title: String,
    onclick: EventHandler<MouseEvent>,
    #[props(default)] onmiddle: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    let look = if active { "is-active bg-side text-ink border-line shadow-[0_1px_0_0_var(--so-side)]" } else { "text-ink2 border-transparent hover:bg-hover" };
    rsx! {
        div {
            class: "group flex items-center gap-2 h-[33px] px-3 min-w-[120px] max-w-[220px] rounded-t-lg border border-b-0 cursor-default {look}",
            title: "{title}",
            onmousedown: move |e| {
                e.stop_propagation();
                if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Auxiliary)
                    && let Some(h) = onmiddle
                {
                    h.call(e);
                }
            },
            ondoubleclick: |e| e.stop_propagation(),
            onclick: move |e| onclick.call(e),
            {children}
        }
    }
}
