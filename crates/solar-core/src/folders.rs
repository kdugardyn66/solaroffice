//! Listing the sidebar folders.

use std::path::{Path, PathBuf};

use crate::apps::is_supported;
use crate::library::FileMeta;

/// A folder's subfolders and the files SolarOffice opens, both sorted by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Listing {
    pub folders: Vec<PathBuf>,
    pub files: Vec<(PathBuf, FileMeta)>,
    /// The folder couldn't be read (missing, or no permission).
    pub error: Option<String>,
}

/// List `dir`, skipping hidden entries and files of other types.
pub fn list(dir: &Path) -> Listing {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => return Listing { error: Some(e.to_string()), ..Listing::default() },
    };
    let mut out = Listing::default();
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name.starts_with("~$") {
            continue;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            // macOS packages (Pages documents, apps, libraries) are not folders to the user.
            if !path.extension().is_some_and(|e| ["app", "pages", "numbers", "key", "photoslibrary"].contains(&e.to_string_lossy().as_ref())) {
                out.folders.push(path);
            }
        } else if is_supported(&path) {
            let meta = FileMeta::read(&path);
            out.files.push((path, meta));
        }
    }
    let key = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    out.folders.sort_by_key(|p| key(p));
    out.files.sort_by_key(|(p, _)| key(p));
    out
}

/// Copy the documents SolarOffice opens from `src` into `dest` (created if needed), skipping
/// files that already exist there. Returns how many were copied.
pub fn copy_missing(src: &Path, dest: &Path) -> std::io::Result<usize> {
    let listing = list(src);
    if let Some(e) = listing.error {
        return Err(std::io::Error::other(e));
    }
    std::fs::create_dir_all(dest)?;
    let mut n = 0;
    for (path, _) in &listing.files {
        let Some(name) = path.file_name() else { continue };
        let to = dest.join(name);
        if !to.exists() {
            std::fs::copy(path, &to)?;
            n += 1;
        }
    }
    Ok(n)
}

/// A free file name in `dir` for `name` ("Untitled.md", "Untitled 2.md", ...).
pub fn unique_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s, format!(".{e}")),
        None => (name, String::new()),
    };
    (2..).map(|i| dir.join(format!("{stem} {i}{ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_missing_keeps_existing_files() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.md"), "new").unwrap();
        std::fs::write(src.path().join("b.docx"), "doc").unwrap();
        std::fs::write(src.path().join("notes.txt"), "skip").unwrap();
        let out = dest.path().join("Examples");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("a.md"), "edited").unwrap();
        assert_eq!(copy_missing(src.path(), &out).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(out.join("a.md")).unwrap(), "edited", "existing files are kept");
        assert!(out.join("b.docx").exists());
        assert!(!out.join("notes.txt").exists(), "only documents SolarOffice opens");
    }

    #[test]
    fn lists_supported_files_and_folders() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("Zeta")).unwrap();
        std::fs::create_dir(d.path().join("alpha")).unwrap();
        std::fs::create_dir(d.path().join(".git")).unwrap();
        for f in ["b.docx", "A.md", "x.exe", ".hidden.md", "~$lock.docx"] {
            std::fs::write(d.path().join(f), "hi").unwrap();
        }
        let l = list(d.path());
        let names = |v: Vec<&PathBuf>| v.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(names(l.folders.iter().collect()), ["alpha", "Zeta"]);
        assert_eq!(names(l.files.iter().map(|(p, _)| p).collect()), ["A.md", "b.docx"]);
        assert_eq!(l.files[0].1.size, 2);
        assert!(list(&d.path().join("missing")).error.is_some());
    }

    #[test]
    fn unique_names() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(unique_name(d.path(), "Untitled.md"), d.path().join("Untitled.md"));
        std::fs::write(d.path().join("Untitled.md"), "").unwrap();
        std::fs::write(d.path().join("Untitled 2.md"), "").unwrap();
        assert_eq!(unique_name(d.path(), "Untitled.md"), d.path().join("Untitled 3.md"));
    }
}
