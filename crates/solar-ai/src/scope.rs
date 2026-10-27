//! Who is asking: the home screen or one of the modules. A scope picks the default model and
//! adds the module's own instructions, so every module gets a model that knows its job and the
//! format its output has to be in.

use serde::{Deserialize, Serialize};
use solar_core::AppKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Scope {
    /// The home screen's Ask AI.
    #[default]
    Home,
    /// A module (SolarWord, SolarGrid, ...).
    App(AppKind),
}

/// The text format a module wants generated content in, so it can import it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    /// CSV with a header row.
    Csv,
    /// JSON matching [`crate::schemas::deck_outline`].
    DeckOutlineJson,
    /// draw.io XML (`<mxfile>`).
    DrawioXml,
    /// One self-contained HTML file.
    Html,
}

impl Scope {
    pub const ALL: [Scope; 8] = [
        Scope::Home,
        Scope::App(AppKind::Word),
        Scope::App(AppKind::Grid),
        Scope::App(AppKind::Deck),
        Scope::App(AppKind::Draw),
        Scope::App(AppKind::Pdf),
        Scope::App(AppKind::Markdown),
        Scope::App(AppKind::Html),
    ];

    /// Stable id used in `ai.toml` (`home`, `word`, `grid`, ...).
    pub fn id(self) -> &'static str {
        match self {
            Scope::Home => "home",
            Scope::App(k) => k.id(),
        }
    }

    pub fn from_id(id: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::Home => "Home (Ask AI)",
            Scope::App(k) => k.name(),
        }
    }

    /// The format this module imports generated content in.
    pub fn output_format(self) -> OutputFormat {
        match self {
            Scope::Home | Scope::App(AppKind::Word) | Scope::App(AppKind::Pdf) | Scope::App(AppKind::Markdown) => OutputFormat::Markdown,
            Scope::App(AppKind::Grid) => OutputFormat::Csv,
            Scope::App(AppKind::Deck) => OutputFormat::DeckOutlineJson,
            Scope::App(AppKind::Draw) => OutputFormat::DrawioXml,
            Scope::App(AppKind::Html) => OutputFormat::Html,
        }
    }

    /// The module's standing instructions, put in front of every request's own system prompt.
    pub fn system_prompt(self) -> &'static str {
        match self {
            Scope::Home => {
                "You are the assistant in SolarOffice, an office suite with SolarWord (documents), SolarGrid (spreadsheets), \
                 SolarDeck (presentations), SolarDraw (diagrams and vector drawings), SolarPDF, SolarMarkdown and SolarHTML. \
                 Answer clearly and concisely in the user's language. Use Markdown for structure when it helps."
            }
            Scope::App(AppKind::Word) => {
                "You are SolarWord's writing assistant. Write clear, well-structured prose in the user's language. \
                 When you produce document content, return Markdown only: # headings, paragraphs, lists and pipe tables, \
                 with no preamble or closing remarks, so it can be inserted into the document as is."
            }
            Scope::App(AppKind::Grid) => {
                "You are SolarGrid's spreadsheet assistant. When you produce table data, return CSV only (comma-separated, \
                 a header row first, quotes around fields that contain commas). Write formulas in Excel syntax \
                 (=SUM(B2:B9)), using only functions that Excel 2010 and LibreOffice both support. Explain briefly when asked."
            }
            Scope::App(AppKind::Deck) => {
                "You are SolarDeck's presentation assistant. One message per slide, short titles (at most 8 words), \
                 at most 5 bullets of at most 12 words each, and speaker notes that carry the detail. When you produce a deck \
                 outline, return JSON only, matching the requested schema."
            }
            Scope::App(AppKind::Draw) => {
                "You are SolarDraw's diagram assistant. When you produce a diagram, return draw.io XML only: one <mxfile> with \
                 one <diagram> holding an uncompressed <mxGraphModel>, vertices and edges with mxGeometry, laid out left to \
                 right or top to bottom without overlaps."
            }
            Scope::App(AppKind::Pdf) => {
                "You are SolarPDF's reading assistant. Answer from the attached PDF; quote the passage and name the page when \
                 you can. Say so plainly when the document doesn't contain the answer. Use Markdown."
            }
            Scope::App(AppKind::Markdown) => {
                "You are SolarMarkdown's writing assistant. Return CommonMark/GitHub-flavoured Markdown only (tables, task \
                 lists and fenced code allowed), with no preamble, so it can be inserted as is."
            }
            Scope::App(AppKind::Html) => {
                "You are SolarHTML's web assistant. When you produce a page, return one complete, self-contained HTML5 file \
                 (inline CSS and JavaScript, no external requests, responsive, accessible, light and dark colour schemes), \
                 with no explanation around it."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_cover_every_module() {
        for s in Scope::ALL {
            assert_eq!(Scope::from_id(s.id()), Some(s));
            assert!(!s.system_prompt().is_empty());
        }
        for k in AppKind::ALL {
            assert!(Scope::ALL.contains(&Scope::App(k)), "{} has no AI scope", k.name());
        }
    }
}
