//! Worker configuration: minipwn.worker.toml parsing and generation.

use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Top-level worker config file (minipwn.worker.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub server: ServerConfig,
    pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsConfig {
    pub fs: Vec<FsPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPermission {
    pub path: Vec<String>,
    pub mode: String,
    #[serde(rename = "match")]
    pub match_glob: Vec<String>,
    pub read: bool,
    pub write: bool,
}

impl WorkerConfig {
    /// Load worker config from a path; generates default if not found.
    pub fn load_or_default(path: &std::path::PathBuf) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let cfg: WorkerConfig = toml::from_str(&content)?;
            Ok(cfg)
        } else {
            let cfg = Self::default();
            let content = toml::to_string_pretty(&cfg)?;
            std::fs::write(path, content)?;
            Ok(cfg)
        }
    }

    /// Apply CLI overrides for secret and port.
    pub fn apply_overrides(&mut self, secret: Option<String>, port: Option<u16>) {
        if let Some(s) = secret {
            self.server.secret = s;
        }
        if let Some(p) = port {
            self.server.port = p;
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                port: 10000,
                secret: generate_secret(),
            },
            permissions: PermissionsConfig {
                fs: vec![FsPermission {
                    path: vec!["{cwd}".to_string()],
                    mode: "allow".to_string(),
                    match_glob: vec!["*".to_string()],
                    read: true,
                    write: true,
                }],
            },
        }
    }
}

/// Generate a random 32-char alphanumeric secret.
pub fn generate_secret() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect()
}
