//! Tool executor: routes tool calls to local or remote worker.

use std::path::PathBuf;

use super::fs_tools::execute_fs_tool;
use super::shell_tools::execute_shell_tool_local;
use super::{ToolCall, ToolResult};
use crate::worker::client::WorkerClient;

/// Whether to execute tools locally or via a remote worker.
pub enum ExecutionMode {
    Local {
        workspace: PathBuf,
    },
    Remote {
        client: WorkerClient,
        workspace: PathBuf,
    },
}

impl ExecutionMode {
    /// Execute a tool call, routing fs tools to local workspace and shell tools to worker/local.
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        let workspace = match self {
            ExecutionMode::Local { workspace } => workspace,
            ExecutionMode::Remote { workspace, .. } => workspace,
        };

        match call.tool.as_str() {
            "fs_ls" | "fs_read" | "fs_write" | "fs_mkdir" | "fs_rm" | "fs_copy" | "fs_mv" => {
                execute_fs_tool(call, workspace)
            }
            "shell_exec" | "shell_open" | "shell_write" | "shell_read" | "shell_close" => {
                match self {
                    ExecutionMode::Remote { client, .. } => {
                        client.execute_tool(call).await.unwrap_or_else(|e| {
                            ToolResult::err(&call.tool, format!("Worker error: {}", e))
                        })
                    }
                    ExecutionMode::Local { .. } => execute_shell_tool_local(call),
                }
            }
            _ => ToolResult::err(&call.tool, format!("Unknown tool: {}", call.tool)),
        }
    }
}
