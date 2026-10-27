//! What the AI layer knows about models: limits, capabilities and prices (`models.toml`).

use serde::{Deserialize, Serialize};

use crate::config::ProviderKind;
use crate::types::Usage;

/// `$2`, `$0.75`, `$0.10`.
fn dollars(v: f64) -> String {
    if v >= 10.0 || (v.fract() == 0.0 && v >= 1.0) { format!("${v:.0}") } else { format!("${v:.2}") }
}

/// What a model is good at. A model lists up to [`Category::MAX_PER_MODEL`] of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Reason,
    Chat,
    Write,
    Translate,
    Code,
    Create,
    Listen,
    Speak,
    See,
    Predict,
}

impl Category {
    pub const ALL: [Category; 10] = [
        Category::Reason,
        Category::Chat,
        Category::Write,
        Category::Translate,
        Category::Code,
        Category::Create,
        Category::Listen,
        Category::Speak,
        Category::See,
        Category::Predict,
    ];
    pub const MAX_PER_MODEL: usize = 5;

    pub fn label(self) -> &'static str {
        match self {
            Category::Reason => "Reason",
            Category::Chat => "Chat",
            Category::Write => "Write",
            Category::Translate => "Translate",
            Category::Code => "Code",
            Category::Create => "Create",
            Category::Listen => "Listen",
            Category::Speak => "Speak",
            Category::See => "See",
            Category::Predict => "Predict",
        }
    }

    /// The universal icon (an emoji; egui draws it in one colour).
    pub fn emoji(self) -> &'static str {
        match self {
            Category::Reason => "🧠",
            Category::Chat => "💬",
            Category::Write => "✍",
            Category::Translate => "🌐",
            Category::Code => "💻",
            Category::Create => "🎨",
            Category::Listen => "🎙",
            Category::Speak => "🔊",
            Category::See => "👁",
            Category::Predict => "📈",
        }
    }

    /// The core task, for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Category::Reason => "Multi-step logic, problem solving, and autonomous code execution",
            Category::Chat => "Fluid conversational interaction, digital companions, and support agents",
            Category::Write => "Content drafting, text summarization, and data extraction",
            Category::Translate => "Cross-lingual conversion and dialect-aware localization",
            Category::Code => "Software generation, bug fixing, and syntax refactoring",
            Category::Create => "Generative imagery, design layout synthesis, and video rendering",
            Category::Listen => "Speech-to-text transcription and audio pattern analysis",
            Category::Speak => "Natural text-to-speech synthesis and voice cloning",
            Category::See => "Visual object detection, facial mapping, and image diagnostics",
            Category::Predict => "Pattern-based forecasting and algorithmic trend analysis",
        }
    }
}

/// One model's facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Which API the model belongs to (`anthropic`, `gemini`, `openai`).
    pub provider: ProviderKind,
    pub id: String,
    pub name: String,
    /// Context window in tokens (0 for translation and image models).
    #[serde(default)]
    pub context: u64,
    #[serde(default)]
    pub max_output: u32,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub pdf: bool,
    #[serde(default)]
    pub tools: bool,
    /// US dollars per million input / output tokens; `None` when unknown.
    pub input: Option<f64>,
    pub output: Option<f64>,
    /// The model suggested by default for its provider.
    #[serde(default)]
    pub recommended: bool,
    /// What it's good at (at most five), strongest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<Category>,
    /// A price that isn't per token (`$0.03 per megapixel`), or a remark on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_note: Option<String>,
}

impl ModelInfo {
    /// Defaults for a model the catalog doesn't know: a local model is free and assumed to
    /// handle tools; a hosted one has unknown prices.
    pub fn unknown(kind: ProviderKind, id: &str) -> ModelInfo {
        let local = kind.is_local();
        ModelInfo {
            provider: kind,
            id: id.into(),
            name: id.into(),
            context: if local { 32_768 } else { 128_000 },
            max_output: if local { 4096 } else { 8192 },
            vision: false,
            pdf: false,
            tools: true,
            input: local.then_some(0.0),
            output: local.then_some(0.0),
            recommended: false,
            categories: Vec::new(),
            price_note: None,
        }
    }

    /// A short price indication for pickers: `$2 in · $10 out per 1M tokens`, `Free · local`,
    /// or `Price not listed`.
    pub fn price_label(&self) -> String {
        if let Some(n) = self.price_note.as_deref().filter(|n| !n.trim().is_empty()) {
            return n.to_string();
        }
        match (self.input, self.output) {
            (Some(i), Some(o)) if i == 0.0 && o == 0.0 && self.provider.is_local() => "Free · runs on your machine".into(),
            (Some(i), Some(o)) if i == 0.0 && o == 0.0 => "Free".into(),
            (Some(i), Some(o)) => format!("{} in · {} out per 1M tokens", dollars(i), dollars(o)),
            _ => "Price not listed".into(),
        }
    }

    /// A compact price tier, `$` to `$$$$`, from the output price (empty when free or unknown).
    pub fn price_tier(&self) -> &'static str {
        match self.output {
            Some(o) if o <= 0.0 => "",
            Some(o) if o < 2.0 => "$",
            Some(o) if o < 8.0 => "$$",
            Some(o) if o < 25.0 => "$$$",
            Some(_) => "$$$$",
            None => "",
        }
    }

    /// Has `c` among its categories.
    pub fn is(&self, c: Category) -> bool {
        self.categories.contains(&c)
    }

    /// Estimated cost of `usage` in US dollars.
    pub fn cost(&self, usage: Usage) -> Option<f64> {
        Some(usage.input_tokens as f64 * self.input? / 1e6 + usage.output_tokens as f64 * self.output? / 1e6)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    model: Vec<ModelInfo>,
}

/// The model catalog.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub models: Vec<ModelInfo>,
}

impl Catalog {
    /// The catalog shipped with SolarOffice.
    pub fn builtin() -> Catalog {
        Catalog::parse(include_str!("../models.toml")).expect("models.toml is valid")
    }

    pub fn parse(text: &str) -> Result<Catalog, String> {
        let f: CatalogFile = toml::from_str(text).map_err(|e| e.to_string())?;
        Ok(Catalog { models: f.model })
    }

    /// Entries in `extra` replace built-in ones with the same provider and id.
    pub fn merged(mut self, extra: &[ModelInfo]) -> Catalog {
        for m in extra {
            self.models.retain(|x| !(x.provider == m.provider && x.id == m.id));
            self.models.push(m.clone());
        }
        self
    }

    /// The facts for `id` under an API of kind `kind`: an exact match, else a dated snapshot of
    /// a known id (`claude-sonnet-5-5-20261001`), else defaults.
    pub fn info(&self, kind: ProviderKind, id: &str) -> ModelInfo {
        let same = |m: &&ModelInfo| m.provider == kind;
        if let Some(m) = self.models.iter().filter(same).find(|m| m.id == id) {
            return m.clone();
        }
        if let Some(m) = self.models.iter().filter(same).filter(|m| id.starts_with(&format!("{}-", m.id))).max_by_key(|m| m.id.len()) {
            let mut m = m.clone();
            m.id = id.into();
            return m;
        }
        ModelInfo::unknown(kind, id)
    }

    /// The catalog's models for an API, recommended first.
    pub fn for_kind(&self, kind: ProviderKind) -> Vec<&ModelInfo> {
        let mut v: Vec<&ModelInfo> = self.models.iter().filter(|m| m.provider == kind).collect();
        v.sort_by_key(|m| !m.recommended);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_sane() {
        let c = Catalog::builtin();
        use crate::config::Service;
        for kind in ProviderKind::ALL.into_iter().filter(|k| !k.lists_own_models()) {
            let ms = c.for_kind(kind);
            assert!(!ms.is_empty(), "{kind:?} has models");
            assert!(ms[0].recommended, "{kind:?} lists its recommended model first");
            assert_eq!(ms.iter().filter(|m| m.recommended).count(), 1, "{kind:?}");
        }
        let mut seen = std::collections::HashSet::new();
        for m in &c.models {
            assert!(seen.insert((m.provider, m.id.clone())), "{} listed twice", m.id);
            if m.provider.service() == Service::Chat {
                assert!(m.context > 0 && m.max_output > 0, "{}", m.id);
            }
            assert!((m.input.is_some() && m.output.is_some()) || m.price_note.is_some(), "{} has a price or a note", m.id);
            assert!(!m.categories.is_empty() && m.categories.len() <= Category::MAX_PER_MODEL, "{}: 1 to 5 categories", m.id);
            let unique: std::collections::HashSet<_> = m.categories.iter().collect();
            assert_eq!(unique.len(), m.categories.len(), "{}", m.id);
        }
        assert!(c.info(ProviderKind::DeepL, "quality_optimized").is(Category::Translate));
        assert!(c.info(ProviderKind::BlackForestLabs, "flux-2-pro").is(Category::Create));
    }

    #[test]
    fn lookups_and_cost() {
        let c = Catalog::builtin();
        let m = c.info(ProviderKind::Anthropic, "claude-sonnet-5-5");
        assert_eq!(m.name, "Claude Sonnet 5.5");
        let snap = c.info(ProviderKind::Anthropic, "claude-sonnet-5-5-20261001");
        assert_eq!((snap.name.as_str(), snap.id.as_str()), ("Claude Sonnet 5.5", "claude-sonnet-5-5-20261001"));
        let cost = m.cost(Usage { input_tokens: 1_000_000, output_tokens: 100_000 }).unwrap();
        assert!((cost - 3.0).abs() < 1e-9);
        let local = c.info(ProviderKind::Ollama, "llama3.2:3b");
        assert_eq!(local.cost(Usage { input_tokens: 5000, output_tokens: 5000 }), Some(0.0));
        let unknown = c.info(ProviderKind::OpenAi, "gpt-9");
        assert_eq!(unknown.cost(Usage::default()), None);
    }

    #[test]
    fn price_labels() {
        let c = Catalog::builtin();
        let sonnet = c.info(ProviderKind::Anthropic, "claude-sonnet-5-5");
        assert_eq!(sonnet.price_label(), "$2 in · $10 out per 1M tokens");
        assert_eq!(sonnet.price_tier(), "$$$");
        let flash = c.info(ProviderKind::Gemini, "gemini-3.8-flash");
        assert_eq!(flash.price_label(), "$0.75 in · $3.75 out per 1M tokens");
        assert_eq!(flash.price_tier(), "$$");
        assert_eq!(c.info(ProviderKind::OpenAi, "gpt-6-luna").price_tier(), "$");
        assert_eq!(c.info(ProviderKind::Anthropic, "claude-fable-5-1").price_tier(), "$$$$");
        let local = c.info(ProviderKind::LmStudio, "qwen3-8b");
        assert_eq!(local.price_label(), "Free · runs on your machine");
        assert_eq!(local.price_tier(), "");
        assert_eq!(c.info(ProviderKind::OpenAi, "gpt-9").price_label(), "Price not listed");
        assert_eq!(c.info(ProviderKind::Zai, "glm-4.7-flash").price_label(), "Free");
        assert_eq!(c.info(ProviderKind::BlackForestLabs, "flux-2-pro").price_label(), "From $0.03 per megapixel");
    }

    #[test]
    fn user_entries_override() {
        let mut mine = ModelInfo::unknown(ProviderKind::Anthropic, "claude-sonnet-5-5");
        mine.input = Some(1.0);
        let c = Catalog::builtin().merged(&[mine]);
        assert_eq!(c.info(ProviderKind::Anthropic, "claude-sonnet-5-5").input, Some(1.0));
    }
}
