//! Shell tool implementations (local execution; remote execution goes via worker client).

use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

use super::{ToolCall, ToolResult};

/// A running shell session.
struct ShellSession {
    child: Child,
}

/// Thread-safe registry of open shell sessions.
#[derive(Default, Clone)]
pub struct ShellRegistry {
    sessions: Arc<Mutex<HashMap<String, ShellSession>>>,
}

impl ShellRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Execute a one-shot shell command and return its combined output.
pub fn shell_exec_local(command: &str) -> ToolResult {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", command]).output()
    } else {
        Command::new("sh").args(["-c", command]).output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let status = out.status.code().unwrap_or(-1);
            let combined = if stderr.is_empty() {
                format!("exit={}\n{}", status, stdout)
            } else {
                format!("exit={}\nstdout:\n{}\nstderr:\n{}", status, stdout, stderr)
            };
            ToolResult::ok("shell_exec", combined.trim().to_string())
        }
        Err(e) => ToolResult::err("shell_exec", e.to_string()),
    }
}

/// Dispatch a shell tool call for local execution.
pub fn execute_shell_tool_local(call: &ToolCall) -> ToolResult {
    match call.tool.as_str() {
        "shell_exec" => {
            let cmd = call
                .args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            shell_exec_local(cmd)
        }
        _ => ToolResult::err(&call.tool, "Interactive shell sessions require a worker"),
    }
}
