//! The home page: greeting, the module cards, and the file list with its filters, search, sort,
//! selection and row actions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use solar_core::library::{FileMeta, Query, Row, SortKey, View, display_name, sort_rows};
use solar_core::{AppKind, fmt, folders, now_ms};

use crate::icons::{self, Badge, Icon};
use crate::state::{Ctx, Menu, MenuKind, Modal, Page};
use crate::ui::primary;

/// What the list shows: the rows, plus subfolders in folder view.
struct Listing {
    folders: Vec<PathBuf>,
    rows: Vec<Row>,
    /// The folder couldn't be read.
    error: Option<String>,
}

fn listing(ctx: &Ctx) -> Listing {
    let _ = ctx.refresh.read();
    let page = ctx.page.read().clone();
    let filter = *ctx.filter.read();
    let search = ctx.search.read().clone();
    let sort = *ctx.sort.read();
    let home = ctx.home.read();
    match page {
        Page::Folder(dir) => {
            let l = folders::list(&dir);
            let terms = solar_core::library::search_terms(&search);
            let matches = |name: &str| {
                let lower = name.to_lowercase();
                terms.iter().all(|t| lower.contains(t.as_str()))
            };
            let mut rows: Vec<Row> = l
                .files
                .into_iter()
                .filter(|(p, _)| filter.is_none() || AppKind::for_path(p) == filter)
                .map(|(path, meta)| Row {
                    name: display_name(&path),
                    kind: AppKind::for_path(&path),
                    activity_ms: meta.modified_ms.unwrap_or(0),
                    starred: home.library.is_starred(&path),
                    meta,
                    path,
                })
                .filter(|r| matches(&r.name))
                .collect();
            sort_rows(&mut rows, sort);
            let folders = if filter.is_none() { l.folders.into_iter().filter(|f| matches(&display_name(f))).collect() } else { Vec::new() };
            Listing { folders, rows, error: l.error }
        }
        page => {
            let view = if page == Page::Starred { View::Starred } else { View::Recent };
            let paths: Vec<PathBuf> = match view {
                View::Recent => home.library.recents.iter().map(|r| r.path.clone()).collect(),
                View::Starred => home.library.starred.iter().map(|s| s.path.clone()).collect(),
            };
            let meta: HashMap<PathBuf, FileMeta> = paths.into_iter().map(|p| (p.clone(), FileMeta::read(&p))).collect();
            let q = Query { view, filter, search, sort };
            Listing { folders: Vec::new(), rows: home.library.rows(&q, &meta), error: None }
        }
    }
}

#[component]
pub fn HomePage() -> Element {
    let ctx = use_context::<Ctx>();
    let page = ctx.page.read().clone();
    let greeting = use_hook(|| {
        use chrono::Timelike;
        let hour = chrono::Local::now().hour();
        let ask = fmt::ASK_LINES[(now_ms() / 1000) as usize % fmt::ASK_LINES.len()];
        (hour, ask)
    });
    let name = ctx.home.read().settings.greeting_name();
    let hello = fmt::greeting(greeting.0, name.as_deref());

    let heading = match &page {
        Page::Starred => "Starred".to_string(),
        Page::Folder(p) => display_name(p),
        _ => "Recent".to_string(),
    };

    rsx! {
        div { class: "max-w-[1240px] mx-auto px-10 pt-9 pb-16",
            if !matches!(page, Page::Folder(_)) {
                h1 { class: "text-[26px] font-semibold tracking-tight", "{hello}" }
                p { class: "mt-1 text-[15px] text-ink2", "{greeting.1}" }
            }
            QuickStart {}
            div { class: "mt-9 flex items-center gap-3",
                h2 { class: "text-[15px] font-semibold", "{heading}" }
                if let Page::Folder(p) = &page {
                    span { class: "text-[12px] text-muted truncate", "{p.display()}" }
                }
            }
            FileList {}
        }
    }
}

/// The module cards and Open local file.
#[component]
fn QuickStart() -> Element {
    let mut ctx = use_context::<Ctx>();
    rsx! {
        div { class: "@container mt-7",
            div { class: "grid grid-cols-4 @[66rem]:grid-cols-8 gap-3",
                for kind in AppKind::ALL {
                    button {
                        key: "{kind.id()}",
                        class: "group text-left p-3.5 rounded-xl border border-line bg-card hover:border-[color:var(--so-line-strong)] hover:shadow-[0_4px_14px_-6px_rgba(0,0,0,.18)] transition-[box-shadow,border-color]",
                        title: "{kind.new_label()} — {kind.name()} (⌘⇧{kind_index(kind)})",
                        onclick: move |_| ctx.new_file(kind),
                        div { class: "flex items-start justify-between",
                            Badge { kind: Some(kind), size: 34 }
                            span { class: "px-1.5 rounded text-[10.5px] font-semibold leading-[17px] bg-accent-soft text-accent", "AI" }
                        }
                        div { class: "mt-3 font-semibold text-[13.5px] truncate", "{kind.name()}" }
                        div { class: "mt-0.5 text-[12px] text-muted truncate", "{kind.new_label()} · .{kind.default_ext()}" }
                    }
                }
                button {
                    class: "text-left p-3.5 rounded-xl border border-dashed border-[color:var(--so-line-strong)] hover:bg-card",
                    title: "Open local file (⌘O)",
                    onclick: move |_| ctx.open_dialog(),
                    span { class: "inline-flex items-center justify-center w-[34px] h-[34px] rounded-[8px] border border-line text-ink2",
                        Icon { svg: icons::FOLDER_OPEN, size: 18 }
                    }
                    div { class: "mt-3 font-semibold text-[13.5px] truncate", "Open local file" }
                    div { class: "mt-0.5 text-[12px] text-muted truncate", "Word, Excel, PowerPoint, PDF, …" }
                }
            }
        }
    }
}

fn kind_index(kind: AppKind) -> usize {
    AppKind::ALL.iter().position(|k| *k == kind).unwrap_or(0) + 1
}

#[component]
fn FileList() -> Element {
    let mut ctx = use_context::<Ctx>();
    let page = ctx.page.read().clone();
    let list = listing(&ctx);
    let filter = *ctx.filter.read();
    let search = ctx.search.read().clone();
    let sort = *ctx.sort.read();
    let selected = ctx.selected.read().clone();
    let in_folder = matches!(page, Page::Folder(_));
    let activity_label = match page {
        Page::Starred => "Starred",
        Page::Folder(_) => "",
        _ => "Opened",
    };
    let cols = if in_folder { "28px minmax(0,1fr) 120px 80px 64px" } else { "28px minmax(0,1fr) 150px 120px 120px 80px 64px" };
    let count = list.rows.len() + list.folders.len();
    let all_selected = !list.rows.is_empty() && list.rows.iter().all(|r| selected.contains(&r.path));
    let row_paths: Vec<PathBuf> = list.rows.iter().map(|r| r.path.clone()).collect();

    let header = move |label: &'static str, key: SortKey| {
        let on = sort.key == key;
        rsx! {
            button {
                class: if on { "flex items-center gap-1 text-ink" } else { "flex items-center gap-1 hover:text-ink" },
                onclick: move |_| {
                    let s = ctx.sort.peek().clicked(key);
                    ctx.sort.set(s);
                },
                "{label}"
                if on {
                    Icon { svg: if sort.ascending { icons::ARROW_UP } else { icons::ARROW_DOWN }, size: 12 }
                }
            }
        }
    };

    rsx! {
        // Filter pills, search, count.
        div { class: "mt-3 flex items-center gap-3 flex-wrap",
            div { class: "flex items-center p-1 rounded-lg bg-pill gap-0.5",
                FilterPill { label: "All", on: filter.is_none(), onclick: move |_| ctx.filter.set(None) }
                for kind in AppKind::ALL {
                    FilterPill { key: "{kind.id()}", label: kind.filter_label(), on: filter == Some(kind), onclick: move |_| ctx.filter.set(Some(kind)) }
                }
            }
            div { class: "flex-1 min-w-[220px] flex items-center gap-2 h-9 px-3 rounded-lg border border-line bg-card focus-within:border-accent",
                Icon { svg: icons::SEARCH, size: 15, class: "text-muted" }
                input {
                    class: "flex-1 bg-transparent outline-none placeholder:text-muted select-text",
                    placeholder: "Search file names…  (⌘F)",
                    value: "{search}",
                    spellcheck: "false",
                    onmounted: move |e| ctx.search_box.set(Some(e.data())),
                    oninput: move |e| ctx.search.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Escape && !ctx.search.peek().is_empty() {
                            e.stop_propagation();
                            ctx.search.set(String::new());
                        }
                    },
                }
                if !search.is_empty() {
                    button { class: "text-muted hover:text-ink", onclick: move |_| ctx.search.set(String::new()),
                        Icon { svg: icons::X, size: 14 }
                    }
                }
            }
            button {
                class: "p-2 rounded-md text-ink2 hover:bg-hover",
                title: "Refresh",
                onclick: move |_| ctx.refresh += 1,
                Icon { svg: icons::REFRESH, size: 15 }
            }
            span { class: "text-[12px] text-muted tabular-nums",
                if count == 1 { "1 item" } else { "{count} items" }
            }
        }

        // Selection bar.
        if !selected.is_empty() {
            SelectionBar { paths: selected.clone(), in_recent: page == Page::Recent }
        }

        div { class: "mt-3 rounded-xl border border-line bg-card overflow-hidden",
            // Header
            div {
                class: "grid items-center gap-3 px-4 h-10 border-b border-line text-[12px] font-medium text-muted",
                style: "grid-template-columns:{cols}",
                input {
                    r#type: "checkbox",
                    class: "accent-[var(--so-accent)]",
                    checked: all_selected,
                    disabled: list.rows.is_empty(),
                    onclick: move |_| {
                        if all_selected {
                            ctx.selected.write().clear();
                        } else {
                            ctx.selected.set(row_paths.clone());
                        }
                    },
                }
                {header("Name", SortKey::Name)}
                if !in_folder {
                    span { "Location" }
                    {header(activity_label, SortKey::Activity)}
                }
                {header("Modified", SortKey::Modified)}
                div { class: "flex justify-end", {header("Size", SortKey::Size)} }
                span {}
            }

            if let Some(e) = &list.error {
                Empty { icon: icons::ALERT, title: "This folder can't be opened.", detail: e.clone() }
            } else if count == 0 {
                if !search.is_empty() {
                    Empty { icon: icons::SEARCH, title: format!("No files match “{search}”."), detail: "Try fewer words, or another type filter.".to_string() }
                } else {
                    match page {
                        Page::Starred => rsx! { Empty { icon: icons::STAR, title: "No starred files yet.", detail: "Hover a file and click its star to keep it here.".to_string() } },
                        Page::Folder(_) => rsx! { Empty { icon: icons::FOLDER, title: "No SolarOffice files in this folder.", detail: "New files you create here are saved in this folder.".to_string() } },
                        _ => rsx! { Empty { icon: icons::CLOCK, title: "Files you open appear here.", detail: "Create one with a card above, or open a local file.".to_string() } },
                    }
                }
            }

            for f in list.folders.clone() {
                FolderRow { key: "dir:{f.display()}", path: f, cols: cols.to_string() }
            }
            for row in list.rows.clone() {
                FileRow {
                    key: "{row.path.display()}",
                    row: row.clone(),
                    cols: cols.to_string(),
                    show_activity: !in_folder,
                    selected: selected.contains(&row.path),
                }
            }
        }
    }
}

#[component]
fn FilterPill(label: &'static str, on: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let look = if on { "bg-card text-ink shadow-[0_1px_2px_rgba(0,0,0,.08)] font-medium" } else { "text-ink2 hover:text-ink" };
    rsx! {
        button { class: "px-3 h-7 rounded-md text-[13px] {look}", onclick: move |e| onclick.call(e), "{label}" }
    }
}

#[component]
fn Empty(icon: &'static str, title: String, detail: String) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center py-16 text-center",
            div { class: "w-11 h-11 rounded-full bg-hover flex items-center justify-center text-muted",
                Icon { svg: icon, size: 20 }
            }
            div { class: "mt-3 font-medium", "{title}" }
            div { class: "mt-1 text-[12px] text-muted", "{detail}" }
        }
    }
}

#[component]
fn SelectionBar(paths: Vec<PathBuf>, in_recent: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    let n = paths.len();
    let p_move = paths.clone();
    let p_trash = paths.clone();
    let p_remove = paths.clone();
    rsx! {
        div { class: "mt-3 flex items-center gap-2 px-3 h-10 rounded-lg bg-accent-soft text-[13px]",
            span { class: "font-medium pr-2", if n == 1 { "1 selected" } else { "{n} selected" } }
            BarButton { icon: icons::FOLDER_OPEN, label: "Move to folder…",
                onclick: move |_| {
                    let paths = p_move.clone();
                    spawn(async move {
                        if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Move to folder").pick_folder().await {
                            let mut moved = 0;
                            for p in &paths {
                                match crate::files::move_into(p, dir.path()) {
                                    Ok(to) => {
                                        ctx.moved(p, &to);
                                        moved += 1;
                                    }
                                    Err(e) => ctx.toast(format!("Couldn't move “{}”: {e}", display_name(p))),
                                }
                            }
                            ctx.selected.write().clear();
                            if moved > 0 {
                                ctx.toast(format!("Moved {moved} file{} to {}.", if moved == 1 { "" } else { "s" }, display_name(dir.path())));
                            }
                        }
                    });
                },
            }
            BarButton { icon: icons::TRASH, label: "Delete files", onclick: move |_| ctx.modal.set(Some(Modal::ConfirmTrash(p_trash.clone()))) }
            if in_recent {
                BarButton { icon: icons::MINUS_CIRCLE, label: "Remove from list",
                    onclick: move |_| {
                        ctx.home.write().library.remove_recents(&p_remove);
                        ctx.selected.write().clear();
                    },
                }
            }
            div { class: "flex-1" }
            button { class: "px-2 py-1 rounded-md text-ink2 hover:bg-hover", onclick: move |_| ctx.selected.write().clear(), "Clear selection" }
        }
    }
}

#[component]
fn BarButton(icon: &'static str, label: &'static str, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button { class: "flex items-center gap-1.5 px-2 py-1 rounded-md hover:bg-hover", onclick: move |e| onclick.call(e),
            Icon { svg: icon, size: 14 }
            "{label}"
        }
    }
}

#[component]
fn FolderRow(path: PathBuf, cols: String) -> Element {
    let mut ctx = use_context::<Ctx>();
    let name = display_name(&path);
    let p = path.clone();
    rsx! {
        div {
            class: "grid items-center gap-3 px-4 h-[46px] border-b border-line last:border-b-0 hover:bg-hover cursor-default",
            style: "grid-template-columns:{cols}",
            ondoubleclick: move |_| ctx.go(Page::Folder(p.clone())),
            onclick: move |_| ctx.go(Page::Folder(path.clone())),
            span {}
            div { class: "flex items-center gap-3 min-w-0",
                span { class: "w-6 h-6 flex items-center justify-center text-ink2", Icon { svg: icons::FOLDER, size: 20 } }
                span { class: "truncate", "{name}" }
            }
        }
    }
}

#[component]
fn FileRow(row: Row, cols: String, show_activity: bool, selected: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    let renaming = ctx.renaming.read().clone().filter(|(p, _)| *p == row.path);
    let missing = !row.meta.exists;
    let location = row.path.parent().map(display_name).unwrap_or_default();
    let modified = row.meta.modified_ms.map(fmt::when_local).unwrap_or_else(|| "—".into());
    let size = if row.meta.exists { fmt::size(row.meta.size) } else { "—".into() };
    let activity = fmt::when_local(row.activity_ms);
    let look = if selected { "bg-sel" } else { "hover:bg-hover" };
    let dim = if missing { "opacity-55" } else { "" };

    let path = row.path.clone();
    let p_open = path.clone();
    let p_check = path.clone();
    let p_star = path.clone();
    let p_menu = path.clone();
    let starred = row.starred;

    rsx! {
        div {
            class: "group grid items-center gap-3 px-4 h-[46px] border-b border-line last:border-b-0 cursor-default {look}",
            style: "grid-template-columns:{cols}",
            title: "{path.display()}",
            onclick: move |e| {
                if primary(e.modifiers()) {
                    toggle(&mut ctx, &p_open);
                } else if ctx.renaming.peek().is_none() {
                    ctx.open_path(&p_open);
                }
            },
            oncontextmenu: move |e| {
                e.prevent_default();
                let c = e.client_coordinates();
                ctx.menu.set(Some(Menu { kind: MenuKind::Row(p_menu.clone()), x: c.x, y: c.y }));
            },
            input {
                r#type: "checkbox",
                class: "accent-[var(--so-accent)]",
                checked: selected,
                onclick: move |e| {
                    e.stop_propagation();
                    toggle(&mut ctx, &p_check);
                },
            }
            div { class: "flex items-center gap-3 min-w-0 {dim}",
                Badge { kind: row.kind, size: 26 }
                if let Some((_, text)) = renaming {
                    RenameBox { path: path.clone(), text }
                } else {
                    span { class: "truncate text-[13.5px]", "{row.name}" }
                    if missing {
                        span { class: "shrink-0 px-1.5 rounded text-[11px] leading-[18px] bg-hover text-muted", "Missing" }
                    }
                }
            }
            if show_activity {
                span { class: "truncate text-ink2 {dim}", "{location}" }
                span { class: "truncate text-ink2 tabular-nums {dim}", "{activity}" }
            }
            span { class: "truncate text-ink2 tabular-nums {dim}", "{modified}" }
            span { class: "text-right text-ink2 tabular-nums {dim}", "{size}" }
            div { class: "flex items-center justify-end gap-0.5",
                button {
                    class: if starred { "p-1 rounded text-accent hover:bg-hover" } else { "p-1 rounded text-muted hover:text-ink hover:bg-hover opacity-0 group-hover:opacity-100" },
                    title: if starred { "Unstar" } else { "Star" },
                    onclick: move |e| {
                        e.stop_propagation();
                        ctx.home.write().library.set_starred(&p_star, !starred, now_ms());
                    },
                    Icon { svg: icons::STAR, size: 15, filled: starred }
                }
                button {
                    class: "p-1 rounded text-muted hover:text-ink hover:bg-hover",
                    title: "More",
                    onclick: move |e| {
                        e.stop_propagation();
                        let c = e.client_coordinates();
                        ctx.menu.set(Some(Menu { kind: MenuKind::Row(path.clone()), x: c.x, y: c.y + 10.0 }));
                    },
                    Icon { svg: icons::MORE, size: 16 }
                }
            }
        }
    }
}

fn toggle(ctx: &mut Ctx, path: &Path) {
    let mut sel = ctx.selected.write();
    if let Some(i) = sel.iter().position(|p| p == path) {
        sel.remove(i);
    } else {
        sel.push(path.to_path_buf());
    }
}

/// The inline rename box: the name without its extension; Enter renames, Escape cancels.
#[component]
fn RenameBox(path: PathBuf, text: String) -> Element {
    let mut ctx = use_context::<Ctx>();
    let ext = crate::files::full_extension(&path);
    let mut commit = move |path: PathBuf| {
        let Some((_, text)) = ctx.renaming.peek().clone() else { return };
        ctx.renaming.set(None);
        match crate::files::rename_keep_ext(&path, &text) {
            Ok(to) if to != path => ctx.moved(&path, &to),
            Ok(_) => {}
            Err(e) => ctx.toast(e.to_string()),
        }
    };
    let p_key = path.clone();
    let p_blur = path.clone();
    rsx! {
        div { class: "flex items-center gap-1 flex-1 min-w-0",
            input {
                class: "flex-1 min-w-0 px-1.5 h-7 rounded border border-accent bg-app outline-none select-text",
                value: "{text}",
                spellcheck: "false",
                onclick: |e| e.stop_propagation(),
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                oninput: move |e| {
                    if let Some((_, t)) = ctx.renaming.write().as_mut() {
                        *t = e.value();
                    }
                },
                onkeydown: move |e| {
                    match e.key() {
                        Key::Enter => commit(p_key.clone()),
                        Key::Escape => {
                            e.stop_propagation();
                            ctx.renaming.set(None);
                        }
                        _ => {}
                    }
                },
                onblur: move |_| commit(p_blur.clone()),
            }
            span { class: "text-muted", "{ext}" }
        }
    }
}
