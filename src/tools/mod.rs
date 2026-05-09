//! Tool definitions and execution routing.
//! Tools are invoked by the AI agent via JSON tool-call responses.

mod executor;
mod fs_tools;
mod sanitize;
mod shell_tools;

pub use executor::*;

use serde::{Deserialize, Serialize};

/// A tool invocation parsed from AI output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub args: serde_json::Value,
}

/// Result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub output: String,
}

impl ToolResult {
    pub fn ok(tool: &str, output: impl Into<String>) -> Self {
        Self {
            tool: tool.to_string(),
            success: true,
            output: output.into(),
        }
    }

    pub fn err(tool: &str, error: impl Into<String>) -> Self {
        Self {
            tool: tool.to_string(),
            success: false,
            output: format!("Error: {}", error.into()),
        }
    }
}

/// Try to extract a tool call from an AI message content.
/// Looks for ```json ... ``` blocks containing a "tool" field.
pub fn extract_tool_call(content: &str) -> Option<ToolCall> {
    // Find ```json ... ``` blocks
    let start = content.find("```json")?;
    let rest = &content[start + 7..];
    let end = rest.find("```")?;
    let json_str = rest[..end].trim();
    serde_json::from_str(json_str).ok()
}
