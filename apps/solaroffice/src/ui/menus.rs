//! Pop-up menus, dialogs and the status toast.

use std::path::PathBuf;

use dioxus::prelude::*;
use solar_core::library::display_name;
use solar_core::settings::ThemePref;
use solar_core::{AppKind, now_ms};

use crate::files;
use crate::icons::{self, Badge, Icon};
use crate::state::{Ctx, MenuKind, Modal, Page};

#[component]
pub fn Overlays() -> Element {
    let mut ctx = use_context::<Ctx>();
    let menu = ctx.menu.read().clone();
    let modal = ctx.modal.read().clone();
    let toast = ctx.toast.read().clone();

    rsx! {
        if let Some(menu) = menu {
            // A click anywhere else closes the menu.
            div {
                class: "fixed inset-0 z-40",
                onclick: move |_| ctx.menu.set(None),
                oncontextmenu: move |e| {
                    e.prevent_default();
                    ctx.menu.set(None);
                },
            }
            {
                let (w, h) = window_size();
                let est_h = match &menu.kind {
                    MenuKind::NewFile => 330.0,
                    MenuKind::Row(_) => 340.0,
                    _ => 160.0,
                };
                let x = if menu.x <= 0.0 { 120.0 } else { menu.x.min(w - 252.0).max(8.0) };
                let y = if menu.y + est_h > h { (menu.y - est_h).max(8.0) } else { menu.y };
                rsx! {
                    div {
                        class: "fixed z-50 min-w-[230px] py-1.5 rounded-xl border border-line bg-card shadow-[0_12px_32px_-8px_rgba(0,0,0,.28)] text-[13px]",
                        style: "left:{x}px;top:{y}px",
                        onclick: |e| e.stop_propagation(),
                        match menu.kind {
                            MenuKind::NewFile => rsx! { NewFileMenu {} },
                            MenuKind::Row(p) => rsx! { RowMenu { path: p } },
                            MenuKind::Folder(p) => rsx! { FolderMenu { path: p } },
                        }
                    }
                }
            }
        }

        if let Some(modal) = modal {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30",
                onclick: move |_| ctx.modal.set(None),
                div {
                    class: "rounded-2xl border border-line bg-card shadow-[0_24px_60px_-12px_rgba(0,0,0,.4)]",
                    onclick: |e| e.stop_propagation(),
                    match modal {
                        Modal::ConfirmTrash(paths) => rsx! { ConfirmTrash { paths } },
                        Modal::Settings => rsx! { SettingsDialog {} },
                        Modal::About => rsx! { About {} },
                    }
                }
            }
        }

        if let Some((_, text)) = toast {
            div { class: "fixed bottom-6 left-1/2 -translate-x-1/2 z-[60] px-4 py-2.5 rounded-lg bg-[#262626] text-white text-[13px] shadow-lg max-w-[560px]",
                "{text}"
            }
        }
    }
}

fn window_size() -> (f64, f64) {
    let w = dioxus::desktop::window();
    let size = w.window.inner_size().to_logical::<f64>(w.window.scale_factor());
    (size.width, size.height)
}

#[component]
fn Item(icon: &'static str, label: String, #[props(default)] hint: String, #[props(default)] danger: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let color = if danger { "text-[#D92D20] dark:text-[#FF7A6E]" } else { "" };
    rsx! {
        button {
            class: "w-full flex items-center gap-2.5 px-3 py-[7px] hover:bg-hover text-left {color}",
            onclick: move |e| onclick.call(e),
            Icon { svg: icon, size: 15, class: if danger { "" } else { "text-ink2" } }
            span { class: "flex-1", "{label}" }
            if !hint.is_empty() {
                span { class: "text-[11.5px] text-muted pl-4", "{hint}" }
            }
        }
    }
}

fn sep() -> Element {
    rsx! { div { class: "my-1.5 border-t border-line" } }
}

#[component]
fn NewFileMenu() -> Element {
    let mut ctx = use_context::<Ctx>();
    rsx! {
        div { class: "px-3 pt-1 pb-1.5 text-[11.5px] font-medium text-muted", "New" }
        for (i, kind) in AppKind::ALL.into_iter().enumerate() {
            button {
                key: "{kind.id()}",
                class: "w-full flex items-center gap-2.5 px-3 py-[6px] hover:bg-hover text-left",
                onclick: move |_| ctx.new_file(kind),
                Badge { kind: Some(kind), size: 18 }
                span { class: "flex-1", "{kind.name()}" }
                span { class: "text-[11.5px] text-muted pl-4", "⌘⇧{i + 1}" }
            }
        }
        {sep()}
        Item { icon: icons::FOLDER_OPEN, label: "Open local file…", hint: "⌘O", onclick: move |_| ctx.open_dialog() }
    }
}

#[component]
fn RowMenu(path: PathBuf) -> Element {
    let mut ctx = use_context::<Ctx>();
    let starred = ctx.home.read().library.is_starred(&path);
    let in_recent = *ctx.page.read() == Page::Recent;
    let exists = path.exists();
    let p = path.clone();
    let (p1, p2, p3, p4, p5, p6, p7, p8) = (p.clone(), p.clone(), p.clone(), p.clone(), p.clone(), p.clone(), p.clone(), p.clone());

    rsx! {
        if exists {
            Item { icon: icons::EXTERNAL, label: "Open", onclick: move |_| ctx.open_path(&p1) }
            Item { icon: icons::FOLDER_OPEN, label: if cfg!(target_os = "macos") { "Reveal in Finder" } else { "Show in folder" },
                onclick: move |_| {
                    ctx.menu.set(None);
                    files::reveal(&p2);
                },
            }
        }
        Item { icon: icons::COPY, label: "Copy path",
            onclick: move |_| {
                ctx.menu.set(None);
                if files::copy_text(&p3.display().to_string()) {
                    ctx.toast("Path copied.");
                }
            },
        }
        {sep()}
        if exists {
            Item { icon: icons::PENCIL, label: "Rename",
                onclick: move |_| {
                    ctx.menu.set(None);
                    ctx.renaming.set(Some((p4.clone(), files::stem(&p4))));
                },
            }
            Item { icon: icons::FOLDER, label: "Move to folder…",
                onclick: move |_| {
                    ctx.menu.set(None);
                    let from = p5.clone();
                    spawn(async move {
                        if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Move to folder").pick_folder().await {
                            match files::move_into(&from, dir.path()) {
                                Ok(to) => {
                                    ctx.moved(&from, &to);
                                    ctx.toast(format!("Moved to {}.", display_name(dir.path())));
                                }
                                Err(e) => ctx.toast(format!("Couldn't move the file: {e}")),
                            }
                        }
                    });
                },
            }
            Item { icon: icons::COPY, label: "Duplicate",
                onclick: move |_| {
                    ctx.menu.set(None);
                    match files::duplicate(&p6) {
                        Ok(to) => {
                            ctx.home.write().library.touch(&to, now_ms());
                            ctx.refresh += 1;
                            ctx.toast(format!("Created “{}”.", display_name(&to)));
                        }
                        Err(e) => ctx.toast(format!("Couldn't duplicate: {e}")),
                    }
                },
            }
        }
        Item { icon: icons::STAR, label: if starred { "Unstar" } else { "Star" },
            onclick: move |_| {
                ctx.menu.set(None);
                ctx.home.write().library.set_starred(&p7, !starred, now_ms());
            },
        }
        {sep()}
        if in_recent || !exists {
            Item { icon: icons::MINUS_CIRCLE, label: "Remove from list",
                onclick: move |_| {
                    ctx.menu.set(None);
                    ctx.home.write().library.forget(std::slice::from_ref(&p8));
                },
            }
        }
        if exists {
            Item { icon: icons::TRASH, label: "Move to Trash", danger: true,
                onclick: move |_| {
                    ctx.menu.set(None);
                    ctx.modal.set(Some(Modal::ConfirmTrash(vec![path.clone()])));
                },
            }
        }
    }
}

#[component]
fn FolderMenu(path: PathBuf) -> Element {
    let mut ctx = use_context::<Ctx>();
    let is_added = ctx.home.read().settings.folders.contains(&path);
    let available = path.is_dir();
    let (p1, p2, p3) = (path.clone(), path.clone(), path.clone());
    rsx! {
        if available {
            Item { icon: icons::FOLDER_OPEN, label: if cfg!(target_os = "macos") { "Open in Finder" } else { "Open in file manager" },
                onclick: move |_| {
                    ctx.menu.set(None);
                    files::open_external(&p1);
                },
            }
            Item { icon: icons::FOLDER_PLUS, label: "New folder",
                onclick: move |_| {
                    ctx.menu.set(None);
                    match files::new_folder(&p2) {
                        Ok(dir) => {
                            ctx.home.write().settings.set_expanded(&p2, true);
                            ctx.refresh += 1;
                            ctx.go(Page::Folder(dir));
                        }
                        Err(e) => ctx.toast(format!("Couldn't create the folder: {e}")),
                    }
                },
            }
        }
        if is_added {
            {sep()}
            Item { icon: icons::MINUS_CIRCLE, label: "Remove from sidebar",
                onclick: move |_| {
                    ctx.menu.set(None);
                    ctx.home.write().settings.remove_folder(&p3);
                    if matches!(&*ctx.page.peek(), Page::Folder(f) if f.starts_with(&p3)) {
                        ctx.go(Page::Recent);
                    }
                },
            }
        }
    }
}

#[component]
fn ConfirmTrash(paths: Vec<PathBuf>) -> Element {
    let mut ctx = use_context::<Ctx>();
    let n = paths.len();
    let title = if n == 1 { format!("Move “{}” to the Trash?", display_name(&paths[0])) } else { format!("Move {n} files to the Trash?") };
    rsx! {
        div { class: "w-[420px] p-6",
            div { class: "flex gap-3",
                span { class: "w-9 h-9 shrink-0 rounded-full bg-[#FEE4E2] text-[#D92D20] flex items-center justify-center", Icon { svg: icons::TRASH, size: 17 } }
                div {
                    div { class: "font-semibold text-[15px] break-words", "{title}" }
                    div { class: "mt-1 text-ink2", "You can restore files from the system Trash." }
                }
            }
            div { class: "mt-6 flex justify-end gap-2",
                button { class: "px-3.5 h-8 rounded-lg border border-line hover:bg-hover", onclick: move |_| ctx.modal.set(None), "Cancel" }
                button {
                    class: "px-3.5 h-8 rounded-lg bg-[#D92D20] text-white hover:brightness-110",
                    onmounted: move |e| async move {
                        let _ = e.set_focus(true).await;
                    },
                    onclick: move |_| {
                        ctx.modal.set(None);
                        ctx.trash(paths.clone());
                    },
                    "Move to Trash"
                }
            }
        }
    }
}

#[component]
fn SettingsDialog() -> Element {
    let mut ctx = use_context::<Ctx>();
    let settings = ctx.home.read().settings.clone();
    let save = settings.save_folder();
    let name = settings.display_name.clone().unwrap_or_default();
    let placeholder = solar_core::settings::capitalise_login(&std::env::var("USER").unwrap_or_default()).unwrap_or_else(|| "Your name".into());
    rsx! {
        div { class: "w-[520px]",
            div { class: "flex items-center px-6 pt-5 pb-3",
                span { class: "flex-1 text-[17px] font-semibold", "Settings" }
                button { class: "p-1 rounded text-muted hover:text-ink hover:bg-hover", onclick: move |_| ctx.modal.set(None), Icon { svg: icons::X, size: 16 } }
            }
            div { class: "px-6 pb-2 flex flex-col gap-5",
                Field { label: "Theme",
                    div { class: "inline-flex p-1 rounded-lg bg-pill gap-0.5",
                        for pref in ThemePref::ALL {
                            button {
                                key: "{pref.label()}",
                                class: if settings.theme == pref { "px-3 h-7 rounded-md bg-card shadow-[0_1px_2px_rgba(0,0,0,.08)] font-medium" } else { "px-3 h-7 rounded-md text-ink2 hover:text-ink" },
                                onclick: move |_| ctx.home.write().settings.theme = pref,
                                "{pref.label()}"
                            }
                        }
                    }
                }
                Field { label: "Your name", hint: "Used in the greeting on Home.",
                    input {
                        class: "w-full h-9 px-3 rounded-lg border border-line bg-app outline-none focus:border-accent select-text",
                        value: "{name}",
                        placeholder: "{placeholder}",
                        oninput: move |e| {
                            let v = e.value();
                            ctx.home.write().settings.display_name = if v.trim().is_empty() { None } else { Some(v) };
                        },
                    }
                }
                Field { label: "Save new files in", hint: "Also the first folder in the sidebar.",
                    div { class: "flex items-center gap-2",
                        div { class: "flex-1 min-w-0 h-9 px-3 flex items-center rounded-lg border border-line bg-app text-ink2 truncate", title: "{save.display()}", "{save.display()}" }
                        button {
                            class: "px-3 h-9 rounded-lg border border-line hover:bg-hover shrink-0",
                            onclick: move |_| {
                                spawn(async move {
                                    if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Save new files in").pick_folder().await {
                                        ctx.home.write().settings.default_folder = Some(dir.path().to_path_buf());
                                    }
                                });
                            },
                            "Change…"
                        }
                    }
                }
            }
            div { class: "flex items-center px-6 py-4 mt-3 border-t border-line",
                button { class: "text-ink2 hover:text-ink", onclick: move |_| ctx.modal.set(Some(Modal::About)), "About SolarOffice" }
                div { class: "flex-1" }
                button { class: "px-4 h-8 rounded-lg bg-accent text-on-accent font-medium hover:brightness-110", onclick: move |_| ctx.modal.set(None), "Done" }
            }
        }
    }
}

#[component]
fn Field(label: &'static str, #[props(default)] hint: &'static str, children: Element) -> Element {
    rsx! {
        div {
            div { class: "mb-1.5 font-medium", "{label}" }
            {children}
            if !hint.is_empty() {
                div { class: "mt-1 text-[12px] text-muted", "{hint}" }
            }
        }
    }
}

#[component]
fn About() -> Element {
    let mut ctx = use_context::<Ctx>();
    let logo = use_hook(super::sidebar::logo_src);
    rsx! {
        div { class: "w-[400px] p-7 flex flex-col items-center text-center",
            img { src: "{logo}", class: "w-16 h-16 rounded-2xl" }
            div { class: "mt-3 text-[18px] font-semibold", "SolarOffice" }
            div { class: "text-ink2", "Version {solar_core::VERSION}" }
            p { class: "mt-3 text-[12.5px] text-ink2 leading-relaxed",
                "A modern office suite in Rust (Dioxus + TailwindCSS). Home screen and AI after GenOffice; ribbons after ONLYOFFICE; editors based on the ArtCraft Craft apps."
            }
            button { class: "mt-5 px-4 h-8 rounded-lg border border-line hover:bg-hover", onclick: move |_| ctx.modal.set(None), "Close" }
        }
    }
}
