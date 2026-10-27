//! File operations behind the home screen's row menu, and the hand-offs to the operating system
//! (Finder, default apps, clipboard, Trash).

use std::io;
use std::path::{Path, PathBuf};

use solar_core::folders::unique_name;

/// Rename `path` to `new_stem`, keeping its extension (`Plan.docx` + "Plan v2" → `Plan v2.docx`).
/// A stem that already ends in the extension is taken as is. Refuses to overwrite.
pub fn rename_keep_ext(path: &Path, new_stem: &str) -> io::Result<PathBuf> {
    let stem = new_stem.trim();
    if stem.is_empty() || stem.contains('/') || stem.contains('\\') || stem == "." || stem == ".." {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "That isn't a valid file name."));
    }
    let ext = full_extension(path);
    let name = if ext.is_empty() || stem.to_lowercase().ends_with(&ext.to_lowercase()) { stem.to_string() } else { format!("{stem}{ext}") };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let to = dir.join(&name);
    if to == path {
        return Ok(to);
    }
    // Case-only renames on case-insensitive file systems: the target "exists" but is the file.
    let same_file = to.to_string_lossy().to_lowercase() == path.to_string_lossy().to_lowercase();
    if to.exists() && !same_file {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("“{name}” already exists in this folder.")));
    }
    std::fs::rename(path, &to)?;
    Ok(to)
}

/// The part of the file name a rename keeps: `.docx`, or `.drawio.png` for draw.io exports.
pub fn full_extension(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let lower = name.to_lowercase();
    for double in [".drawio.png", ".drawio.svg"] {
        if lower.ends_with(double) {
            return name[name.len() - double.len()..].to_string();
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_string(),
        _ => String::new(),
    }
}

/// The name without [`full_extension`], for the rename box.
pub fn stem(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = full_extension(path);
    name[..name.len() - ext.len()].to_string()
}

/// Copy `path` beside itself as `Name copy.ext` (`Name copy 2.ext`, ...).
pub fn duplicate(path: &Path) -> io::Result<PathBuf> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = format!("{} copy{}", stem(path), full_extension(path));
    let to = unique_name(dir, &name);
    std::fs::copy(path, &to)?;
    Ok(to)
}

/// Move `path` into `dir`; a name already taken there gets a number (`Plan 2.docx`).
pub fn move_into(path: &Path, dir: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No file name"))?;
    if path.parent() == Some(dir) {
        return Ok(path.to_path_buf());
    }
    let to = unique_name(dir, &name.to_string_lossy());
    if std::fs::rename(path, &to).is_err() {
        // Another volume: copy, then remove the original.
        std::fs::copy(path, &to)?;
        std::fs::remove_file(path)?;
    }
    Ok(to)
}

/// Create a subfolder `New folder` (`New folder 2`, ...) in `dir`.
pub fn new_folder(dir: &Path) -> io::Result<PathBuf> {
    let to = unique_name(dir, "New folder");
    std::fs::create_dir_all(&to)?;
    Ok(to)
}

/// Move `paths` to the system Trash (not a permanent delete).
pub fn move_to_trash(paths: &[PathBuf]) -> Result<(), String> {
    trash::delete_all(paths).map_err(|e| e.to_string())
}

/// Show `path` selected in Finder / the file manager.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(if path.is_dir() { path } else { path.parent().unwrap_or(path) }).spawn();
    if let Err(e) = r {
        log::warn!("reveal {}: {e}", path.display());
    }
}

/// Open `path` with the system's default app.
pub fn open_external(path: &Path) {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    if let Err(e) = r {
        log::warn!("open {}: {e}", path.display());
    }
}

/// Put `text` on the clipboard.
pub fn copy_text(text: &str) -> bool {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).is_ok()
}

/// The folder with the bundled example documents: `Contents/Resources` in the macOS app,
/// `examples/` beside the executable, or the source tree's `examples/` in development.
pub fn bundled_examples() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let candidates = [
        exe_dir.join("../Resources"),
        exe_dir.join("examples"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples"),
    ];
    // (`target/debug/examples` is Cargo's own, empty folder: a candidate must hold documents.)
    candidates.into_iter().find(|p| !solar_core::folders::list(p).files.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_stems() {
        assert_eq!(full_extension(Path::new("/a/Plan.docx")), ".docx");
        assert_eq!(full_extension(Path::new("/a/Flow.drawio.PNG")), ".drawio.PNG");
        assert_eq!(full_extension(Path::new("/a/README")), "");
        assert_eq!(full_extension(Path::new("/a/.hidden")), "");
        assert_eq!(stem(Path::new("/a/Project Sunrise Notes.md")), "Project Sunrise Notes");
        assert_eq!(stem(Path::new("/a/Flow.drawio.svg")), "Flow");
    }

    #[test]
    fn rename_keeps_extension_and_refuses_overwrite() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("Plan.docx");
        std::fs::write(&a, "x").unwrap();
        std::fs::write(d.path().join("Taken.docx"), "y").unwrap();
        let b = rename_keep_ext(&a, " Plan v2 ").unwrap();
        assert_eq!(b, d.path().join("Plan v2.docx"));
        assert!(rename_keep_ext(&b, "Taken").is_err());
        assert!(rename_keep_ext(&b, "").is_err());
        assert!(rename_keep_ext(&b, "a/b").is_err());
        let c = rename_keep_ext(&b, "Final.docx").unwrap();
        assert_eq!(c, d.path().join("Final.docx"), "a typed extension is not doubled");
    }

    #[test]
    fn duplicate_and_move() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("Notes.md");
        std::fs::write(&a, "hello").unwrap();
        assert_eq!(duplicate(&a).unwrap(), d.path().join("Notes copy.md"));
        assert_eq!(duplicate(&a).unwrap(), d.path().join("Notes copy 2.md"));
        let sub = new_folder(d.path()).unwrap();
        assert_eq!(sub, d.path().join("New folder"));
        std::fs::write(sub.join("Notes.md"), "other").unwrap();
        let moved = move_into(&a, &sub).unwrap();
        assert_eq!(moved, sub.join("Notes 2.md"));
        assert!(!a.exists());
        assert_eq!(std::fs::read_to_string(moved).unwrap(), "hello");
    }
}
