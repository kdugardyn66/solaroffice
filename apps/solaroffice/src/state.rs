//! The app's shared state: the persisted home state plus tabs, the current page, menus, dialogs
//! and the status toast. Every field is a signal, so [`Ctx`] is `Copy` and any component takes it
//! with `use_context::<Ctx>()`.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use solar_core::library::Sort;
use solar_core::settings::{HomeState, Store, ThemePref};
use solar_core::{AppKind, now_ms};

/// The most recently closed tabs ⌘⇧T can reopen.
pub const MAX_CLOSED: usize = 25;

/// What the home tab shows on the right of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Recent,
    Starred,
    Folder(PathBuf),
    AskAi,
}

/// A module tab. Until the editors arrive it shows the file's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u64,
    pub kind: Option<AppKind>,
    pub title: String,
    /// `None` for a new file that hasn't been saved yet.
    pub path: Option<PathBuf>,
    /// The folder a new file will be saved in.
    pub folder: PathBuf,
}

/// A pop-up menu, anchored at a window position.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuKind {
    /// The tab strip's + menu (also ⌘T).
    NewFile,
    /// A file row's ⋯ menu.
    Row(PathBuf),
    /// A sidebar folder's ⋯ menu.
    Folder(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub kind: MenuKind,
    pub x: f64,
    pub y: f64,
}

/// A modal dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    ConfirmTrash(Vec<PathBuf>),
    Settings,
    About,
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub home: Signal<HomeState>,
    pub store: Signal<Store>,
    pub tabs: Signal<Vec<Tab>>,
    /// The active tab's id; `None` is Home.
    pub active: Signal<Option<u64>>,
    pub closed: Signal<Vec<Tab>>,
    pub next_id: Signal<u64>,
    pub page: Signal<Page>,
    pub filter: Signal<Option<AppKind>>,
    pub search: Signal<String>,
    pub sort: Signal<Sort>,
    pub selected: Signal<Vec<PathBuf>>,
    /// The row being renamed and the text in its box.
    pub renaming: Signal<Option<(PathBuf, String)>>,
    pub menu: Signal<Option<Menu>>,
    pub modal: Signal<Option<Modal>>,
    pub toast: Signal<Option<(u64, String)>>,
    /// The OS is in dark mode (read at start; used when the theme follows the system).
    pub system_dark: Signal<bool>,
    /// Bumped to re-read file metadata and folder listings.
    pub refresh: Signal<u64>,
    /// The search box, for ⌘F.
    pub search_box: Signal<Option<std::rc::Rc<MountedData>>>,
}

impl Ctx {
    pub fn new(state: HomeState, store: Store, system_dark: bool) -> Ctx {
        Ctx {
            home: Signal::new(state),
            store: Signal::new(store),
            tabs: Signal::new(Vec::new()),
            active: Signal::new(None),
            closed: Signal::new(Vec::new()),
            next_id: Signal::new(1),
            page: Signal::new(Page::Recent),
            filter: Signal::new(None),
            search: Signal::new(String::new()),
            sort: Signal::new(Sort::default()),
            selected: Signal::new(Vec::new()),
            renaming: Signal::new(None),
            menu: Signal::new(None),
            modal: Signal::new(None),
            toast: Signal::new(None),
            system_dark: Signal::new(system_dark),
            refresh: Signal::new(0),
            search_box: Signal::new(None),
        }
    }

    pub fn dark(&self) -> bool {
        match self.home.read().settings.theme {
            ThemePref::System => *self.system_dark.read(),
            ThemePref::Light => false,
            ThemePref::Dark => true,
        }
    }

    /// The titlebar switch: flip between light and dark (an explicit choice from then on).
    pub fn toggle_theme(&mut self) {
        let dark = self.dark();
        self.home.write().settings.theme = if dark { ThemePref::Light } else { ThemePref::Dark };
    }

    /// Show `text` in the status toast for a few seconds.
    pub fn toast(&mut self, text: impl Into<String>) {
        let id = now_ms() as u64;
        self.toast.set(Some((id, text.into())));
        let mut toast = self.toast;
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
            if toast.peek().as_ref().is_some_and(|(t, _)| *t == id) {
                toast.set(None);
            }
        });
    }

    pub fn close_overlays(&mut self) {
        self.menu.set(None);
        self.modal.set(None);
    }

    fn alloc_id(&mut self) -> u64 {
        let id = *self.next_id.peek();
        self.next_id.set(id + 1);
        id
    }

    /// The folder new files go to: the folder shown in the sidebar, else the save folder.
    pub fn target_folder(&self) -> PathBuf {
        match &*self.page.peek() {
            Page::Folder(p) if p.is_dir() => p.clone(),
            _ => self.home.peek().settings.save_folder(),
        }
    }

    /// A new, unsaved file of `kind` in a new tab (written on first save, as in GenOffice).
    pub fn new_file(&mut self, kind: AppKind) {
        let folder = self.target_folder();
        let n = self.tabs.peek().iter().filter(|t| t.path.is_none() && t.kind == Some(kind)).count();
        let mut title = kind.untitled();
        if n > 0 {
            let (s, e) = title.rsplit_once('.').map(|(s, e)| (s.to_string(), e.to_string())).unwrap_or_default();
            title = format!("{s} {}.{e}", n + 1);
        }
        let id = self.alloc_id();
        self.tabs.write().push(Tab { id, kind: Some(kind), title, path: None, folder });
        self.active.set(Some(id));
        self.menu.set(None);
    }

    /// Open `path` in a tab (or switch to the tab that has it) and record it in Recent.
    pub fn open_path(&mut self, path: &Path) {
        self.menu.set(None);
        if !path.exists() {
            self.toast(format!("“{}” can't be found. It may have been moved or deleted.", solar_core::library::display_name(path)));
            return;
        }
        self.home.write().library.touch(path, now_ms());
        if let Some(t) = self.tabs.peek().iter().find(|t| t.path.as_deref() == Some(path)) {
            self.active.set(Some(t.id));
            return;
        }
        let id = self.alloc_id();
        let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let tab = Tab { id, kind: AppKind::for_path(path), title: solar_core::library::display_name(path), path: Some(path.to_path_buf()), folder };
        self.tabs.write().push(tab);
        self.active.set(Some(id));
    }

    /// The system Open dialog; every picked file gets its own tab.
    pub fn open_dialog(&mut self) {
        self.menu.set(None);
        let mut ctx = *self;
        let start = self.target_folder();
        spawn(async move {
            let exts = solar_core::apps::all_extensions();
            let mut dialog = rfd::AsyncFileDialog::new().set_title("Open").add_filter("SolarOffice documents", &exts);
            for k in AppKind::ALL {
                dialog = dialog.add_filter(k.name(), k.extensions());
            }
            if start.is_dir() {
                dialog = dialog.set_directory(&start);
            }
            if let Some(files) = dialog.pick_files().await {
                for f in files {
                    ctx.open_path(f.path());
                }
            }
        });
    }

    pub fn close_tab(&mut self, id: u64) {
        let mut tabs = self.tabs.write();
        let Some(i) = tabs.iter().position(|t| t.id == id) else { return };
        let tab = tabs.remove(i);
        let was_active = *self.active.peek() == Some(id);
        if was_active {
            // The tab to the left, like a browser; Home when it was the first.
            let next = if i > 0 { tabs.get(i - 1).map(|t| t.id) } else { tabs.first().map(|t| t.id) };
            self.active.set(next);
        }
        drop(tabs);
        let mut closed = self.closed.write();
        closed.push(tab);
        let excess = closed.len().saturating_sub(MAX_CLOSED);
        closed.drain(..excess);
    }

    pub fn reopen_closed(&mut self) {
        let Some(tab) = self.closed.write().pop() else { return };
        match tab.path.clone() {
            Some(p) => self.open_path(&p),
            None => {
                let id = self.alloc_id();
                self.tabs.write().push(Tab { id, ..tab });
                self.active.set(Some(id));
            }
        }
    }

    /// Activate tab `index` in strip order: 0 is Home; past the end selects the last tab.
    pub fn select_index(&mut self, index: usize) {
        let tabs = self.tabs.peek();
        let id = if index == 0 { None } else { tabs.get(index - 1).or(tabs.last()).map(|t| t.id) };
        drop(tabs);
        self.active.set(id);
    }

    /// Next (`+1`) or previous (`-1`) tab, wrapping through Home.
    pub fn cycle(&mut self, step: i32) {
        let tabs = self.tabs.peek();
        let n = tabs.len() as i32 + 1;
        let cur = match *self.active.peek() {
            None => 0,
            Some(id) => tabs.iter().position(|t| t.id == id).map(|i| i as i32 + 1).unwrap_or(0),
        };
        drop(tabs);
        self.select_index((cur + step).rem_euclid(n) as usize);
    }

    pub fn go(&mut self, page: Page) {
        self.page.set(page);
        self.active.set(None);
        self.selected.write().clear();
        self.renaming.set(None);
    }

    /// The paths were renamed or moved: follow them in the lists and open tabs.
    pub fn moved(&mut self, from: &Path, to: &Path) {
        self.home.write().library.rename(from, to);
        for t in self.tabs.write().iter_mut() {
            if t.path.as_deref() == Some(from) {
                t.path = Some(to.to_path_buf());
                t.title = solar_core::library::display_name(to);
            }
        }
        self.refresh += 1;
    }

    /// Move `paths` to the Trash and forget them.
    pub fn trash(&mut self, paths: Vec<PathBuf>) {
        let existing: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
        match crate::files::move_to_trash(&existing) {
            Ok(()) => {
                self.home.write().library.forget(&paths);
                self.tabs.write().retain(|t| t.path.as_ref().is_none_or(|p| !paths.contains(p)));
                let active = *self.active.peek();
                if active.is_some_and(|id| !self.tabs.peek().iter().any(|t| t.id == id)) {
                    self.active.set(None);
                }
                self.selected.write().retain(|p| !paths.contains(p));
                let n = paths.len();
                self.toast(if n == 1 { "Moved 1 file to the Trash.".to_string() } else { format!("Moved {n} files to the Trash.") });
            }
            Err(e) => self.toast(format!("Couldn't move to the Trash: {e}")),
        }
        self.refresh += 1;
    }

    /// Pick a folder and add it to the sidebar.
    pub fn add_folder_dialog(&mut self) {
        let mut ctx = *self;
        spawn(async move {
            if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Add folder to sidebar").pick_folder().await {
                let path = dir.path().to_path_buf();
                if !ctx.home.write().settings.add_folder(&path) {
                    ctx.toast("That folder is already in the sidebar.");
                }
                ctx.go(Page::Folder(path));
            }
        });
    }
}
