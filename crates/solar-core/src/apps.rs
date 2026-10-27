//! The SolarOffice modules (apps) and the file types each one opens.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One SolarOffice module. Later modules are added here; everything else (home cards, filters,
/// file badges, the open dialog) is driven by this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AppKind {
    Word,
    Grid,
    Deck,
    Draw,
    Pdf,
    Markdown,
    Html,
}

impl AppKind {
    /// Every module, in home-screen order.
    pub const ALL: [AppKind; 7] =
        [AppKind::Word, AppKind::Grid, AppKind::Deck, AppKind::Draw, AppKind::Pdf, AppKind::Markdown, AppKind::Html];

    /// The module's product name.
    pub fn name(self) -> &'static str {
        match self {
            AppKind::Word => "SolarWord",
            AppKind::Grid => "SolarGrid",
            AppKind::Deck => "SolarDeck",
            AppKind::Draw => "SolarDraw",
            AppKind::Pdf => "SolarPDF",
            AppKind::Markdown => "SolarMarkdown",
            AppKind::Html => "SolarHTML",
        }
    }

    /// The home card's title ("New document", ...).
    pub fn new_label(self) -> &'static str {
        match self {
            AppKind::Word => "New document",
            AppKind::Grid => "New workbook",
            AppKind::Deck => "New presentation",
            AppKind::Draw => "New drawing",
            AppKind::Pdf => "New PDF",
            AppKind::Markdown => "New Markdown",
            AppKind::Html => "New web page",
        }
    }

    /// The name a new, unsaved file gets.
    pub fn untitled(self) -> String {
        let stem = match self {
            AppKind::Word => "Untitled document",
            AppKind::Grid => "Untitled workbook",
            AppKind::Deck => "Untitled presentation",
            AppKind::Draw => "Untitled drawing",
            AppKind::Pdf => "Untitled PDF",
            AppKind::Markdown => "Untitled",
            AppKind::Html => "Untitled page",
        };
        format!("{stem}.{}", self.default_ext())
    }

    /// The file-type filter pill's label.
    pub fn filter_label(self) -> &'static str {
        match self {
            AppKind::Word => "Docs",
            AppKind::Grid => "Sheets",
            AppKind::Deck => "Slides",
            AppKind::Draw => "Drawings",
            AppKind::Pdf => "PDF",
            AppKind::Markdown => "Markdown",
            AppKind::Html => "HTML",
        }
    }

    /// The extension new files are saved with.
    pub fn default_ext(self) -> &'static str {
        self.extensions()[0]
    }

    /// Every extension (lower case, no dot) the module opens; the first is the default.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            AppKind::Word => &["docx", "docm", "dotx", "doc", "odt", "ott", "rtf"],
            AppKind::Grid => &["xlsx", "xlsm", "xltx", "xls", "ods", "ots", "csv"],
            AppKind::Deck => &["pptx", "pptm", "potx", "ppt", "odp", "otp"],
            // Diagrams (draw.io) and vector art (VectorCraft); `.drawio.png` / `.drawio.svg` are
            // recognised by `for_path`.
            AppKind::Draw => &["drawio", "dio", "vsdx", "vectorcraft", "svg", "svgz", "ai", "eps", "dxf", "afdesign"],
            AppKind::Pdf => &["pdf"],
            AppKind::Markdown => &["md", "markdown"],
            AppKind::Html => &["html", "htm"],
        }
    }

    /// The module colour (sRGB), used for file badges and tab accents.
    pub fn color(self) -> [u8; 3] {
        match self {
            AppKind::Word => [0x32, 0x76, 0xCD],
            AppKind::Grid => [0x4F, 0xA1, 0x6B],
            AppKind::Deck => [0xD3, 0x39, 0x22],
            AppKind::Draw => [0xE0, 0x8A, 0x00],
            AppKind::Pdf => [0xEF, 0x44, 0x44],
            AppKind::Markdown => [0x8B, 0x5C, 0xF6],
            AppKind::Html => [0x0F, 0xA3, 0xA3],
        }
    }

    /// The module that opens files with extension `ext` (any case, no dot).
    pub fn from_ext(ext: &str) -> Option<AppKind> {
        let ext = ext.to_ascii_lowercase();
        AppKind::ALL.into_iter().find(|k| k.extensions().contains(&ext.as_str()))
    }

    /// The module that opens `path`, from its extension.
    pub fn for_path(path: &Path) -> Option<AppKind> {
        // draw.io's editable PNG and SVG exports carry the diagram inside: `Flow.drawio.png`.
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if name.ends_with(".drawio.png") || name.ends_with(".drawio.svg") {
            return Some(AppKind::Draw);
        }
        path.extension().and_then(|e| e.to_str()).and_then(AppKind::from_ext)
    }

    /// Stable lower-case id, for settings and logs.
    pub fn id(self) -> &'static str {
        match self {
            AppKind::Word => "word",
            AppKind::Grid => "grid",
            AppKind::Deck => "deck",
            AppKind::Draw => "draw",
            AppKind::Pdf => "pdf",
            AppKind::Markdown => "markdown",
            AppKind::Html => "html",
        }
    }
}

/// Every extension SolarOffice opens, for the Open dialog filter.
pub fn all_extensions() -> Vec<&'static str> {
    AppKind::ALL.iter().flat_map(|k| k.extensions().iter().copied()).collect()
}

/// Whether SolarOffice opens `path`.
pub fn is_supported(path: &Path) -> bool {
    AppKind::for_path(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_unique_across_apps() {
        let all = all_extensions();
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(all.len(), sorted.len(), "an extension belongs to two apps");
    }

    #[test]
    fn from_ext_is_case_insensitive() {
        assert_eq!(AppKind::from_ext("DOCX"), Some(AppKind::Word));
        assert_eq!(AppKind::from_ext("Md"), Some(AppKind::Markdown));
        assert_eq!(AppKind::from_ext("exe"), None);
        assert_eq!(AppKind::for_path(Path::new("/a/b/Flow.DRAWIO")), Some(AppKind::Draw));
        assert_eq!(AppKind::for_path(Path::new("/a/b/Flow.drawio.png")), Some(AppKind::Draw));
        assert_eq!(AppKind::for_path(Path::new("/a/b/photo.png")), None);
        assert_eq!(AppKind::for_path(Path::new("/a/b/Poster.vectorcraft")), Some(AppKind::Draw));
        assert_eq!(AppKind::for_path(Path::new("/a/b/noext")), None);
    }

    #[test]
    fn untitled_uses_default_ext() {
        for k in AppKind::ALL {
            assert!(k.untitled().ends_with(&format!(".{}", k.default_ext())));
            assert_eq!(AppKind::from_ext(k.default_ext()), Some(k));
        }
    }
}
