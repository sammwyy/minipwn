//! OpenAI-compatible API request/response types.

use serde::{Deserialize, Serialize};

/// A message in a chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

/// Chat completion request body.
#[derive(Debug, Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMsg>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Chat completion response.
#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
pub struct CompletionChoice {
    pub message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
pub struct ChoiceMessage {
    pub content: Option<String>,
}

/// A model entry from the /models endpoint.
#[derive(Debug, Deserialize)]
pub struct ModelEntry {
    pub id: String,
}

/// Response from the /models endpoint.
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelEntry>,
}
