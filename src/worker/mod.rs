//! Worker mode: HTTP server exposing shell and system info endpoints.

pub mod client;
mod handlers;
mod routes;
mod state;

use anyhow::Result;
use std::net::SocketAddr;

use crate::config::{WorkerConfig, worker_config_path};

/// Entry point for `minipwn worker` / `minipwn w`.
pub async fn run(
    secret_override: Option<String>,
    port_override: Option<u16>,
    config_path_override: Option<String>,
) -> Result<()> {
    let cfg_path = worker_config_path(config_path_override.as_deref())?;
    let mut cfg = WorkerConfig::load_or_default(&cfg_path)?;
    cfg.apply_overrides(secret_override, port_override);

    let addr: SocketAddr = format!("0.0.0.0:{}", cfg.server.port).parse()?;

    println!("MiniPWN Worker starting on {}", addr);
    println!("Secret: {}", cfg.server.secret);
    println!("Config: {}", cfg_path.display());

    let app = routes::build_router(cfg);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
