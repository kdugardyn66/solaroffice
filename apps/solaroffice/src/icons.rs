//! Interface icons as inline SVG.
//!
//! Line icons follow Lucide (ISC licence, see NOTICE); the module badges are SolarOffice's own.

use dioxus::prelude::*;
use solar_core::AppKind;

pub const HOME: &str = r#"<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#;
pub const PLUS: &str = r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#;
pub const X: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
pub const CLOCK: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#;
pub const STAR: &str = r#"<path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/>"#;
pub const SPARKLES: &str = r#"<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/><path d="M20 3v4"/><path d="M22 5h-4"/><path d="M4 17v2"/><path d="M5 18H3"/>"#;
pub const FOLDER: &str = r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#;
pub const FOLDER_PLUS: &str = r#"<path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#;
pub const FOLDER_OPEN: &str = r#"<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>"#;
pub const CHEVRON_RIGHT: &str = r#"<path d="m9 18 6-6-6-6"/>"#;
pub const CHEVRON_DOWN: &str = r#"<path d="m6 9 6 6 6-6"/>"#;
pub const SETTINGS: &str = r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>"#;
pub const SEARCH: &str = r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#;
pub const MORE: &str = r#"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"#;
pub const SUN: &str = r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>"#;
pub const MOON: &str = r#"<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>"#;
pub const ARROW_UP: &str = r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#;
pub const ARROW_DOWN: &str = r#"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"#;
pub const TRASH: &str = r#"<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>"#;
pub const COPY: &str = r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#;
pub const PENCIL: &str = r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>"#;
pub const EXTERNAL: &str = r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"#;
pub const MINUS_CIRCLE: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M8 12h8"/>"#;
pub const REFRESH: &str = r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#;
pub const ALERT: &str = r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#;
pub const FILE: &str = r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>"#;
pub const CHAT: &str = r#"<path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>"#;
pub const STOP: &str = r#"<rect x="6" y="6" width="12" height="12" rx="2"/>"#;
pub const BROOM: &str = r#"<path d="m19 11-8-8-8.6 8.6a2 2 0 0 0 0 2.8l5.2 5.2c.8.8 2 .8 2.8 0L19 11Z"/><path d="m5 2 5 5"/><path d="M2 13h15"/>"#;
pub const SEND: &str = r#"<path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"/><path d="m21.854 2.147-10.94 10.939"/>"#;

/// A line icon, `size` px, in the current text colour.
#[component]
pub fn Icon(svg: &'static str, #[props(default = 16)] size: u32, #[props(default)] class: String, #[props(default)] filled: bool) -> Element {
    let fill = if filled { "currentColor" } else { "none" };
    rsx! {
        svg {
            class: "shrink-0 {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "{fill}",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            dangerous_inner_html: svg,
        }
    }
}

/// The white glyph drawn on a module's badge.
fn glyph(kind: AppKind) -> &'static str {
    match kind {
        // A page with lines of text.
        AppKind::Word => r#"<path d="M7 7.5h10M7 11h10M7 14.5h10M7 18h6"/>"#,
        // A table.
        AppKind::Grid => r#"<rect x="5" y="5.5" width="14" height="13" rx="1.5"/><path d="M5 10h14M5 14.5h14M10.5 5.5v13"/>"#,
        // A slide on an easel.
        AppKind::Deck => r#"<rect x="4.5" y="5" width="15" height="10" rx="1.5"/><path d="M12 15v3.5M9 19.5l3-1.5 3 1.5"/><path d="M8 12l2.5-2.5 2 2L16 8" />"#,
        // Shapes joined by a connector.
        AppKind::Draw => r#"<rect x="4.5" y="4.5" width="6" height="6" rx="1"/><circle cx="16.5" cy="16.5" r="3.2"/><path d="M10.5 7.5h3a3 3 0 0 1 3 3v2.8"/>"#,
        AppKind::Pdf => r##"<text x="12" y="15.6" text-anchor="middle" font-size="8.2" font-weight="800" font-family="-apple-system,Segoe UI,sans-serif" fill="#fff" stroke="none" letter-spacing="-.2">PDF</text>"##,
        AppKind::Markdown => r#"<path d="M5 16V8l3.5 4L12 8v8"/><path d="M17 8v8M14.5 13.5 17 16l2.5-2.5"/>"#,
        AppKind::Html => r#"<path d="m9 8-4 4 4 4M15 8l4 4-4 4M13 6.5l-2 11"/>"#,
    }
}

/// `#RRGGBB` for a module's colour.
pub fn hex(kind: AppKind) -> String {
    let [r, g, b] = kind.color();
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// The module's badge: its glyph on a rounded square in the module colour. Unknown file types
/// get a grey page.
#[component]
pub fn Badge(kind: Option<AppKind>, #[props(default = 32)] size: u32) -> Element {
    let radius = (size as f32 * 0.24).round();
    let (bg, inner) = match kind {
        Some(k) => (hex(k), glyph(k)),
        None => ("#9CA3AF".to_string(), FILE),
    };
    let glyph_size = (size as f32 * 0.78).round();
    rsx! {
        span {
            class: "inline-flex items-center justify-center shrink-0 text-white",
            style: "width:{size}px;height:{size}px;border-radius:{radius}px;background:{bg};box-shadow:inset 0 -1px 0 rgba(0,0,0,.12)",
            svg {
                width: "{glyph_size}",
                height: "{glyph_size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "#fff",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                dangerous_inner_html: inner,
            }
        }
    }
}
