//! `ai.toml`: the providers you connect, their keys and endpoints, and which model each part
//! of SolarOffice uses.
//!
//! Keys are stored in plain text in the settings folder with owner-only permissions (0600), or
//! read from the environment (`ANTHROPIC_API_KEY`, ...) when the file has none.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::ModelInfo;
use crate::error::AiError;
use crate::scope::Scope;
use crate::types::ModelRef;

/// The wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    /// Anthropic Messages API.
    Anthropic,
    /// Gemini `generateContent` (v1beta).
    Gemini,
    /// OpenAI Chat Completions (also Ollama, LM Studio, vLLM, llama.cpp, LocalAI, OpenRouter, ...).
    #[default]
    #[serde(rename = "openai")]
    OpenAiChat,
    /// DeepL's translation API.
    #[serde(rename = "deepl")]
    DeepL,
    /// Black Forest Labs' FLUX image API.
    #[serde(rename = "bfl")]
    Bfl,
}

impl Protocol {
    /// The protocols a custom provider can speak.
    pub const CHAT: [Protocol; 3] = [Protocol::OpenAiChat, Protocol::Anthropic, Protocol::Gemini];

    pub fn label(self) -> &'static str {
        match self {
            Protocol::Anthropic => "Anthropic Messages",
            Protocol::Gemini => "Gemini generateContent",
            Protocol::OpenAiChat => "OpenAI Chat Completions",
            Protocol::DeepL => "DeepL",
            Protocol::Bfl => "FLUX (Black Forest Labs)",
        }
    }
}

/// What a provider does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    /// Language models: chat, writing, code, tools, JSON (Ask AI and the modules).
    Chat,
    /// Machine translation ([`crate::AiHub::translate`]).
    Translate,
    /// Image generation ([`crate::AiHub::generate_image`]).
    Image,
}

/// Where a provider is based, for grouping in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Region {
    /// Europe.
    Eu,
    /// Asia-Pacific.
    Apac,
    /// North America (United States and Canada).
    Us,
    /// On your own machine or network, or a provider you set up yourself.
    Local,
}

impl Region {
    pub const ALL: [Region; 4] = [Region::Eu, Region::Apac, Region::Us, Region::Local];

    pub fn label(self) -> &'static str {
        match self {
            Region::Eu => "Europe (EU)",
            Region::Apac => "Asia-Pacific (APAC)",
            Region::Us => "North America (US/CA)",
            Region::Local => "Local & custom",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Region::Eu => "EU",
            Region::Apac => "APAC",
            Region::Us => "US",
            Region::Local => "Local",
        }
    }
}

/// How a provider wants its key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// `Authorization: Bearer <key>`.
    Bearer,
    /// `<header>: <key>` (Microsoft's `api-key`, BFL's `x-key`).
    Header(&'static str),
    /// `Authorization: DeepL-Auth-Key <key>`.
    DeepL,
}

/// Differences between OpenAI-compatible APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quirks {
    /// Send `max_completion_tokens` instead of `max_tokens`.
    pub max_completion_tokens: bool,
    /// Ask for usage in the stream (`stream_options.include_usage`).
    pub stream_usage: bool,
    /// Structured output as `json_schema`; else `json_object` plus the schema in the prompt.
    pub json_schema: bool,
    /// Supports `tool_choice: "required"` and a named function (else `auto` is sent).
    pub forced_tools: bool,
    /// The model's `reasoning_content` must be sent back with its turns.
    pub send_reasoning: bool,
}

impl Quirks {
    const STANDARD: Quirks = Quirks { max_completion_tokens: false, stream_usage: true, json_schema: true, forced_tools: true, send_reasoning: false };
}

/// The kinds of provider SolarOffice can connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    // Europe
    AlephAlpha,
    #[serde(rename = "bfl")]
    BlackForestLabs,
    #[serde(rename = "deepl")]
    DeepL,
    Mistral,
    // Asia-Pacific
    Alibaba,
    #[serde(rename = "deepseek")]
    DeepSeek,
    Moonshot,
    Sakana,
    #[serde(rename = "zai")]
    Zai,
    // North America
    Anthropic,
    Cohere,
    Gemini,
    Meta,
    Microsoft,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "spacex-ai")]
    SpaceXAi,
    // Local & custom
    #[serde(rename = "lmstudio")]
    LmStudio,
    Ollama,
    /// Any server with an OpenAI-compatible `/v1/chat/completions` (vLLM, llama.cpp, LocalAI,
    /// OpenRouter, Groq, ...).
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
    /// A provider you describe yourself: name, endpoint, protocol, region and country.
    Custom,
}

use ProviderKind as K;

impl ProviderKind {
    /// Every kind, grouped by region in the order Settings shows them.
    pub const ALL: [ProviderKind; 20] = [
        K::AlephAlpha, K::BlackForestLabs, K::DeepL, K::Mistral,
        K::Alibaba, K::DeepSeek, K::Moonshot, K::Sakana, K::Zai,
        K::Anthropic, K::Cohere, K::Gemini, K::Meta, K::Microsoft, K::OpenAi, K::SpaceXAi,
        K::LmStudio, K::Ollama, K::OpenAiCompatible, K::Custom,
    ];

    /// The providers a new `ai.toml` starts with.
    pub const DEFAULTS: [ProviderKind; 5] = [K::Anthropic, K::Gemini, K::OpenAi, K::Ollama, K::LmStudio];

    /// Kinds in `region`, in Settings order.
    pub fn in_region(region: Region) -> Vec<ProviderKind> {
        ProviderKind::ALL.into_iter().filter(|k| k.region() == region).collect()
    }

    pub fn protocol(self) -> Protocol {
        match self {
            K::Anthropic => Protocol::Anthropic,
            K::Gemini => Protocol::Gemini,
            K::DeepL => Protocol::DeepL,
            K::BlackForestLabs => Protocol::Bfl,
            _ => Protocol::OpenAiChat,
        }
    }

    pub fn service(self) -> Service {
        match self {
            K::DeepL => Service::Translate,
            K::BlackForestLabs => Service::Image,
            _ => Service::Chat,
        }
    }

    pub fn region(self) -> Region {
        match self {
            K::AlephAlpha | K::BlackForestLabs | K::DeepL | K::Mistral => Region::Eu,
            K::Alibaba | K::DeepSeek | K::Moonshot | K::Sakana | K::Zai => Region::Apac,
            K::Anthropic | K::Cohere | K::Gemini | K::Meta | K::Microsoft | K::OpenAi | K::SpaceXAi => Region::Us,
            K::LmStudio | K::Ollama | K::OpenAiCompatible | K::Custom => Region::Local,
        }
    }

    /// ISO 3166 country code of the company's home (empty for local and custom).
    pub fn country(self) -> &'static str {
        match self {
            K::AlephAlpha | K::BlackForestLabs | K::DeepL => "DE",
            K::Mistral => "FR",
            K::Alibaba | K::DeepSeek | K::Moonshot | K::Zai => "CN",
            K::Sakana => "JP",
            K::Cohere => "CA",
            K::Anthropic | K::Gemini | K::Meta | K::Microsoft | K::OpenAi | K::SpaceXAi => "US",
            K::LmStudio | K::Ollama | K::OpenAiCompatible | K::Custom => "",
        }
    }

    /// The company name, as listed in Settings.
    pub fn vendor(self) -> &'static str {
        match self {
            K::AlephAlpha => "Aleph Alpha",
            K::BlackForestLabs => "Black Forest Labs",
            K::DeepL => "DeepL",
            K::Mistral => "Mistral AI",
            K::Alibaba => "Alibaba",
            K::DeepSeek => "DeepSeek",
            K::Moonshot => "Moonshot AI",
            K::Sakana => "Sakana AI",
            K::Zai => "Z.AI",
            K::Anthropic => "Anthropic",
            K::Cohere => "Cohere",
            K::Gemini => "Google",
            K::Meta => "Meta",
            K::Microsoft => "Microsoft",
            K::OpenAi => "OpenAI",
            K::SpaceXAi => "SpaceX AI",
            K::LmStudio => "LM Studio",
            K::Ollama => "Ollama",
            K::OpenAiCompatible => "OpenAI-compatible",
            K::Custom => "Provider",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            K::AlephAlpha => "Aleph Alpha (PhariaAI)",
            K::BlackForestLabs => "Black Forest Labs (FLUX)",
            K::DeepL => "DeepL",
            K::Mistral => "Mistral AI",
            K::Alibaba => "Alibaba Cloud (Qwen)",
            K::DeepSeek => "DeepSeek",
            K::Moonshot => "Moonshot AI (Kimi)",
            K::Sakana => "Sakana AI (Fugu)",
            K::Zai => "Z.AI (GLM)",
            K::Anthropic => "Anthropic (Claude)",
            K::Cohere => "Cohere (Command)",
            K::Gemini => "Google Gemini",
            K::Meta => "Meta (Muse)",
            K::Microsoft => "Microsoft Foundry",
            K::OpenAi => "OpenAI",
            K::SpaceXAi => "SpaceX AI (Grok)",
            K::LmStudio => "LM Studio (local)",
            K::Ollama => "Ollama (local)",
            K::OpenAiCompatible => "OpenAI-compatible endpoint",
            K::Custom => "Custom provider",
        }
    }

    /// One line on what the provider offers.
    pub fn blurb(self) -> &'static str {
        match self {
            K::AlephAlpha => "Sovereign AI platform; connect your PhariaAI endpoint",
            K::BlackForestLabs => "FLUX image generation and editing",
            K::DeepL => "Translation and text improvement",
            K::Mistral => "Mistral Large, Medium, Small and Codestral",
            K::Alibaba => "Qwen models on Model Studio (international)",
            K::DeepSeek => "DeepSeek Flash and V4 Pro",
            K::Moonshot => "Kimi K3 models",
            K::Sakana => "Fugu multi-model reasoning and Namazu",
            K::Zai => "GLM models, including free ones",
            K::Anthropic => "Claude Fable, Opus, Sonnet and Haiku",
            K::Cohere => "Command A models for business",
            K::Gemini => "Gemini models through Google AI Studio",
            K::Meta => "Muse models through Meta's Model API",
            K::Microsoft => "MAI and OpenAI models in your Azure resource",
            K::OpenAi => "GPT models",
            K::SpaceXAi => "Grok models (formerly xAI)",
            K::LmStudio => "Models running in LM Studio on this Mac",
            K::Ollama => "Models running in Ollama on this Mac",
            K::OpenAiCompatible => "vLLM, llama.cpp, LocalAI, OpenRouter, ...",
            K::Custom => "Any other provider: choose its protocol and region",
        }
    }

    /// The default id for a provider of this kind in `ai.toml`.
    pub fn default_id(self) -> &'static str {
        match self {
            K::AlephAlpha => "alephalpha",
            K::BlackForestLabs => "bfl",
            K::DeepL => "deepl",
            K::Mistral => "mistral",
            K::Alibaba => "alibaba",
            K::DeepSeek => "deepseek",
            K::Moonshot => "moonshot",
            K::Sakana => "sakana",
            K::Zai => "zai",
            K::Anthropic => "anthropic",
            K::Cohere => "cohere",
            K::Gemini => "gemini",
            K::Meta => "meta",
            K::Microsoft => "microsoft",
            K::OpenAi => "openai",
            K::SpaceXAi => "xai",
            K::LmStudio => "lmstudio",
            K::Ollama => "ollama",
            K::OpenAiCompatible => "custom",
            K::Custom => "provider",
        }
    }

    /// The endpoint; empty when you have to enter your own (Aleph Alpha, Microsoft, custom).
    pub fn default_base_url(self) -> &'static str {
        match self {
            K::AlephAlpha | K::Microsoft | K::Custom => "",
            K::BlackForestLabs => "https://api.bfl.ai",
            K::DeepL => "https://api.deepl.com",
            K::Mistral => "https://api.mistral.ai/v1",
            K::Alibaba => "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            K::DeepSeek => "https://api.deepseek.com",
            K::Moonshot => "https://api.moonshot.ai/v1",
            K::Sakana => "https://api.sakana.ai/v1",
            K::Zai => "https://api.z.ai/api/paas/v4",
            K::Anthropic => "https://api.anthropic.com",
            K::Cohere => "https://api.cohere.ai/compatibility/v1",
            K::Gemini => "https://generativelanguage.googleapis.com",
            K::Meta => "https://api.meta.ai/v1",
            K::OpenAi => "https://api.openai.com/v1",
            K::SpaceXAi => "https://api.x.ai/v1",
            K::LmStudio => "http://localhost:1234/v1",
            K::Ollama => "http://localhost:11434/v1",
            K::OpenAiCompatible => "http://localhost:8000/v1",
        }
    }

    /// An example endpoint, shown when you have to enter your own.
    pub fn endpoint_hint(self) -> &'static str {
        match self {
            K::AlephAlpha => "https://inference-api.your-company.example",
            K::Microsoft => "https://<resource>.openai.azure.com/openai/v1",
            K::Custom => "https://api.example.com/v1",
            _ => self.default_base_url(),
        }
    }

    /// You must enter the endpoint (it's specific to your account or install).
    pub fn needs_endpoint(self) -> bool {
        self.default_base_url().is_empty()
    }

    /// The endpoint can be changed in Settings.
    pub fn endpoint_editable(self) -> bool {
        self.needs_endpoint() || self.is_local()
    }

    /// The environment variables tried for the key, in order.
    pub fn key_env(self) -> &'static [&'static str] {
        match self {
            K::AlephAlpha => &["ALEPH_ALPHA_API_KEY", "PHARIA_TOKEN"],
            K::BlackForestLabs => &["BFL_API_KEY"],
            K::DeepL => &["DEEPL_AUTH_KEY", "DEEPL_API_KEY"],
            K::Mistral => &["MISTRAL_API_KEY"],
            K::Alibaba => &["DASHSCOPE_API_KEY"],
            K::DeepSeek => &["DEEPSEEK_API_KEY"],
            K::Moonshot => &["MOONSHOT_API_KEY"],
            K::Sakana => &["SAKANA_API_KEY"],
            K::Zai => &["ZAI_API_KEY"],
            K::Anthropic => &["ANTHROPIC_API_KEY"],
            K::Cohere => &["COHERE_API_KEY", "CO_API_KEY"],
            K::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
            K::Meta => &["META_API_KEY", "MODEL_API_KEY"],
            K::Microsoft => &["AZURE_OPENAI_API_KEY", "AZURE_INFERENCE_CREDENTIAL"],
            K::OpenAi => &["OPENAI_API_KEY"],
            K::SpaceXAi => &["XAI_API_KEY"],
            K::OpenAiCompatible | K::Ollama | K::LmStudio | K::Custom => &[],
        }
    }

    /// Hosted APIs need a key; local servers don't (an OpenAI-compatible or custom one may).
    pub fn needs_key(self) -> bool {
        !matches!(self, K::Ollama | K::LmStudio | K::OpenAiCompatible | K::Custom)
    }

    /// Runs on your own machine or network.
    pub fn is_local(self) -> bool {
        matches!(self, K::Ollama | K::LmStudio | K::OpenAiCompatible)
    }

    /// Where the models come from: a fixed catalog, or a list you (or the server) supply.
    pub fn lists_own_models(self) -> bool {
        matches!(self, K::Ollama | K::LmStudio | K::OpenAiCompatible | K::Custom | K::AlephAlpha | K::Microsoft)
    }

    pub fn auth(self) -> Auth {
        match self {
            K::Microsoft => Auth::Header("api-key"),
            K::BlackForestLabs => Auth::Header("x-key"),
            K::DeepL => Auth::DeepL,
            _ => Auth::Bearer,
        }
    }

    /// How this kind's OpenAI-compatible API differs from OpenAI's.
    pub fn quirks(self) -> Quirks {
        let q = Quirks::STANDARD;
        match self {
            K::OpenAi | K::Microsoft => Quirks { max_completion_tokens: true, ..q },
            K::Alibaba => Quirks { max_completion_tokens: true, forced_tools: false, ..q },
            K::Moonshot => Quirks { max_completion_tokens: true, send_reasoning: true, ..q },
            K::Zai => Quirks { json_schema: false, forced_tools: false, send_reasoning: true, ..q },
            K::DeepSeek => Quirks { json_schema: false, ..q },
            K::Cohere => Quirks { stream_usage: false, json_schema: false, ..q },
            K::AlephAlpha => Quirks { stream_usage: false, ..q },
            _ => q,
        }
    }

    /// Where to get a key, or how to start the server.
    pub fn help(self) -> &'static str {
        match self {
            K::AlephAlpha => "Aleph Alpha has no public sign-up: enter your company's PhariaAI inference URL and a token from PhariaOS.",
            K::BlackForestLabs => "Create a key at dashboard.bfl.ai › API keys. Images are billed in credits (1 credit = $0.01).",
            K::DeepL => "Create a key at deepl.com › Account › API keys. Free keys (ending in :fx) use api-free.deepl.com automatically.",
            K::Mistral => "Create a key at console.mistral.ai › API keys.",
            K::Alibaba => "Create a key in Alibaba Cloud Model Studio (international, Singapore). New accounts get a free quota.",
            K::DeepSeek => "Create a key at platform.deepseek.com › API keys. Off-peak hours are half price.",
            K::Moonshot => "Create a key at platform.kimi.ai (international keys don't work on the China platform).",
            K::Sakana => "Create a key at console.sakana.ai (it is shown only once).",
            K::Zai => "Create a key at z.ai › API keys. GLM-4.7-Flash and GLM-4.6V-Flash are free.",
            K::Anthropic => "Create a key at platform.claude.com › API keys.",
            K::Cohere => "Create a key at dashboard.cohere.com › API keys.",
            K::Gemini => "Create a key in Google AI Studio (aistudio.google.com › Get API key).",
            K::Meta => "Create a key at dev.meta.ai (Model API).",
            K::Microsoft => "In Microsoft Foundry, deploy a model, then copy the resource endpoint and key. Use your deployment names as models.",
            K::OpenAi => "Create a key at platform.openai.com › API keys.",
            K::SpaceXAi => "Create a key at console.x.ai › API keys.",
            K::OpenAiCompatible => "Any server with /v1/chat/completions; add a key if it needs one.",
            K::Ollama => "Install Ollama, then `ollama pull <model>`; it listens on port 11434.",
            K::LmStudio => "In LM Studio, load a model and start the local server (Developer › Start Server, port 1234).",
            K::Custom => "Enter the provider's endpoint and pick the API it speaks; most providers speak OpenAI Chat Completions.",
        }
    }
}

/// One connected provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Unique id; the first part of model refs (`ollama:llama3.2`).
    pub id: String,
    pub kind: ProviderKind,
    /// A name to show, for custom endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Defaults to the kind's URL (for OpenAI-style APIs, including the `/v1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The key, stored in this file. When empty, the kind's environment variables are tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// The model this provider uses when SolarOffice picks it without a model being chosen
    /// (and the one marked as its default in the pickers). `None`: its recommended model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// Models to offer besides the catalog's and the server's list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    /// Extra HTTP headers (e.g. OpenRouter's `HTTP-Referer`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// For a custom provider: the API it speaks (default OpenAI Chat Completions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Protocol>,
    /// For a custom provider: where it's based (default: Local & custom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// For a custom provider: its country (ISO code, e.g. `FR`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

fn yes() -> bool {
    true
}

impl ProviderConfig {
    pub fn new(kind: ProviderKind) -> ProviderConfig {
        ProviderConfig {
            id: kind.default_id().into(),
            kind,
            name: None,
            enabled: true,
            base_url: None,
            api_key: None,
            default_model: None,
            models: Vec::new(),
            headers: BTreeMap::new(),
            protocol: None,
            region: None,
            country: None,
        }
    }

    pub fn display_name(&self) -> String {
        self.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.kind.label().to_string())
    }

    /// The API it speaks (a custom provider's choice, else its kind's).
    pub fn protocol(&self) -> Protocol {
        match self.kind {
            ProviderKind::Custom => self.protocol.filter(|p| Protocol::CHAT.contains(p)).unwrap_or_default(),
            k => k.protocol(),
        }
    }

    pub fn service(&self) -> Service {
        self.kind.service()
    }

    /// Where it's based (a custom provider's choice, else its kind's).
    pub fn region(&self) -> Region {
        match self.kind {
            ProviderKind::Custom => self.region.unwrap_or(Region::Local),
            k => k.region(),
        }
    }

    /// Its country code, if known.
    pub fn country(&self) -> String {
        match self.kind {
            ProviderKind::Custom => self.country.clone().unwrap_or_default().trim().to_ascii_uppercase(),
            k => k.country().into(),
        }
    }

    pub fn base_url(&self) -> String {
        let set = self.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty());
        let url = match set {
            Some(u) => u,
            // DeepL's free keys end in ":fx" and only work on the free endpoint.
            None if self.kind == ProviderKind::DeepL && self.api_key().is_some_and(|k| k.ends_with(":fx")) => "https://api-free.deepl.com",
            None => self.kind.default_base_url(),
        };
        url.trim_end_matches('/').to_string()
    }

    /// The key: the stored one, else the first environment variable that is set.
    pub fn api_key(&self) -> Option<String> {
        if let Some(k) = self.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
            return Some(k.to_string());
        }
        self.kind.key_env().iter().find_map(|v| std::env::var(v).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()))
    }

    /// Where the key came from, for the settings screen.
    pub fn key_source(&self) -> KeySource {
        if self.api_key.as_deref().is_some_and(|k| !k.trim().is_empty()) {
            return KeySource::Stored;
        }
        match self.kind.key_env().iter().find(|v| std::env::var(v).is_ok_and(|k| !k.trim().is_empty())) {
            Some(v) => KeySource::Env(v),
            None => KeySource::None,
        }
    }

    /// Ready to use: enabled, keyed if the kind needs a key, and with an endpoint.
    pub fn usable(&self) -> bool {
        self.enabled && (!self.kind.needs_key() || self.api_key().is_some()) && !self.base_url().is_empty()
    }

    /// Usable for chat (Ask AI and the modules' language-model features).
    pub fn usable_for_chat(&self) -> bool {
        self.usable() && self.service() == Service::Chat
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Stored,
    Env(&'static str),
    None,
}

/// The whole AI configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// Used when a scope has no model of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<ModelRef>,
    /// Per scope (`home`, `word`, `grid`, ...).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ModelRef>,
    pub providers: Vec<ProviderConfig>,
    /// Extra or corrected catalog entries (prices, limits).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub catalog: Vec<ModelInfo>,
    /// Request timeout until the first byte, in seconds (local models can take a while to load).
    pub first_byte_timeout_secs: u64,
    /// Retries for rate limits, overloads and network errors.
    pub max_retries: u32,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            default_model: None,
            models: BTreeMap::new(),
            providers: ProviderKind::DEFAULTS.iter().map(|k| ProviderConfig::new(*k)).collect(),
            catalog: Vec::new(),
            first_byte_timeout_secs: 300,
            max_retries: 3,
        }
    }
}

impl AiConfig {
    pub fn file(dir: &Path) -> PathBuf {
        dir.join("ai.toml")
    }

    /// Load `ai.toml` from `dir`, or the defaults when there is none. A damaged file is kept as
    /// `ai.toml.bad`.
    pub fn load(dir: &Path) -> AiConfig {
        let path = AiConfig::file(dir);
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<AiConfig>(&text) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("{}: {e}; using defaults", path.display());
                    let _ = std::fs::rename(&path, path.with_extension("toml.bad"));
                    AiConfig::default()
                }
            },
            Err(_) => AiConfig::default(),
        }
    }

    /// Save to `dir/ai.toml`, readable by the owner only.
    pub fn save(&self, dir: &Path) -> Result<(), AiError> {
        std::fs::create_dir_all(dir).map_err(|e| AiError::Config(e.to_string()))?;
        let text = format!(
            "# SolarOffice AI settings. Edit in Settings › AI providers / AI models, or here.\n# API keys are stored in plain text; this file is readable by you only.\n\n{}",
            toml::to_string_pretty(self).map_err(|e| AiError::Config(e.to_string()))?
        );
        let tmp = dir.join(format!(".ai.toml.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text).map_err(|e| AiError::Config(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, AiConfig::file(dir)).map_err(|e| AiError::Config(e.to_string()))
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn provider_mut(&mut self, id: &str) -> Option<&mut ProviderConfig> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    /// A free provider id based on `base` (`custom`, `custom-2`, ...).
    pub fn free_id(&self, base: &str) -> String {
        if self.provider(base).is_none() {
            return base.into();
        }
        (2..).map(|i| format!("{base}-{i}")).find(|id| self.provider(id).is_none()).unwrap_or_else(|| base.into())
    }

    /// The model a scope uses when the request names none: the scope's choice, else the
    /// default, else the first usable provider's own default model (see
    /// [`ProviderConfig::default_model`]), else its first listed or recommended model.
    /// `catalog_first` gives a provider's recommended (or first known) model.
    pub fn model_for(&self, scope: Scope, catalog_first: impl Fn(&ProviderConfig) -> Option<String>) -> Option<ModelRef> {
        let usable = |m: &ModelRef| self.provider(&m.provider).is_some_and(|p| p.usable_for_chat());
        if let Some(m) = self.models.get(scope.id()).filter(|m| usable(m)) {
            return Some(m.clone());
        }
        if let Some(m) = self.default_model.as_ref().filter(|m| usable(m)) {
            return Some(m.clone());
        }
        self.providers.iter().filter(|p| p.usable_for_chat()).find_map(|p| {
            let model = p.default_model.clone().filter(|m| !m.trim().is_empty()).or_else(|| p.models.first().cloned()).or_else(|| catalog_first(p))?;
            Some(ModelRef::new(p.id.clone(), model))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_hosted_and_local() {
        let c = AiConfig::default();
        let kinds: Vec<_> = c.providers.iter().map(|p| p.kind).collect();
        for k in [ProviderKind::Anthropic, ProviderKind::Gemini, ProviderKind::OpenAi, ProviderKind::Ollama, ProviderKind::LmStudio] {
            assert!(kinds.contains(&k));
        }
        assert_eq!(c.provider("ollama").unwrap().base_url(), "http://localhost:11434/v1");
        assert!(c.provider("lmstudio").unwrap().usable(), "local servers need no key");
    }

    #[test]
    fn round_trip_and_permissions() {
        let d = tempfile::tempdir().unwrap();
        let mut c = AiConfig::default();
        c.provider_mut("anthropic").unwrap().api_key = Some("sk-test".into());
        c.default_model = ModelRef::parse("anthropic:claude-sonnet-5-5");
        c.models.insert("grid".into(), ModelRef::parse("ollama:qwen3:8b").unwrap());
        let mut custom = ProviderConfig::new(ProviderKind::OpenAiCompatible);
        custom.id = c.free_id("custom");
        custom.base_url = Some("https://openrouter.ai/api/v1/".into());
        custom.headers.insert("HTTP-Referer".into(), "https://example.org".into());
        c.providers.push(custom);
        c.save(d.path()).unwrap();
        let back = AiConfig::load(d.path());
        assert_eq!(back, c);
        assert_eq!(back.provider("custom").unwrap().base_url(), "https://openrouter.ai/api/v1");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(AiConfig::file(d.path())).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        std::fs::write(AiConfig::file(d.path()), "providers = 5").unwrap();
        assert_eq!(AiConfig::load(d.path()), AiConfig::default());
        assert!(d.path().join("ai.toml.bad").exists());
    }

    #[test]
    fn model_resolution_order() {
        let mut c = AiConfig::default();
        for p in &mut c.providers {
            p.enabled = false;
        }
        let first = |p: &ProviderConfig| Some(format!("{}-best", p.id));
        assert_eq!(c.model_for(Scope::Home, first), None);
        c.provider_mut("lmstudio").unwrap().enabled = true;
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("lmstudio:lmstudio-best"));
        c.default_model = ModelRef::parse("lmstudio:qwen");
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("lmstudio:qwen"));
        c.models.insert("word".into(), ModelRef::parse("lmstudio:writer").unwrap());
        assert_eq!(c.model_for(Scope::App(solar_core::AppKind::Word), first), ModelRef::parse("lmstudio:writer"));
        assert_eq!(c.model_for(Scope::App(solar_core::AppKind::Grid), first), ModelRef::parse("lmstudio:qwen"));
        // A scope model on a disabled provider falls back.
        c.models.insert("grid".into(), ModelRef::parse("ollama:x").unwrap());
        assert_eq!(c.model_for(Scope::App(solar_core::AppKind::Grid), first), ModelRef::parse("lmstudio:qwen"));
    }

    #[test]
    fn readme_example_parses() {
        let c: AiConfig = toml::from_str(
            r#"
default_model = "anthropic:claude-sonnet-5-5"

[models]
grid = "ollama:qwen3:8b"
draw = "gemini:gemini-3.8-flash"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key = "sk-ant-x"

[[providers]]
id = "openrouter"
kind = "openai-compatible"
base_url = "https://openrouter.ai/api/v1"
api_key = "sk-or-x"
"#,
        )
        .unwrap();
        assert_eq!(c.models["grid"], ModelRef::new("ollama", "qwen3:8b"));
        assert_eq!(c.provider("openrouter").unwrap().kind, ProviderKind::OpenAiCompatible);
        assert_eq!(c.max_retries, 3, "unspecified fields keep their defaults");
    }

    #[test]
    fn provider_default_models() {
        let mut c = AiConfig::default();
        for p in &mut c.providers {
            p.enabled = p.id == "ollama";
        }
        let first = |p: &ProviderConfig| Some(format!("{}-recommended", p.id));
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("ollama:ollama-recommended"));
        c.provider_mut("ollama").unwrap().models = vec!["llama3.2:3b".into(), "qwen3:8b".into()];
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("ollama:llama3.2:3b"));
        c.provider_mut("ollama").unwrap().default_model = Some("qwen3:8b".into());
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("ollama:qwen3:8b"), "the provider's default wins");
        c.default_model = ModelRef::parse("ollama:llama3.2:3b");
        assert_eq!(c.model_for(Scope::Home, first), ModelRef::parse("ollama:llama3.2:3b"), "an explicit default model wins over it");
        let d = tempfile::tempdir().unwrap();
        c.save(d.path()).unwrap();
        assert_eq!(AiConfig::load(d.path()).provider("ollama").unwrap().default_model.as_deref(), Some("qwen3:8b"));
    }

    #[test]
    fn regions_and_vendors() {
        use ProviderKind as K;
        let names = |r| ProviderKind::in_region(r).iter().map(|k| k.vendor()).collect::<Vec<_>>();
        assert_eq!(names(Region::Eu), ["Aleph Alpha", "Black Forest Labs", "DeepL", "Mistral AI"]);
        assert_eq!(names(Region::Apac), ["Alibaba", "DeepSeek", "Moonshot AI", "Sakana AI", "Z.AI"]);
        assert_eq!(names(Region::Us), ["Anthropic", "Cohere", "Google", "Meta", "Microsoft", "OpenAI", "SpaceX AI"]);
        assert_eq!(names(Region::Local), ["LM Studio", "Ollama", "OpenAI-compatible", "Provider"]);
        assert_eq!(K::Cohere.country(), "CA");
        assert_eq!(K::Sakana.country(), "JP");
        let ids: std::collections::HashSet<_> = ProviderKind::ALL.iter().map(|k| k.default_id()).collect();
        assert_eq!(ids.len(), ProviderKind::ALL.len(), "default ids are unique");
        for k in ProviderKind::ALL {
            assert!(k.needs_endpoint() || k.default_base_url().starts_with("http"), "{k:?}");
            let toml = toml::to_string(&ProviderConfig::new(k)).unwrap();
            assert_eq!(toml::from_str::<ProviderConfig>(&toml).unwrap().kind, k, "{toml}");
        }
        assert_eq!(K::DeepL.service(), Service::Translate);
        assert_eq!(K::BlackForestLabs.service(), Service::Image);
        assert_eq!(K::Microsoft.auth(), Auth::Header("api-key"));
    }

    #[test]
    fn custom_providers_and_endpoints() {
        let mut p = ProviderConfig::new(ProviderKind::Custom);
        assert!(!p.usable(), "no endpoint yet");
        p.base_url = Some("https://api.example.eu/v1".into());
        p.protocol = Some(Protocol::Anthropic);
        p.region = Some(Region::Eu);
        p.country = Some("nl".into());
        assert!(p.usable());
        assert_eq!((p.protocol(), p.region(), p.country().as_str()), (Protocol::Anthropic, Region::Eu, "NL"));
        p.protocol = Some(Protocol::DeepL);
        assert_eq!(p.protocol(), Protocol::OpenAiChat, "custom providers speak a chat protocol");

        let mut d = ProviderConfig::new(ProviderKind::DeepL);
        d.api_key = Some("abc:fx".into());
        assert_eq!(d.base_url(), "https://api-free.deepl.com");
        d.api_key = Some("abc".into());
        assert_eq!(d.base_url(), "https://api.deepl.com");
        assert!(d.usable() && !d.usable_for_chat());

        let mut c = AiConfig::default();
        c.providers.push(d);
        for p in &mut c.providers {
            p.enabled = p.id == "deepl";
        }
        assert_eq!(c.model_for(Scope::Home, |_| Some("x".into())), None, "a translation service never answers chats");
    }

    #[test]
    fn keys_from_file_or_env() {
        let mut p = ProviderConfig::new(ProviderKind::OpenAiCompatible);
        assert_eq!(p.key_source(), KeySource::None);
        assert!(p.usable(), "a custom endpoint may need no key");
        p.api_key = Some("  abc ".into());
        assert_eq!(p.api_key().as_deref(), Some("abc"));
        assert_eq!(p.key_source(), KeySource::Stored);
    }
}
