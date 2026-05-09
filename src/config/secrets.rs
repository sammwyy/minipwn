//! Read/write secrets.env for API keys and endpoints.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::paths::secrets_env_path;

/// Provider identifier.
#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    OpenAI,
    OpenRouter,
    Custom,
}

impl Provider {
    pub fn prefix(&self) -> &'static str {
        match self {
            Provider::OpenAI => "OPENAI",
            Provider::OpenRouter => "OPENROUTER",
            Provider::Custom => "CUSTOM",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(Provider::OpenAI),
            "openrouter" => Some(Provider::OpenRouter),
            "custom" => Some(Provider::Custom),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::OpenAI => "OpenAI",
            Provider::OpenRouter => "OpenRouter",
            Provider::Custom => "Custom",
        }
    }

    pub fn default_endpoint(&self) -> &'static str {
        match self {
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
            Provider::Custom => "",
        }
    }
}

/// Parsed secrets from secrets.env.
#[derive(Debug, Default)]
pub struct Secrets {
    pub values: HashMap<String, String>,
}

impl Secrets {
    /// Load secrets from the global secrets.env file.
    pub fn load() -> Result<Self> {
        let path = secrets_env_path()?;
        Self::load_from(&path)
    }

    /// Load secrets from a specific path.
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let mut values = HashMap::new();
        if !path.exists() {
            return Ok(Self { values });
        }
        let content = std::fs::read_to_string(path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let val = val.trim_matches('"');
                values.insert(key.trim().to_string(), val.to_string());
            }
        }
        Ok(Self { values })
    }

    /// Get a value for a given key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Set a value and persist to disk.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.values.insert(key.to_string(), value.to_string());
        self.save()
    }

    /// Get provider-specific secret key.
    pub fn api_key(&self, provider: &Provider) -> Option<&str> {
        self.get(&format!("{}_SECRETKEY", provider.prefix()))
    }

    /// Get provider-specific endpoint.
    pub fn endpoint(&self, provider: &Provider) -> String {
        self.get(&format!("{}_ENDPOINT", provider.prefix()))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| provider.default_endpoint().to_string())
    }

    /// Get provider-specific model.
    pub fn model(&self, provider: &Provider) -> Option<&str> {
        self.get(&format!("{}_MODEL", provider.prefix()))
            .filter(|s| !s.is_empty())
    }

    /// Save all secrets back to disk in KEY="VALUE" format.
    fn save(&self) -> Result<()> {
        let path = secrets_env_path()?;
        let mut lines = Vec::new();
        for (k, v) in &self.values {
            lines.push(format!("{}=\"{}\"", k, v));
        }
        lines.sort();
        std::fs::write(path, lines.join("\n") + "\n")?;
        Ok(())
    }
}

/// Censor an API key for display: show first 4 and last 4 chars.
pub fn censor_key(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}
