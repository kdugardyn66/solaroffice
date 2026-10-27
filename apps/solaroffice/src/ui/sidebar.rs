//! The home sidebar: brand, Recent / Starred / Ask AI, the folder tree, Settings.

use std::path::PathBuf;

use dioxus::prelude::*;
use solar_core::folders;

use crate::icons::{self, Icon};
use crate::state::{Ctx, Menu, MenuKind, Page};

/// The app icon, embedded.
pub fn logo_src() -> String {
    use base64::Engine;
    let png = include_bytes!("../../../../assets/app-icon/solaroffice-256.png");
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

#[component]
pub fn Sidebar() -> Element {
    let mut ctx = use_context::<Ctx>();
    let page = ctx.page.read().clone();
    let (recent_n, starred_n) = ctx.home.read().library.counts(None);
    let roots = ctx.home.read().settings.roots();
    let logo = use_hook(logo_src);

    rsx! {
        aside { class: "w-[248px] shrink-0 flex flex-col bg-side border-r border-line",
            div { class: "flex items-center gap-2.5 px-5 pt-5 pb-4",
                img { src: "{logo}", class: "w-8 h-8 rounded-[8px]", draggable: "false" }
                span { class: "text-[19px] font-semibold tracking-tight", "SolarOffice" }
            }
            nav { class: "px-3 flex flex-col gap-0.5",
                NavItem {
                    icon: icons::CLOCK,
                    label: "Recent",
                    count: Some(recent_n),
                    active: page == Page::Recent,
                    onclick: move |_| ctx.go(Page::Recent),
                }
                NavItem {
                    icon: icons::STAR,
                    label: "Starred",
                    count: Some(starred_n),
                    active: page == Page::Starred,
                    onclick: move |_| ctx.go(Page::Starred),
                }
                NavItem {
                    icon: icons::SPARKLES,
                    label: "Ask AI",
                    count: None,
                    active: page == Page::AskAi,
                    onclick: move |_| ctx.go(Page::AskAi),
                }
            }
            div { class: "mx-4 my-3 border-t border-line" }
            div { class: "flex items-center px-5 pb-1 text-[12px] font-medium text-muted",
                span { class: "flex-1", "Folders" }
                button {
                    class: "p-1 rounded hover:bg-hover hover:text-ink",
                    title: "Add folder to sidebar…",
                    onclick: move |_| ctx.add_folder_dialog(),
                    Icon { svg: icons::FOLDER_PLUS, size: 14 }
                }
            }
            div { class: "flex-1 min-h-0 overflow-y-auto px-3 pb-2",
                for (i, root) in roots.into_iter().enumerate() {
                    FolderNode { key: "{root.display()}", path: root, depth: 0, is_root: true, is_save: i == 0 }
                }
            }
            div { class: "border-t border-line px-3 py-2 flex items-center",
                button {
                    class: "flex items-center gap-2.5 flex-1 px-2.5 py-1.5 rounded-md text-ink2 hover:bg-hover hover:text-ink",
                    onclick: move |_| ctx.open_settings(crate::ai_state::SettingsTab::General),
                    Icon { svg: icons::SETTINGS, size: 16 }
                    span { "Settings" }
                }
                span { class: "text-[11px] text-muted pr-2", "v{solar_core::VERSION}" }
            }
        }
    }
}

#[component]
fn NavItem(icon: &'static str, label: &'static str, count: Option<usize>, active: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let look = if active { "bg-sel text-ink font-medium" } else { "text-ink2 hover:bg-hover hover:text-ink" };
    rsx! {
        button {
            class: "flex items-center gap-2.5 px-2.5 py-[7px] rounded-md text-left {look}",
            onclick: move |e| onclick.call(e),
            Icon { svg: icon, size: 16, class: if active { "text-accent" } else { "" } }
            span { class: "flex-1", "{label}" }
            if let Some(n) = count {
                span { class: "text-[12px] text-muted tabular-nums", "{n}" }
            }
        }
    }
}

#[component]
fn FolderNode(path: PathBuf, depth: u32, is_root: bool, is_save: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    let _ = ctx.refresh.read();
    let expanded = ctx.home.read().settings.is_expanded(&path);
    let active = *ctx.page.read() == Page::Folder(path.clone());
    let available = path.is_dir();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
    let children = if expanded && available { folders::list(&path).folders } else { Vec::new() };
    let has_children = !available || expanded || std::fs::read_dir(&path).map(|rd| rd.flatten().any(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !e.file_name().to_string_lossy().starts_with('.'))).unwrap_or(false);
    let indent = 4 + depth * 14;
    let look = if active { "bg-sel text-ink font-medium" } else { "text-ink2 hover:bg-hover hover:text-ink" };
    let dim = if available { "" } else { "opacity-50" };
    let title = if available { path.display().to_string() } else { format!("{} — unavailable", path.display()) };

    let p_toggle = path.clone();
    let p_go = path.clone();
    let p_menu = path.clone();
    rsx! {
        div {
            class: "group flex items-center gap-1 pr-1 py-[5px] rounded-md cursor-default {look} {dim}",
            style: "padding-left:{indent}px",
            title: "{title}",
            onclick: move |_| ctx.go(Page::Folder(p_go.clone())),
            button {
                class: "p-0.5 rounded text-muted hover:text-ink",
                style: if has_children { "" } else { "visibility:hidden" },
                onclick: move |e| {
                    e.stop_propagation();
                    ctx.home.write().settings.set_expanded(&p_toggle, !expanded);
                },
                Icon { svg: if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, size: 13 }
            }
            Icon { svg: if active { icons::FOLDER_OPEN } else { icons::FOLDER }, size: 15, class: if is_save { "text-accent" } else { "" } }
            span { class: "flex-1 truncate ml-1", "{name}" }
            if is_root || available {
                button {
                    class: "p-0.5 rounded text-muted hover:text-ink hover:bg-hover opacity-0 group-hover:opacity-100",
                    title: "More",
                    onclick: move |e| {
                        e.stop_propagation();
                        let c = e.client_coordinates();
                        ctx.menu.set(Some(Menu { kind: MenuKind::Folder(p_menu.clone()), x: c.x, y: c.y + 8.0 }));
                    },
                    Icon { svg: icons::MORE, size: 14 }
                }
            }
        }
        for child in children {
            FolderNode { key: "{child.display()}", path: child, depth: depth + 1, is_root: false, is_save: false }
        }
    }
}
