//! Docker-backed worker: deploys a Kali container and drives it as a [`Worker`].
//!
//! [`client`] holds the low-level Docker daemon client; this module owns the
//! deployment orchestration and the runtime [`DockerWorker`] that the agent
//! talks to (which behaves like a remote worker over the container's port).

mod client;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::generate_secret;
use crate::tools::{ToolCall, ToolResult};

use super::client::WorkerClient;
use super::remote::RemoteWorker;
use super::{Worker, WorkerKind};

const DOCKER_SOCKET: &str = "/var/run/docker.sock";
const KALI_IMAGE: &str = "kalilinux/kali-rolling";
const CONTAINER_PORT: u16 = 10000;
const WORKER_BIN_PATH: &str = "/usr/local/bin/minipwn";
const WORKSPACE_PATH: &str = "/workspace";

/// Metadata describing a freshly deployed worker container.
#[derive(Debug, Clone)]
pub struct DeployedContainer {
    pub name: String,
    pub url: String,
    pub secret: String,
    pub container_id: String,
}

/// A runtime worker backed by an **ephemeral** Docker container.
///
/// Tool execution is identical to a [`RemoteWorker`] (filesystem local, shell
/// over HTTP). The deployment is owned entirely by this process: the container
/// is never persisted to the saved-workers list, and [`Drop`] force-removes it
/// when the worker goes away, so nothing is left running after the session.
pub struct DockerWorker {
    inner: RemoteWorker,
    container_id: String,
}

impl Drop for DockerWorker {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let api = client::DockerApi::new(DOCKER_SOCKET);
            match api.remove_container(&self.container_id) {
                Ok(()) => {
                    tracing::debug!("Removed ephemeral worker container {}", self.container_id)
                }
                Err(err) => tracing::warn!(
                    "Failed to remove ephemeral worker container {}: {}",
                    self.container_id,
                    err
                ),
            }
        }
    }
}

impl DockerWorker {
    /// Build a runtime worker from a deployed container descriptor.
    pub fn new(deployed: &DeployedContainer, workspace: PathBuf) -> Self {
        Self {
            inner: RemoteWorker::new(
                deployed.name.clone(),
                &deployed.url,
                &deployed.secret,
                workspace,
            ),
            container_id: deployed.container_id.clone(),
        }
    }

    /// Borrow the underlying HTTP client (e.g. to validate before connecting).
    pub fn client(&self) -> &WorkerClient {
        self.inner.client()
    }
}

#[async_trait]
impl Worker for DockerWorker {
    fn kind(&self) -> WorkerKind {
        WorkerKind::Docker
    }

    fn display_name(&self) -> String {
        format!("Kali Docker ({})", self.inner.base_url())
    }

    fn status_label(&self) -> String {
        format!(
            "◈ {} ({})",
            self.kind().label(),
            super::traits::host_of(self.inner.base_url())
        )
    }

    async fn execute(&self, call: &ToolCall) -> ToolResult {
        self.inner.execute(call).await
    }

    async fn system_info(&self) -> String {
        self.inner.system_info().await
    }
}

/// Deploy a Kali Linux container running the current MiniPWN binary as a worker.
pub async fn deploy_kali_worker(
    workspace: &Path,
    log_tx: Option<UnboundedSender<String>>,
) -> Result<DeployedContainer> {
    #[cfg(not(unix))]
    {
        let _ = (workspace, log_tx);
        anyhow::bail!("Kali Docker workers require a Unix Docker socket");
    }

    #[cfg(unix)]
    {
        use client::DockerApi;
        use std::time::Duration;

        let bin = std::env::current_exe().context("Could not resolve current minipwn binary")?;
        let bin = bin
            .canonicalize()
            .with_context(|| format!("Could not canonicalize {}", bin.display()))?;
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("Could not canonicalize {}", workspace.display()))?;

        let api = DockerApi::new(DOCKER_SOCKET);
        log(
            &log_tx,
            format!("Resolving Docker socket at {}", DOCKER_SOCKET),
        );
        log(&log_tx, format!("Pulling image {}", KALI_IMAGE));
        api.pull_image(KALI_IMAGE)?;

        let secret = generate_secret();
        let name = format!("minipwn-kali-{}", short_id());
        log(&log_tx, format!("Creating container {}", name));
        let container_id = api.create_worker_container(&name, &bin, &workspace, &secret)?;
        log(&log_tx, format!("Starting container {}", container_id));
        api.start_container(&container_id)?;

        log(&log_tx, "Inspecting published port".to_string());
        let host_port = api.inspect_host_port(&container_id)?;
        let url = format!("http://127.0.0.1:{}", host_port);

        log(&log_tx, format!("Waiting for worker at {}", url));
        wait_for_worker(&url, &secret, Duration::from_millis(500)).await?;
        log(&log_tx, "Worker validated successfully".to_string());

        Ok(DeployedContainer {
            name: "kali-docker".to_string(),
            url,
            secret,
            container_id,
        })
    }
}

fn log(tx: &Option<UnboundedSender<String>>, message: String) {
    if let Some(tx) = tx {
        let _ = tx.send(message);
    }
}

#[cfg(unix)]
async fn wait_for_worker(url: &str, secret: &str, interval: std::time::Duration) -> Result<()> {
    let client = WorkerClient::new(url, secret);
    let mut last_err = None;

    for _ in 0..30 {
        match client.validate().await {
            Ok(validation) if validation.ok && validation.secret_valid => return Ok(()),
            Ok(_) => last_err = Some(anyhow::anyhow!("worker rejected validation")),
            Err(err) => last_err = Some(err),
        }
        tokio::time::sleep(interval).await;
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("worker did not become ready")))
}

fn short_id() -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            match idx {
                0..=9 => (b'0' + idx) as char,
                _ => (b'a' + idx - 10) as char,
            }
        })
        .collect()
}
