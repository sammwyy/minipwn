//! The UI surface the agent loop drives while a turn runs.

use anyhow::Result;
use async_trait::async_trait;

use crate::ai::StreamPiece;

/// How [`super::run_turn`] reports progress to the outside world.
///
/// The agent never touches the terminal directly: it only calls these hooks.
/// The TUI provides a ratatui-backed implementation, but a test (or a future
/// headless frontend) can provide its own — which is what makes the agentic
/// loop reusable and testable in isolation.
#[async_trait]
pub trait AgentUi {
    /// Append an assistant message. `ephemeral` messages are not persisted.
    fn assistant(&mut self, text: String, ephemeral: bool);

    /// Begin a live, streaming assistant bubble; returns a handle to it.
    fn stream_begin(&mut self) -> usize;

    /// Append a streamed piece (reasoning or answer) to a streaming bubble.
    fn stream_push(&mut self, handle: usize, piece: &StreamPiece);

    /// Finish a streaming bubble: replace it with `text`, or remove it when
    /// `None` (e.g. the message was only a tool call with no prose).
    fn stream_end(&mut self, handle: usize, text: Option<String>);

    /// Append a tool bubble in its "running" state, returning a handle to it.
    fn tool_begin(&mut self, content: String) -> usize;

    /// Replace the content of a tool bubble started by [`AgentUi::tool_begin`].
    fn tool_update(&mut self, handle: usize, content: String);

    /// Finalize a tool execution with structured raw output.
    fn tool_finish(&mut self, _handle: usize, _cmd: String, _success: bool, _output: String) {}

    /// Record estimated token usage for the current turn.
    fn record_tokens(&mut self, count: u64);

    /// Toggle the "thinking" indicator.
    fn set_thinking(&mut self, thinking: bool);

    /// Set a transient status-line message.
    fn set_status(&mut self, status: String);

    /// Render the current state.
    fn redraw(&mut self) -> Result<()>;

    /// Process any pending user input (typing queues messages, slash commands
    /// run immediately). Returns `true` if the user requested cancellation of
    /// the in-flight generation (Esc / Ctrl-C).
    async fn poll_input(&mut self) -> Result<bool>;
}
