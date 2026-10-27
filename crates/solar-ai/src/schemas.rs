//! Structured outputs modules import: JSON schemas plus the Rust types they parse into.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A presentation outline (SolarDeck).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckOutline {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub slides: Vec<SlideOutline>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideOutline {
    pub title: String,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub fn deck_outline() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "slides": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "bullets": {"type": "array", "items": {"type": "string"}},
                    "notes": {"type": "string"}
                },
                "required": ["title", "bullets"]
            }}
        },
        "required": ["title", "slides"]
    })
}

/// A table (SolarGrid, tables in documents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

pub fn table() -> Value {
    json!({
        "type": "object",
        "properties": {
            "columns": {"type": "array", "items": {"type": "string"}},
            "rows": {"type": "array", "items": {"type": "array", "items": {"type": ["string", "number", "boolean", "null"]}}}
        },
        "required": ["columns", "rows"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_parse() {
        let d: DeckOutline = serde_json::from_value(json!({"title":"Pitch","slides":[{"title":"Why","bullets":["a","b"]}]})).unwrap();
        assert_eq!(d.slides[0].bullets.len(), 2);
        let t: Table = serde_json::from_value(json!({"columns":["Item","Cost"],"rows":[["Rent", 4200]]})).unwrap();
        assert_eq!(t.rows[0][1], json!(4200));
        assert_eq!(deck_outline()["required"][1], "slides");
    }
}
