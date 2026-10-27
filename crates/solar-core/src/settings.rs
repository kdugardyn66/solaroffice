//! User settings and where SolarOffice keeps its files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::Library;

/// The interface theme setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub const ALL: [ThemePref; 3] = [ThemePref::System, ThemePref::Light, ThemePref::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemePref::System => "System",
            ThemePref::Light => "Light",
            ThemePref::Dark => "Dark",
        }
    }
}

/// Persisted settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemePref,
    /// The name in the greeting; `None` uses the account's login name.
    pub display_name: Option<String>,
    /// Where new files are saved; the first folder in the sidebar.
    pub default_folder: Option<PathBuf>,
    /// Folders the user added to the sidebar, after the default one.
    pub folders: Vec<PathBuf>,
    /// Sidebar folders whose subfolders are shown.
    pub expanded: Vec<PathBuf>,
    /// The example documents have been copied into `<save folder>/Examples` (done once, so
    /// deleting them keeps them gone).
    pub examples_installed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: ThemePref::System, display_name: None, default_folder: None, folders: Vec::new(), expanded: Vec::new(), examples_installed: false }
    }
}

impl Settings {
    /// The default save folder: the setting, else `~/Documents/SolarOffice`.
    pub fn save_folder(&self) -> PathBuf {
        self.default_folder.clone().unwrap_or_else(default_save_folder)
    }

    /// Every sidebar folder root, the save folder first, without duplicates.
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut out = vec![self.save_folder()];
        for f in &self.folders {
            if !out.contains(f) {
                out.push(f.clone());
            }
        }
        out
    }

    /// Add a sidebar folder; `false` if it was already there.
    pub fn add_folder(&mut self, path: &Path) -> bool {
        if self.roots().iter().any(|r| r == path) {
            return false;
        }
        self.folders.push(path.to_path_buf());
        true
    }

    pub fn remove_folder(&mut self, path: &Path) {
        self.folders.retain(|f| f != path);
        self.expanded.retain(|e| !e.starts_with(path));
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.iter().any(|e| e == path)
    }

    pub fn set_expanded(&mut self, path: &Path, open: bool) {
        self.expanded.retain(|e| e != path);
        if open {
            self.expanded.push(path.to_path_buf());
        }
    }

    /// The greeting name: the setting, else the capitalised login name.
    pub fn greeting_name(&self) -> Option<String> {
        if let Some(n) = self.display_name.as_ref().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            return Some(n.to_string());
        }
        let login = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok()?;
        capitalise_login(&login)
    }
}

/// `duggie` → `Duggie`, `kris.dugardyn` → `Kris`; `None` for system accounts.
pub fn capitalise_login(login: &str) -> Option<String> {
    let first = login.split(['.', '_', '-', ' ']).find(|s| !s.is_empty())?;
    if matches!(first, "root" | "admin" | "user") {
        return None;
    }
    let mut c = first.chars();
    let head = c.next()?;
    Some(head.to_uppercase().chain(c).collect())
}

/// `~/Documents/SolarOffice`.
pub fn default_save_folder() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(".")).join("SolarOffice")
}

/// SolarOffice's settings folder: `$SOLAROFFICE_CONFIG_DIR`, else the platform config folder
/// (`~/Library/Application Support/SolarOffice` on macOS).
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SOLAROFFICE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("SolarOffice")
}

/// Everything the home screen persists, in `home.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeState {
    pub version: u32,
    pub settings: Settings,
    pub library: Library,
}

/// Loads and saves [`HomeState`] in a folder.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store { dir }
    }

    /// The store in [`config_dir`].
    pub fn default_location() -> Store {
        Store::new(config_dir())
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("home.json")
    }

    /// The saved state, or the defaults when there is none or it can't be read (a damaged file
    /// is kept beside as `home.json.bad`).
    pub fn load(&self) -> HomeState {
        let path = self.path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return HomeState { version: 1, ..HomeState::default() };
        };
        match serde_json::from_str::<HomeState>(&text) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("{}: {e}; starting fresh", path.display());
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                HomeState { version: 1, ..HomeState::default() }
            }
        }
    }

    /// Write `state` atomically (temporary file, then rename).
    pub fn save(&self, state: &HomeState) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let json = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
        let tmp = self.dir.join(format!(".home.json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, self.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_names() {
        assert_eq!(capitalise_login("duggie").as_deref(), Some("Duggie"));
        assert_eq!(capitalise_login("kris.dugardyn").as_deref(), Some("Kris"));
        assert_eq!(capitalise_login("root"), None);
        assert_eq!(capitalise_login(""), None);
    }

    #[test]
    fn roots_put_save_folder_first_without_duplicates() {
        let mut s = Settings { default_folder: Some("/a".into()), ..Settings::default() };
        assert!(s.add_folder(Path::new("/b")));
        assert!(!s.add_folder(Path::new("/a")));
        assert!(!s.add_folder(Path::new("/b")));
        assert_eq!(s.roots(), [PathBuf::from("/a"), PathBuf::from("/b")]);
        s.set_expanded(Path::new("/b/x"), true);
        s.remove_folder(Path::new("/b"));
        assert_eq!(s.roots(), [PathBuf::from("/a")]);
        assert!(s.expanded.is_empty());
    }

    #[test]
    fn store_round_trip_and_bad_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        assert_eq!(store.load().settings, Settings::default());
        let mut st = HomeState::default();
        st.settings.theme = ThemePref::Dark;
        st.library.touch(Path::new("/x/a.md"), 5);
        store.save(&st).unwrap();
        assert_eq!(store.load(), st);
        std::fs::write(store.path(), "{ not json").unwrap();
        assert_eq!(store.load().settings.theme, ThemePref::System);
        assert!(dir.path().join("home.json.bad").exists());
    }
}
