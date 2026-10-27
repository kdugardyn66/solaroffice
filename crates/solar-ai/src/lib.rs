#![allow(clippy::expect_used)] // lock poisoning and the built-in catalog are programming errors
//! # solar-ai — SolarOffice's AI layer
//!
//! One API over every model SolarOffice can use, replacing GenOffice's built-in GenSpark service:
//!
//! - **Hosted:** Anthropic (Messages API), Google Gemini (`generateContent`), OpenAI (Chat
//!   Completions).
//! - **Hosted, by region:** Europe (Aleph Alpha, Mistral AI), Asia-Pacific (Alibaba Qwen,
//!   DeepSeek, Moonshot Kimi, Sakana AI, Z.AI GLM) and North America (Anthropic, Cohere, Google
//!   Gemini, Meta Muse, Microsoft Foundry, OpenAI, SpaceX AI Grok); see [`ProviderKind`] and
//!   [`Region`].
//! - **Translation and images:** DeepL ([`AiHub::translate`]) and Black Forest Labs' FLUX
//!   ([`AiHub::generate_image`]).
//! - **Local / self-hosted:** Ollama, LM Studio, any OpenAI-compatible endpoint (vLLM,
//!   llama.cpp server, LocalAI, OpenRouter, Groq, ...) and custom providers speaking the OpenAI,
//!   Anthropic or Gemini protocol. Add as many as you like.
//!
//! Every model in the catalog lists what it's good at ([`Category`]: Reason, Chat, Write,
//! Translate, Code, Create, Listen, Speak, See, Predict; at most five per model).
//!
//! Everything streams; requests can carry images and PDFs, tools (with an agent loop in
//! [`AiHub::run_tools`]) and a JSON schema for structured output. Rate limits, overloads and
//! network errors are retried; errors come back as one [`AiError`]; usage and an estimated cost
//! come with every [`Response`].
//!
//! ## Modules
//!
//! Each module asks through its [`Scope`] (`hub.scoped(Scope::App(AppKind::Grid))`): the scope
//! picks the module's model (Settings › AI lets you choose one per module), adds instructions for
//! the module's job and output format ([`OutputFormat`]), and lists ready-made actions
//! ([`tasks::tasks_for`]). Structured outputs a module imports live in [`schemas`].
//!
//! Configuration is `ai.toml` in the SolarOffice settings folder ([`AiConfig`]).

pub mod catalog;
pub mod config;
pub mod error;
pub mod http;
pub mod hub;
pub mod media;
pub mod providers;
pub mod schemas;
pub mod scope;
pub mod tasks;
pub mod types;

pub use catalog::{Catalog, Category, ModelInfo};
pub use config::{AiConfig, KeySource, Protocol, ProviderConfig, ProviderKind, Region, Service};
pub use media::{GeneratedImage, ImageRequest, TranslateRequest, Translation};
pub use error::AiError;
pub use http::Cancel;
pub use hub::{AiHub, Job, JobEvent, Notify, ScopedAi};
pub use scope::{OutputFormat, Scope};
pub use types::{JsonOutput, Message, ModelRef, Part, Request, Response, Role, StopReason, ToolCall, ToolChoice, ToolSpec, Usage};
