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
    pub fn from_secrets(secrets: &Secrets, provider: &dyn Provider) -> Result<Self> {
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

    /// Send a streaming chat completion request.
    ///
    /// Display deltas (reasoning, then answer text) are pushed to `deltas` as
    /// they arrive so the UI can show progress. The returned String is the
    /// final *answer* content only (reasoning excluded) — that is what tool-call
    /// parsing and history persistence operate on.
    pub async fn complete_stream(
        &self,
        messages: Vec<ChatMsg>,
        deltas: tokio::sync::mpsc::UnboundedSender<StreamPiece>,
    ) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        let request = CompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            stream: true,
        };

        let mut resp = self
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

        let mut answer = String::new();
        let mut buf: Vec<u8> = Vec::new();

        // Server-Sent Events: one `data: {json}` per line, terminated by `\n\n`.
        while let Some(chunk) = resp.chunk().await? {
            buf.extend_from_slice(&chunk);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&raw);
                let line = line.trim();

                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) else {
                    continue;
                };
                let Some(choice) = chunk.choices.into_iter().next() else {
                    continue;
                };

                if let Some(reasoning) = choice.delta.reasoning_content {
                    if !reasoning.is_empty() {
                        let _ = deltas.send(StreamPiece::Reasoning(reasoning));
                    }
                }
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        answer.push_str(&content);
                        let _ = deltas.send(StreamPiece::Answer(content));
                    }
                }
            }
        }

        Ok(answer)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ChatMsg;
    use crate::config::{OpenCode, Secrets};

    // Live check against OpenCode Zen. Ignored by default (needs network + a
    // key). Run with:
    //   OPENCODE_TEST_KEY=sk-... cargo test --bin minipwn -- --ignored opencode_live --nocapture
    #[tokio::test]
    #[ignore]
    async fn opencode_live() {
        let key = std::env::var("OPENCODE_TEST_KEY").expect("set OPENCODE_TEST_KEY");
        let mut secrets = Secrets::default();
        secrets
            .values
            .insert("OPENCODE_SECRETKEY".into(), key);
        secrets
            .values
            .insert("OPENCODE_MODEL".into(), "glm-5".into());

        let client = AiClient::from_secrets(&secrets, &OpenCode).expect("client");
        assert_eq!(client.model, "glm-5");

        let models = client.list_models().await.expect("list_models");
        assert!(models.iter().any(|m| m == "glm-5"), "models: {:?}", models);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let pieces = tokio::spawn(async move {
            let mut n = 0;
            while rx.recv().await.is_some() {
                n += 1;
            }
            n
        });
        let reply = client
            .complete_stream(
                vec![ChatMsg {
                    role: "user".into(),
                    content: "Reply with exactly one word: pong".into(),
                }],
                tx,
            )
            .await
            .expect("complete_stream");
        assert!(!reply.trim().is_empty(), "empty reply");
        assert!(pieces.await.unwrap() > 0, "expected streamed pieces");
        println!("opencode streamed reply = {:?}", reply);
    }
}
