//! AI client: sends chat completion requests to any OpenAI-compatible endpoint.

use anyhow::{Result, bail};
use reqwest::Client;

use super::models::*;
use crate::config::{Provider, Secrets};

/// AI completion client backed by an OpenAI-compatible API.
pub struct AiClient {
    client: Client,
    endpoint: String,
    api_key: String,
    pub model: String,
}

impl AiClient {
    /// Construct from the active provider settings in secrets.
    pub fn from_secrets(secrets: &Secrets, provider: &Provider) -> Result<Self> {
        let api_key = secrets
            .api_key(provider)
            .filter(|k| !k.is_empty())
            .map(|k| k.to_string())
            .ok_or_else(|| anyhow::anyhow!("No API key set for {}", provider.display_name()))?;

        let endpoint = secrets.endpoint(provider);
        let model = secrets.model(provider).unwrap_or("gpt-4o-mini").to_string();

        Ok(Self {
            client: Client::new(),
            endpoint,
            api_key,
            model,
        })
    }

    /// Send a chat completion request and return the assistant's reply.
    pub async fn complete(&self, messages: Vec<ChatMsg>) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        let request = CompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: Some(4096),
            temperature: Some(0.7),
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error {}: {}", status, body);
        }

        let completion: CompletionResponse = resp.json().await?;

        completion
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| anyhow::anyhow!("Empty response from API"))
    }

    /// List available models from the provider.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.endpoint.trim_end_matches('/'));

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await?
            .error_for_status()?
            .json::<ModelsResponse>()
            .await?;

        let mut ids: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}
