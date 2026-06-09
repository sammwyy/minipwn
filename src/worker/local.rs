//! Local ("no worker") sandbox: runs every tool directly on the host machine.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::tools::{
    ToolCall, ToolResult, execute_fs_tool, execute_shell_tool_local, is_fs_tool, is_shell_tool,
};

use super::{Worker, WorkerKind};

/// Executes filesystem and shell tools on the current machine, scoped to a
/// workspace directory for filesystem access.
pub struct LocalWorker {
    workspace: PathBuf,
}

impl LocalWorker {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Worker for LocalWorker {
    fn kind(&self) -> WorkerKind {
        WorkerKind::Local
    }

    fn display_name(&self) -> String {
        "Local (no worker)".to_string()
    }

    async fn execute(&self, call: &ToolCall) -> ToolResult {
        if is_fs_tool(&call.tool) {
            execute_fs_tool(call, &self.workspace)
        } else if is_shell_tool(&call.tool) {
            execute_shell_tool_local(call)
        } else {
            ToolResult::err(&call.tool, format!("Unknown tool: {}", call.tool))
        }
    }

    async fn system_info(&self) -> String {
        format!(
            "Local execution: {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    }
}
