//! Core worker abstraction shared by the local, remote, and docker backends.

use async_trait::async_trait;

use crate::tools::{ToolCall, ToolResult};

/// Identifies which kind of backend a [`Worker`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerKind {
    /// Runs tools directly on the host machine ("no worker").
    Local,
    /// Talks to a worker server deployed inside a Docker container.
    Docker,
    /// Talks to a standalone remote worker server.
    Remote,
}

impl WorkerKind {
    /// Short uppercase label, handy for the status bar.
    pub fn label(&self) -> &'static str {
        match self {
            WorkerKind::Local => "LOCAL",
            WorkerKind::Docker => "DOCKER",
            WorkerKind::Remote => "REMOTE",
        }
    }
}

/// An execution backend for agent tool calls.
///
/// Filesystem tools always run against the local workspace; shell tools are
/// routed to wherever the worker actually lives (the host, a container, or a
/// remote box). Each concrete worker decides how to format itself for the UI
/// via [`Worker::display_name`] and [`Worker::status_label`].
#[async_trait]
pub trait Worker: Send + Sync {
    /// Which backend this worker represents.
    fn kind(&self) -> WorkerKind;

    /// Human-friendly name used in the TUI (status messages, `/worker`).
    fn display_name(&self) -> String;

    /// Compact label for the status bar, e.g. `◈ LOCAL` or `◈ REMOTE (host)`.
    fn status_label(&self) -> String {
        format!("◈ {}", self.kind().label())
    }

    /// Execute a single tool call and return its result.
    async fn execute(&self, call: &ToolCall) -> ToolResult;

    /// One-line description of the execution environment for the system prompt.
    async fn system_info(&self) -> String;
}

/// Extract the bare host (no scheme or port) from a worker URL for display.
pub(crate) fn host_of(url: &str) -> &str {
    url.trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .unwrap_or(url)
}
