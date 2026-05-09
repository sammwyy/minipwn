//! Configuration management: global config dir, worker config, secrets, workspace.

mod paths;
mod secrets;
mod worker_cfg;
mod workspace;

pub use paths::*;
pub use secrets::*;
pub use worker_cfg::*;
pub use workspace::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub provider: String,
    pub theme: String,
    pub max_iterations: usize,
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    pub max_history_display: usize,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            theme: "dracula".to_string(),
            max_iterations: 15,
            tui: TuiConfig {
                max_history_display: 10,
            },
        }
    }
}

/// Initialize the global config directory structure for minipwn.
pub fn init_config_dirs() -> Result<()> {
    let base = config_dir()?;
    std::fs::create_dir_all(&base)?;

    let config_path = global_config_path()?;
    if !config_path.exists() {
        let config = GlobalConfig::default();
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, content)?;
    }

    let workers_path = workers_toml_path()?;
    if !workers_path.exists() {
        std::fs::write(&workers_path, DEFAULT_WORKERS_TOML)?;
    }

    let secrets_path = secrets_env_path()?;
    if !secrets_path.exists() {
        std::fs::write(&secrets_path, DEFAULT_SECRETS_ENV)?;
    }

    Ok(())
}

pub fn load_global_config() -> Result<GlobalConfig> {
    let path = global_config_path()?;
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

pub fn save_global_config(config: &GlobalConfig) -> Result<()> {
    let path = global_config_path()?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

const DEFAULT_WORKERS_TOML: &str = r#"# MiniPWN saved workers
workers = []
"#;

const DEFAULT_SECRETS_ENV: &str = r#"OPENAI_ENDPOINT="https://api.openai.com/v1"
OPENAI_SECRETKEY=""
OPENAI_MODEL=""
OPENROUTER_ENDPOINT="https://openrouter.ai/api/v1"
OPENROUTER_SECRETKEY=""
OPENROUTER_MODEL=""
CUSTOM_ENDPOINT=""
CUSTOM_SECRETKEY=""
CUSTOM_MODEL=""
"#;
