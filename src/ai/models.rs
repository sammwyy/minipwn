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
    /// When true, the server streams the response as SSE chunks.
    pub stream: bool,
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

/// A piece of a streaming response, tagged so the UI can render the model's
/// "thinking" differently from the actual answer.
#[derive(Debug, Clone)]
pub enum StreamPiece {
    /// Reasoning/thinking tokens — shown as live progress, not the final answer.
    Reasoning(String),
    /// Answer text — the actual reply content.
    Answer(String),
}

/// A single SSE chunk from a streaming chat completion.
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
}

/// The incremental delta carried by a streaming chunk.
#[derive(Debug, Default, Deserialize)]
pub struct StreamDelta {
    #[serde(default)]
    pub content: Option<String>,
    /// Reasoning tokens emitted by "thinking" models before the final answer.
    #[serde(default)]
    pub reasoning_content: Option<String>,
}
