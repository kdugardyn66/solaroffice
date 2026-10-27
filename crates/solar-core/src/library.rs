//! Recent and starred files, and the queries the home screen's file list runs over them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::apps::AppKind;

/// The most recent files kept; older ones drop off the list.
pub const MAX_RECENTS: usize = 500;

/// A file the user opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent {
    pub path: PathBuf,
    /// When it was last opened (Unix milliseconds).
    pub opened_ms: i64,
}

/// A file the user starred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Starred {
    pub path: PathBuf,
    pub starred_ms: i64,
}

/// What the file system says about a listed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileMeta {
    pub exists: bool,
    pub size: u64,
    /// Last modified (Unix milliseconds), when known.
    pub modified_ms: Option<i64>,
}

impl FileMeta {
    pub fn read(path: &Path) -> FileMeta {
        match std::fs::metadata(path) {
            Ok(m) if m.is_file() => FileMeta {
                exists: true,
                size: m.len(),
                modified_ms: m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64),
            },
            _ => FileMeta::default(),
        }
    }
}

/// The recent and starred lists (persisted).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub recents: Vec<Recent>,
    pub starred: Vec<Starred>,
}

/// Which list the home screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Recent,
    Starred,
}

/// The column the file list is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// Recent: when opened. Starred: when starred.
    #[default]
    Activity,
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sort {
    pub key: SortKey,
    /// Ascending (oldest / A→Z / smallest first).
    pub ascending: bool,
}

impl Sort {
    /// Clicking a column header: the same column flips the direction, a new one starts at its
    /// natural direction (names A→Z, everything else newest/largest first).
    pub fn clicked(self, key: SortKey) -> Sort {
        if self.key == key {
            Sort { key, ascending: !self.ascending }
        } else {
            Sort { key, ascending: key == SortKey::Name }
        }
    }
}

/// A list query: view, type filter, search text and sort.
#[derive(Debug, Clone, Default)]
pub struct Query {
    pub view: View,
    /// `None` shows every type.
    pub filter: Option<AppKind>,
    pub search: String,
    pub sort: Sort,
}

/// One row of the file list.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub kind: Option<AppKind>,
    pub activity_ms: i64,
    pub starred: bool,
    pub meta: FileMeta,
}

/// The file name shown for `path`.
pub fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// The search words of `search` (lower case); a row matches when its name has all of them.
pub fn search_terms(search: &str) -> Vec<String> {
    search.split_whitespace().map(|w| w.to_lowercase()).collect()
}

impl Library {
    pub fn is_starred(&self, path: &Path) -> bool {
        self.starred.iter().any(|s| s.path == path)
    }

    /// Record that `path` was opened now (moves it to the top).
    pub fn touch(&mut self, path: &Path, now_ms: i64) {
        self.recents.retain(|r| r.path != path);
        self.recents.insert(0, Recent { path: path.to_path_buf(), opened_ms: now_ms });
        self.recents.truncate(MAX_RECENTS);
    }

    pub fn set_starred(&mut self, path: &Path, on: bool, now_ms: i64) {
        let had = self.is_starred(path);
        if on && !had {
            self.starred.insert(0, Starred { path: path.to_path_buf(), starred_ms: now_ms });
        } else if !on && had {
            self.starred.retain(|s| s.path != path);
        }
    }

    /// Take `paths` off the recent list (the files stay on disk).
    pub fn remove_recents(&mut self, paths: &[PathBuf]) {
        self.recents.retain(|r| !paths.contains(&r.path));
    }

    /// Forget `paths` everywhere (after they were deleted).
    pub fn forget(&mut self, paths: &[PathBuf]) {
        self.remove_recents(paths);
        self.starred.retain(|s| !paths.contains(&s.path));
    }

    /// A file was renamed or moved: follow it in both lists.
    pub fn rename(&mut self, from: &Path, to: &Path) {
        for r in &mut self.recents {
            if r.path == from {
                r.path = to.to_path_buf();
            }
        }
        for s in &mut self.starred {
            if s.path == from {
                s.path = to.to_path_buf();
            }
        }
    }

    /// Repair entries that point at a stale copy of a file: one that is gone, or one inside the
    /// application bundle. If the save folder holds a file of the same name, the entry follows it
    /// (or is dropped when that file is already listed). Returns whether anything changed.
    pub fn relocate_stale(&mut self, save: &Path) -> bool {
        let stale = |p: &Path| !p.exists() || p.components().any(|c| c.as_os_str().to_string_lossy().ends_with(".app"));
        let target = |p: &Path| -> Option<PathBuf> {
            let t = save.join(p.file_name()?);
            (t != p && t.is_file() && stale(p)).then_some(t)
        };
        let mut changed = false;
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut recents = Vec::new();
        // Newest first, so the most recent opening of a file wins.
        let mut old = std::mem::take(&mut self.recents);
        old.sort_by_key(|r| std::cmp::Reverse(r.opened_ms));
        for mut r in old {
            if let Some(t) = target(&r.path) {
                r.path = t;
                changed = true;
            }
            if seen.contains(&r.path) {
                changed = true;
                continue;
            }
            seen.push(r.path.clone());
            recents.push(r);
        }
        self.recents = recents;
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut starred = Vec::new();
        for mut s in std::mem::take(&mut self.starred) {
            if let Some(t) = target(&s.path) {
                s.path = t;
                changed = true;
            }
            if seen.contains(&s.path) {
                changed = true;
                continue;
            }
            seen.push(s.path.clone());
            starred.push(s);
        }
        self.starred = starred;
        changed
    }

    /// The number of entries per view under `filter`, for the sidebar counts.
    pub fn counts(&self, filter: Option<AppKind>) -> (usize, usize) {
        let ok = |p: &Path| filter.is_none() || AppKind::for_path(p) == filter;
        (self.recents.iter().filter(|r| ok(&r.path)).count(), self.starred.iter().filter(|s| ok(&s.path)).count())
    }

    /// The rows for `q`, with file metadata from `meta` (missing entries count as unknown).
    pub fn rows(&self, q: &Query, meta: &HashMap<PathBuf, FileMeta>) -> Vec<Row> {
        let terms = search_terms(&q.search);
        let entries: Vec<(&PathBuf, i64)> = match q.view {
            View::Recent => self.recents.iter().map(|r| (&r.path, r.opened_ms)).collect(),
            View::Starred => self.starred.iter().map(|s| (&s.path, s.starred_ms)).collect(),
        };
        let mut rows: Vec<Row> = entries
            .into_iter()
            .filter_map(|(path, activity_ms)| {
                let kind = AppKind::for_path(path);
                if q.filter.is_some() && kind != q.filter {
                    return None;
                }
                let name = display_name(path);
                let lower = name.to_lowercase();
                if !terms.iter().all(|t| lower.contains(t.as_str())) {
                    return None;
                }
                Some(Row {
                    path: path.clone(),
                    name,
                    kind,
                    activity_ms,
                    starred: q.view == View::Starred || self.is_starred(path),
                    meta: meta.get(path).copied().unwrap_or(FileMeta { exists: true, ..FileMeta::default() }),
                })
            })
            .collect();
        sort_rows(&mut rows, q.sort);
        rows
    }
}

/// Sort `rows` in place; ties keep the most recent activity first.
pub fn sort_rows(rows: &mut [Row], sort: Sort) {
    rows.sort_by(|a, b| {
        // Ascending: A→Z, oldest, smallest first.
        let ord = match sort.key {
            SortKey::Activity => a.activity_ms.cmp(&b.activity_ms),
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Modified => a.meta.modified_ms.cmp(&b.meta.modified_ms),
            SortKey::Size => a.meta.size.cmp(&b.meta.size),
        };
        let ord = if sort.ascending { ord } else { ord.reverse() };
        ord.then_with(|| b.activity_ms.cmp(&a.activity_ms))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_entries_follow_the_copy_in_the_save_folder() {
        let d = std::env::temp_dir().join(format!("so-stale-{}", std::process::id()));
        let save = d.join("save");
        std::fs::create_dir_all(&save).unwrap();
        std::fs::write(save.join("Plan.docx"), "x").unwrap();
        std::fs::write(save.join("Notes.md"), "x").unwrap();
        let mut l = Library::default();
        l.touch(&save.join("Examples/Plan.docx"), 10); // gone
        l.touch(&save.join("Plan.docx"), 20); // already listed
        l.touch(Path::new("/Applications/SolarOffice.app/Contents/Resources/Examples/Notes.md"), 30);
        l.touch(Path::new("/nowhere/Other.md"), 40); // no copy: left as is
        l.set_starred(&save.join("Examples/Plan.docx"), true, 5);
        assert!(l.relocate_stale(&save));
        let paths: Vec<_> = l.recents.iter().map(|r| r.path.clone()).collect();
        assert_eq!(paths, vec![PathBuf::from("/nowhere/Other.md"), save.join("Notes.md"), save.join("Plan.docx")]);
        assert_eq!(l.starred[0].path, save.join("Plan.docx"));
        assert!(!l.relocate_stale(&save), "second run changes nothing");
        std::fs::remove_dir_all(&d).ok();
    }

    fn lib() -> Library {
        let mut l = Library::default();
        l.touch(Path::new("/d/Budget 2026.xlsx"), 10);
        l.touch(Path::new("/d/Plan.docx"), 20);
        l.touch(Path::new("/d/Notes.md"), 30);
        l.touch(Path::new("/d/Deck.pptx"), 40);
        l
    }

    fn names(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn touch_moves_to_top_and_dedups() {
        let mut l = lib();
        l.touch(Path::new("/d/Plan.docx"), 50);
        assert_eq!(l.recents.len(), 4);
        assert_eq!(l.recents[0].path, Path::new("/d/Plan.docx"));
        assert_eq!(l.recents[0].opened_ms, 50);
    }

    #[test]
    fn recent_rows_default_to_newest_first() {
        let rows = lib().rows(&Query::default(), &HashMap::new());
        assert_eq!(names(&rows), ["Deck.pptx", "Notes.md", "Plan.docx", "Budget 2026.xlsx"]);
    }

    #[test]
    fn filter_and_search() {
        let l = lib();
        let q = Query { filter: Some(AppKind::Word), ..Query::default() };
        assert_eq!(names(&l.rows(&q, &HashMap::new())), ["Plan.docx"]);
        let q = Query { search: "  budget 26 ".into(), ..Query::default() };
        assert_eq!(names(&l.rows(&q, &HashMap::new())), ["Budget 2026.xlsx"]);
        let q = Query { search: "zzz".into(), ..Query::default() };
        assert!(l.rows(&q, &HashMap::new()).is_empty());
    }

    #[test]
    fn sort_by_name_both_ways() {
        let l = lib();
        let asc = Query { sort: Sort { key: SortKey::Name, ascending: true }, ..Query::default() };
        assert_eq!(names(&l.rows(&asc, &HashMap::new())), ["Budget 2026.xlsx", "Deck.pptx", "Notes.md", "Plan.docx"]);
        let desc = Query { sort: Sort { key: SortKey::Name, ascending: false }, ..Query::default() };
        assert_eq!(names(&l.rows(&desc, &HashMap::new())), ["Plan.docx", "Notes.md", "Deck.pptx", "Budget 2026.xlsx"]);
    }

    #[test]
    fn sort_by_size_uses_meta() {
        let l = lib();
        let mut meta = HashMap::new();
        for (p, s) in [("/d/Budget 2026.xlsx", 5), ("/d/Plan.docx", 50), ("/d/Notes.md", 1), ("/d/Deck.pptx", 9)] {
            meta.insert(PathBuf::from(p), FileMeta { exists: true, size: s, modified_ms: None });
        }
        let q = Query { sort: Sort { key: SortKey::Size, ascending: false }, ..Query::default() };
        assert_eq!(names(&l.rows(&q, &meta)), ["Plan.docx", "Deck.pptx", "Budget 2026.xlsx", "Notes.md"]);
    }

    #[test]
    fn sort_header_click_cycle() {
        let s = Sort::default().clicked(SortKey::Name);
        assert_eq!(s, Sort { key: SortKey::Name, ascending: true });
        assert_eq!(s.clicked(SortKey::Name), Sort { key: SortKey::Name, ascending: false });
        assert_eq!(s.clicked(SortKey::Size), Sort { key: SortKey::Size, ascending: false });
    }

    #[test]
    fn starring_and_counts() {
        let mut l = lib();
        l.set_starred(Path::new("/d/Plan.docx"), true, 99);
        l.set_starred(Path::new("/d/Plan.docx"), true, 100);
        assert_eq!(l.starred.len(), 1);
        assert_eq!(l.counts(None), (4, 1));
        assert_eq!(l.counts(Some(AppKind::Grid)), (1, 0));
        let q = Query { view: View::Starred, ..Query::default() };
        let rows = l.rows(&q, &HashMap::new());
        assert_eq!(names(&rows), ["Plan.docx"]);
        assert!(rows[0].starred);
        l.remove_recents(&[PathBuf::from("/d/Plan.docx")]);
        assert!(l.is_starred(Path::new("/d/Plan.docx")), "removing from Recent keeps the star");
        l.forget(&[PathBuf::from("/d/Plan.docx")]);
        assert!(!l.is_starred(Path::new("/d/Plan.docx")));
    }

    #[test]
    fn rename_follows_file() {
        let mut l = lib();
        l.set_starred(Path::new("/d/Notes.md"), true, 1);
        l.rename(Path::new("/d/Notes.md"), Path::new("/e/Notes v2.md"));
        assert!(l.recents.iter().any(|r| r.path == Path::new("/e/Notes v2.md")));
        assert!(l.is_starred(Path::new("/e/Notes v2.md")));
    }

    #[test]
    fn recents_are_capped() {
        let mut l = Library::default();
        for i in 0..(MAX_RECENTS + 20) {
            l.touch(&PathBuf::from(format!("/d/{i}.md")), i as i64);
        }
        assert_eq!(l.recents.len(), MAX_RECENTS);
    }
}
