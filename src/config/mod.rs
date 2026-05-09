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

/// Initialize the global config directory structure for minipwn.
/// Creates: $CONFIG_DIR/minipwn/{config.toml,workers.toml,secrets.env}
pub fn init_config_dirs() -> Result<()> {
    let base = config_dir()?;
    std::fs::create_dir_all(&base)?;

    let config_path = base.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG_TOML)?;
    }

    let workers_path = base.join("workers.toml");
    if !workers_path.exists() {
        std::fs::write(&workers_path, DEFAULT_WORKERS_TOML)?;
    }

    let secrets_path = base.join("secrets.env");
    if !secrets_path.exists() {
        std::fs::write(&secrets_path, DEFAULT_SECRETS_ENV)?;
    }

    Ok(())
}

const DEFAULT_CONFIG_TOML: &str = r#"# MiniPWN global configuration
[tui]
max_history_display = 10
"#;

const DEFAULT_WORKERS_TOML: &str = r#"# MiniPWN saved workers
# [[workers]]
# name = "my-server"
# url = "http://localhost:10000"
# secret = "changeme"
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
