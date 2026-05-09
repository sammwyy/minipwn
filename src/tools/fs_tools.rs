//! File system tool implementations (local execution in workspace).

use anyhow::Result;
use std::path::Path;

use super::sanitize::sanitize_path;
use super::{ToolCall, ToolResult};

/// Execute a file system tool call within the given workspace root.
pub fn execute_fs_tool(call: &ToolCall, workspace: &Path) -> ToolResult {
    match call.tool.as_str() {
        "fs_ls" => fs_ls(call, workspace),
        "fs_read" => fs_read(call, workspace),
        "fs_write" => fs_write(call, workspace),
        "fs_mkdir" => fs_mkdir(call, workspace),
        "fs_rm" => fs_rm(call, workspace),
        "fs_copy" => fs_copy(call, workspace),
        "fs_mv" => fs_mv(call, workspace),
        _ => ToolResult::err(&call.tool, "Unknown fs tool"),
    }
}

fn resolve(workspace: &Path, raw: &str) -> Result<std::path::PathBuf> {
    let rel = sanitize_path(raw)?;
    Ok(workspace.join(rel))
}

fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing arg: {}", key))
}

fn fs_ls(call: &ToolCall, workspace: &Path) -> ToolResult {
    let raw = call
        .args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let path = match resolve(workspace, raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };

    match std::fs::read_dir(&path) {
        Ok(entries) => {
            let mut lines: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Hide .minipwn from listing
                    if name == ".minipwn" {
                        return None;
                    }
                    let meta = e.metadata().ok()?;
                    let kind = if meta.is_dir() { "d" } else { "f" };
                    let size = if meta.is_file() {
                        format!("{:>10}", meta.len())
                    } else {
                        "         -".to_string()
                    };
                    Some(format!("[{}] {} {}", kind, size, name))
                })
                .collect();
            lines.sort();
            if lines.is_empty() {
                ToolResult::ok(&call.tool, "(empty directory)")
            } else {
                ToolResult::ok(&call.tool, lines.join("\n"))
            }
        }
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}

fn fs_read(call: &ToolCall, workspace: &Path) -> ToolResult {
    let raw = match get_str(&call.args, "path") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let path = match resolve(workspace, raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => ToolResult::ok(&call.tool, content),
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}

fn fs_write(call: &ToolCall, workspace: &Path) -> ToolResult {
    let raw = match get_str(&call.args, "path") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let content = call
        .args
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = match resolve(workspace, raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    // Create parent dirs if needed
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return ToolResult::err(&call.tool, e.to_string());
        }
    }
    match std::fs::write(&path, content) {
        Ok(_) => ToolResult::ok(
            &call.tool,
            format!("Written {} bytes to {}", content.len(), raw),
        ),
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}

fn fs_mkdir(call: &ToolCall, workspace: &Path) -> ToolResult {
    let raw = match get_str(&call.args, "path") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let path = match resolve(workspace, raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    match std::fs::create_dir_all(&path) {
        Ok(_) => ToolResult::ok(&call.tool, format!("Directory created: {}", raw)),
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}

fn fs_rm(call: &ToolCall, workspace: &Path) -> ToolResult {
    let raw = match get_str(&call.args, "path") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let path = match resolve(workspace, raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    if path.is_dir() {
        match std::fs::remove_dir_all(&path) {
            Ok(_) => ToolResult::ok(&call.tool, format!("Removed directory: {}", raw)),
            Err(e) => ToolResult::err(&call.tool, e.to_string()),
        }
    } else {
        match std::fs::remove_file(&path) {
            Ok(_) => ToolResult::ok(&call.tool, format!("Removed file: {}", raw)),
            Err(e) => ToolResult::err(&call.tool, e.to_string()),
        }
    }
}

fn fs_copy(call: &ToolCall, workspace: &Path) -> ToolResult {
    let from_raw = match get_str(&call.args, "from") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let to_raw = match get_str(&call.args, "to") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let from = match resolve(workspace, from_raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let to = match resolve(workspace, to_raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    match std::fs::copy(&from, &to) {
        Ok(bytes) => ToolResult::ok(
            &call.tool,
            format!("Copied {} bytes: {} -> {}", bytes, from_raw, to_raw),
        ),
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}

fn fs_mv(call: &ToolCall, workspace: &Path) -> ToolResult {
    let from_raw = match get_str(&call.args, "from") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let to_raw = match get_str(&call.args, "to") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let from = match resolve(workspace, from_raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    let to = match resolve(workspace, to_raw) {
        Ok(p) => p,
        Err(e) => return ToolResult::err(&call.tool, e.to_string()),
    };
    match std::fs::rename(&from, &to) {
        Ok(_) => ToolResult::ok(&call.tool, format!("Moved: {} -> {}", from_raw, to_raw)),
        Err(e) => ToolResult::err(&call.tool, e.to_string()),
    }
}
