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
pub fn extract_tool_call(content: &str) -> Option<ToolCall> {
    if let Some(start) = content.find("```json") {
        let rest = &content[start + 7..];
        if let Some(end) = rest.find("```") {
            let json_str = rest[..end].trim();
            if let Ok(tc) = serde_json::from_str(json_str) {
                return Some(tc);
            }
        }
    }
    
    // Fallback: try parsing the entire content as JSON
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return serde_json::from_str(trimmed).ok();
    }
    
    None
}

/// Strip the tool call JSON from the content for UI display.
pub fn strip_tool_call(content: &str) -> String {
    if let Some(start) = content.find("```json") {
        let rest = &content[start + 7..];
        if let Some(end) = rest.find("```") {
            let before = &content[..start];
            let after = &rest[end + 3..];
            return format!("{}{}", before, after).trim().to_string();
        }
    }
    
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return String::new();
        }
    }
    
    content.to_string()
}
