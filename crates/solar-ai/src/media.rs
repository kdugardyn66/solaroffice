//! Requests and results for the non-chat services: translation (DeepL) and image generation
//! (Black Forest Labs' FLUX). See [`crate::AiHub::translate`] and [`crate::AiHub::generate_image`].

/// Text to translate.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TranslateRequest {
    /// One or more texts; the results come back in the same order.
    pub texts: Vec<String>,
    /// Language code, e.g. `NL`, `EN-GB`, `PT-BR`.
    pub target_lang: String,
    /// Detected when `None`.
    pub source_lang: Option<String>,
    /// `more`, `less`, `prefer_more`, `prefer_less` (languages that have formal forms).
    pub formality: Option<String>,
    /// The service's model; for DeepL `quality_optimized` or `latency_optimized`.
    pub model: Option<String>,
    /// Text around the texts that helps the translation (not translated, not billed).
    pub context: Option<String>,
}

impl TranslateRequest {
    pub fn new(text: impl Into<String>, target_lang: impl Into<String>) -> TranslateRequest {
        TranslateRequest { texts: vec![text.into()], target_lang: target_lang.into(), ..TranslateRequest::default() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Translation {
    pub text: String,
    pub detected_source_language: Option<String>,
    pub billed_characters: Option<u64>,
}

/// An image to generate (or edit, with `input_image`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImageRequest {
    pub prompt: String,
    /// The service's model; for BFL the endpoint name, e.g. `flux-2-pro`. `None`: the provider's
    /// default model.
    pub model: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub seed: Option<u64>,
    /// `png` (default), `jpeg` or `webp`.
    pub output_format: Option<String>,
    /// An image to edit: base64 or a URL.
    pub input_image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub media_type: String,
    /// Where it was downloaded from (expires after a few minutes).
    pub url: String,
    pub model: String,
    pub cost_usd: Option<f64>,
}
