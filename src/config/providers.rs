//! AI provider definitions.
//!
//! Each provider is a small struct implementing [`Provider`]. They all speak
//! the OpenAI-compatible chat-completions protocol (see [`crate::ai`]); a
//! provider only contributes its identity, env-var prefix, and default
//! endpoint. Adding a provider is therefore just a new struct + a line in
//! [`all_providers`] / [`provider_from_id`].

/// A configured AI provider.
pub trait Provider: Send + Sync {
    /// Stable lowercase id used in config files and commands (e.g. `openai`).
    fn id(&self) -> &'static str;

    /// Human-friendly name shown in the UI.
    fn display_name(&self) -> &'static str;

    /// Uppercase prefix for this provider's secrets keys (e.g. `OPENAI` →
    /// `OPENAI_SECRETKEY`, `OPENAI_ENDPOINT`, `OPENAI_MODEL`).
    fn prefix(&self) -> &'static str;

    /// Default API base endpoint used when none is configured.
    fn default_endpoint(&self) -> &'static str;
}

/// OpenAI's API.
pub struct OpenAi;
/// OpenRouter aggregator.
pub struct OpenRouter;
/// OpenCode Zen (OpenAI-compatible chat completions).
pub struct OpenCode;
/// User-defined OpenAI-compatible endpoint.
pub struct Custom;

impl Provider for OpenAi {
    fn id(&self) -> &'static str {
        "openai"
    }
    fn display_name(&self) -> &'static str {
        "OpenAI"
    }
    fn prefix(&self) -> &'static str {
        "OPENAI"
    }
    fn default_endpoint(&self) -> &'static str {
        "https://api.openai.com/v1"
    }
}

impl Provider for OpenRouter {
    fn id(&self) -> &'static str {
        "openrouter"
    }
    fn display_name(&self) -> &'static str {
        "OpenRouter"
    }
    fn prefix(&self) -> &'static str {
        "OPENROUTER"
    }
    fn default_endpoint(&self) -> &'static str {
        "https://openrouter.ai/api/v1"
    }
}

impl Provider for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn display_name(&self) -> &'static str {
        "OpenCode Zen"
    }
    fn prefix(&self) -> &'static str {
        "OPENCODE"
    }
    fn default_endpoint(&self) -> &'static str {
        "https://opencode.ai/zen/go/v1"
    }
}

impl Provider for Custom {
    fn id(&self) -> &'static str {
        "custom"
    }
    fn display_name(&self) -> &'static str {
        "Custom"
    }
    fn prefix(&self) -> &'static str {
        "CUSTOM"
    }
    fn default_endpoint(&self) -> &'static str {
        ""
    }
}

/// All known providers, in display order (used by the picker).
pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(OpenAi),
        Box::new(OpenRouter),
        Box::new(OpenCode),
        Box::new(Custom),
    ]
}

/// Resolve a provider by its [`Provider::id`].
pub fn provider_from_id(id: &str) -> Option<Box<dyn Provider>> {
    all_providers()
        .into_iter()
        .find(|p| p.id() == id.to_lowercase())
}

/// The provider used when configuration is missing or invalid.
pub fn default_provider() -> Box<dyn Provider> {
    Box::new(OpenAi)
}
