//! Remote worker: routes shell tools to a standalone worker server over HTTP.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::tools::{ToolCall, ToolResult, execute_fs_tool, is_fs_tool, is_shell_tool};

use super::client::WorkerClient;
use super::traits::host_of;
use super::{Worker, WorkerKind};

/// A worker backed by a remote MiniPWN server.
///
/// Filesystem tools still run against the local workspace; shell tools are
/// forwarded to the remote server via [`WorkerClient`].
pub struct RemoteWorker {
    name: String,
    workspace: PathBuf,
    client: WorkerClient,
}

impl RemoteWorker {
    pub fn new(name: impl Into<String>, url: &str, secret: &str, workspace: PathBuf) -> Self {
        Self {
            name: name.into(),
            workspace,
            client: WorkerClient::new(url, secret),
        }
    }

    /// Borrow the underlying HTTP client (e.g. to validate before connecting).
    pub fn client(&self) -> &WorkerClient {
        &self.client
    }

    /// Base URL of the remote worker server.
    pub fn base_url(&self) -> &str {
        &self.client.base_url
    }
}

#[async_trait]
impl Worker for RemoteWorker {
    fn kind(&self) -> WorkerKind {
        WorkerKind::Remote
    }

    fn display_name(&self) -> String {
        format!("{} ({})", self.name, self.client.base_url)
    }

    fn status_label(&self) -> String {
        format!("◈ {} ({})", self.kind().label(), host_of(&self.client.base_url))
    }

    async fn execute(&self, call: &ToolCall) -> ToolResult {
        if is_fs_tool(&call.tool) {
            execute_fs_tool(call, &self.workspace)
        } else if is_shell_tool(&call.tool) {
            self.client
                .execute_tool(call)
                .await
                .unwrap_or_else(|e| ToolResult::err(&call.tool, format!("Worker error: {}", e)))
        } else {
            ToolResult::err(&call.tool, format!("Unknown tool: {}", call.tool))
        }
    }

    async fn system_info(&self) -> String {
        match self.client.get_info().await {
            Ok(info) => format!(
                "Remote worker: {} ({}), hostname: {}, cwd: {}",
                info.os, info.arch, info.hostname, info.cwd
            ),
            Err(_) => "Remote worker (info unavailable)".to_string(),
        }
    }
}
