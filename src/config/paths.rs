//! Platform-aware path resolution for config and workspace directories.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Returns the minipwn config directory:
///   Windows: %APPDATA%\minipwn
///   macOS:   ~/Library/Application Support/minipwn
///   Linux:   ~/.config/minipwn
pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .context("Could not determine system config directory")?;
    Ok(base.join("minipwn"))
}

/// Returns the workers.toml path inside the config directory.
pub fn workers_toml_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("workers.toml"))
}

/// Returns the secrets.env path inside the config directory.
pub fn secrets_env_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("secrets.env"))
}

/// Returns the worker config path for the worker mode.
/// Looks for minipwn.worker.toml in the current directory first,
/// then falls back to the global config directory.
pub fn worker_config_path(override_path: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(PathBuf::from(p));
    }
    let local = std::env::current_dir()?.join("minipwn.worker.toml");
    if local.exists() {
        return Ok(local);
    }
    Ok(config_dir()?.join("minipwn.worker.toml"))
}