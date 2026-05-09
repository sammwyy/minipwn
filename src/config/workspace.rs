//! Workspace management: .minipwn/ directory, workspace.toml, chat history.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Name of the hidden workspace directory created in cwd.
pub const WORKSPACE_DIR: &str = ".minipwn";
const CHATS_DIR: &str = "chats";
const WORKSPACE_TOML: &str = "workspace.toml";
const STATS_TOML: &str = "stats.toml";
pub const SYSTEM_PROMPT_FILE: &str = "system_prompt.md";

/// Default system prompt embedded at compile time.
pub const DEFAULT_SYSTEM_PROMPT: &str = include_str!("../../data/system_prompt.md");

/// workspace.toml schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub current_chat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceStats {
    pub total_tokens: u64,
}

impl Default for WorkspaceMeta {
    fn default() -> Self {
        Self {
            current_chat: "1".to_string(),
        }
    }
}

/// A single chat message stored in the chat file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// A chat session loaded from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub messages: Vec<ChatMessage>,
}

/// Returns the path to the workspace directory in cwd.
pub fn workspace_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(WORKSPACE_DIR))
}

/// Initialize the workspace directory and default files.
pub fn init_workspace() -> Result<()> {
    let dir = workspace_dir()?;
    std::fs::create_dir_all(dir.join(CHATS_DIR))?;

    let meta_path = dir.join(WORKSPACE_TOML);
    if !meta_path.exists() {
        let meta = WorkspaceMeta::default();
        let content = toml::to_string_pretty(&meta)?;
        std::fs::write(meta_path, content)?;
    }

    Ok(())
}

/// Load workspace metadata.
pub fn load_workspace_meta() -> Result<WorkspaceMeta> {
    let path = workspace_dir()?.join(WORKSPACE_TOML);
    if !path.exists() {
        return Ok(WorkspaceMeta::default());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

/// Save workspace metadata.
pub fn save_workspace_meta(meta: &WorkspaceMeta) -> Result<()> {
    let path = workspace_dir()?.join(WORKSPACE_TOML);
    let content = toml::to_string_pretty(meta)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Load workspace stats.
pub fn load_workspace_stats() -> Result<WorkspaceStats> {
    let path = workspace_dir()?.join(STATS_TOML);
    if !path.exists() {
        return Ok(WorkspaceStats::default());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

/// Save workspace stats.
pub fn save_workspace_stats(stats: &WorkspaceStats) -> Result<()> {
    let path = workspace_dir()?.join(STATS_TOML);
    let content = toml::to_string_pretty(stats)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Record token usage.
pub fn add_tokens(tokens: u64) -> Result<()> {
    let mut stats = load_workspace_stats()?;
    stats.total_tokens += tokens;
    save_workspace_stats(&stats)
}

/// Load a chat session by ID, creating it if it doesn't exist.
pub fn load_chat(id: &str) -> Result<ChatSession> {
    let path = chat_path(id)?;
    if !path.exists() {
        return Ok(ChatSession {
            id: id.to_string(),
            messages: vec![],
        });
    }
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

/// Append a message to a chat session on disk.
pub fn append_message(id: &str, message: ChatMessage) -> Result<()> {
    let mut session = load_chat(id)?;
    session.messages.push(message);
    save_chat(&session)
}

/// Save an entire chat session.
pub fn save_chat(session: &ChatSession) -> Result<()> {
    let path = chat_path(&session.id)?;
    let content = toml::to_string_pretty(session)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Clear all messages in a chat.
pub fn clear_chat(id: &str) -> Result<()> {
    let session = ChatSession {
        id: id.to_string(),
        messages: vec![],
    };
    save_chat(&session)
}

/// List all chat IDs available in the workspace.
pub fn list_chats() -> Result<Vec<String>> {
    let dir = workspace_dir()?.join(CHATS_DIR);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut ids = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".toml") {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

/// Get the path to a chat file by ID.
fn chat_path(id: &str) -> Result<PathBuf> {
    Ok(workspace_dir()?
        .join(CHATS_DIR)
        .join(format!("{}.toml", id)))
}

/// Load the system prompt: workspace system_prompt.md > default embedded.
pub fn load_system_prompt() -> String {
    let path = workspace_dir()
        .map(|d| d.join(SYSTEM_PROMPT_FILE))
        .unwrap_or_else(|_| PathBuf::from(SYSTEM_PROMPT_FILE));

    if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string())
    } else {
        DEFAULT_SYSTEM_PROMPT.to_string()
    }
}

/// Saved worker entry in the global workers.toml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedWorker {
    pub name: String,
    pub url: String,
    pub secret: String,
}

/// Top-level structure of workers.toml.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkersList {
    pub workers: Vec<SavedWorker>,
}

/// Load the saved workers list.
pub fn load_workers_list() -> Result<WorkersList> {
    let path = super::paths::workers_toml_path()?;
    if !path.exists() {
        return Ok(WorkersList::default());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

/// Save the workers list to disk.
pub fn save_workers_list(list: &WorkersList) -> Result<()> {
    let path = super::paths::workers_toml_path()?;
    let content = toml::to_string_pretty(list)?;
    std::fs::write(path, content)?;
    Ok(())
}
