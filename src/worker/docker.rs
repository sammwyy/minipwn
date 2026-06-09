//! Docker-backed worker deployment helpers.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::generate_secret;

const DOCKER_SOCKET: &str = "/var/run/docker.sock";
const KALI_IMAGE: &str = "kalilinux/kali-rolling";
const CONTAINER_PORT: u16 = 10000;
const WORKER_BIN_PATH: &str = "/usr/local/bin/minipwn";
const WORKSPACE_PATH: &str = "/workspace";

#[derive(Debug, Clone)]
pub struct DockerWorker {
    pub name: String,
    pub url: String,
    pub secret: String,
}

/// Deploy a Kali Linux container running the current MiniPWN binary as a worker.
pub async fn deploy_kali_worker(
    workspace: &Path,
    log_tx: Option<UnboundedSender<String>>,
) -> Result<DockerWorker> {
    #[cfg(not(unix))]
    {
        let _ = workspace;
        bail!("Kali Docker workers require a Unix Docker socket");
    }

    #[cfg(unix)]
    {
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
        wait_for_worker(&url, &secret).await?;
        log(&log_tx, "Worker validated successfully".to_string());

        Ok(DockerWorker {
            name: "kali-docker".to_string(),
            url,
            secret,
        })
    }
}

fn log(tx: &Option<UnboundedSender<String>>, message: String) {
    if let Some(tx) = tx {
        let _ = tx.send(message);
    }
}

async fn wait_for_worker(url: &str, secret: &str) -> Result<()> {
    let client = crate::worker::client::WorkerClient::new(url, secret);
    let mut last_err = None;

    for _ in 0..30 {
        match client.validate().await {
            Ok(validation) if validation.ok && validation.secret_valid => return Ok(()),
            Ok(_) => last_err = Some(anyhow::anyhow!("worker rejected validation")),
            Err(err) => last_err = Some(err),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
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

#[cfg(unix)]
struct DockerApi {
    socket_path: &'static str,
}

#[cfg(unix)]
impl DockerApi {
    fn new(socket_path: &'static str) -> Self {
        Self { socket_path }
    }

    fn pull_image(&self, image: &str) -> Result<()> {
        let path = format!("/images/create?fromImage={}", percent_encode(image));
        let resp = self.request("POST", &path, None)?;
        resp.ensure_success()
            .with_context(|| format!("Docker image pull failed for {}", image))
    }

    fn create_worker_container(
        &self,
        name: &str,
        bin: &Path,
        workspace: &Path,
        secret: &str,
    ) -> Result<String> {
        let binds = vec![
            format!("{}:{}:ro", bind_path(bin), WORKER_BIN_PATH),
            format!("{}:{}:ro", bind_path(workspace), WORKSPACE_PATH),
        ];

        let body = json!({
            "Image": KALI_IMAGE,
            "Cmd": [
                WORKER_BIN_PATH,
                "worker",
                "--secret",
                secret,
                "--port",
                CONTAINER_PORT.to_string()
            ],
            "WorkingDir": WORKSPACE_PATH,
            "ExposedPorts": {
                format!("{}/tcp", CONTAINER_PORT): {}
            },
            "HostConfig": {
                "Binds": binds,
                "PortBindings": {
                    format!("{}/tcp", CONTAINER_PORT): [
                        { "HostIp": "127.0.0.1", "HostPort": "" }
                    ]
                }
            }
        });

        let path = format!("/containers/create?name={}", percent_encode(name));
        let resp = self.request("POST", &path, Some(body.to_string()))?;
        resp.ensure_success()
            .with_context(|| format!("Docker container create failed for {}", name))?;

        let created: CreateContainerResponse = serde_json::from_slice(&resp.body)?;
        Ok(created.id)
    }

    fn start_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/start", percent_encode(id));
        let resp = self.request("POST", &path, None)?;
        resp.ensure_success()
            .with_context(|| format!("Docker container start failed for {}", id))
    }

    fn inspect_host_port(&self, id: &str) -> Result<u16> {
        let path = format!("/containers/{}/json", percent_encode(id));
        let resp = self.request("GET", &path, None)?;
        resp.ensure_success()
            .with_context(|| format!("Docker container inspect failed for {}", id))?;

        let inspect: InspectContainerResponse = serde_json::from_slice(&resp.body)?;
        let key = format!("{}/tcp", CONTAINER_PORT);
        let binding = inspect
            .network_settings
            .ports
            .get(&key)
            .and_then(|bindings| bindings.as_ref())
            .and_then(|bindings| bindings.first())
            .context("Docker did not publish the worker port")?;

        binding
            .host_port
            .parse::<u16>()
            .context("Docker returned an invalid worker host port")
    }

    fn request(&self, method: &str, path: &str, body: Option<String>) -> Result<DockerResponse> {
        use std::os::unix::net::UnixStream;

        let mut stream = UnixStream::connect(self.socket_path)
            .with_context(|| format!("Could not connect to Docker socket {}", self.socket_path))?;
        stream.set_read_timeout(Some(Duration::from_secs(600)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        let body = body.unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: docker\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(request.as_bytes())?;

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        parse_response(raw)
    }
}

#[cfg(unix)]
struct DockerResponse {
    status: u16,
    body: Vec<u8>,
}

#[cfg(unix)]
impl DockerResponse {
    fn ensure_success(&self) -> Result<()> {
        if (200..300).contains(&self.status) {
            Ok(())
        } else {
            let body = String::from_utf8_lossy(&self.body);
            bail!("Docker API returned HTTP {}: {}", self.status, body.trim())
        }
    }
}

#[cfg(unix)]
fn parse_response(raw: Vec<u8>) -> Result<DockerResponse> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .context("Docker returned an invalid HTTP response")?;
    let headers = String::from_utf8_lossy(&raw[..header_end]);
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .context("Docker returned an HTTP response without a status")?;

    let mut body = raw[header_end + 4..].to_vec();
    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body = decode_chunked(&body)?;
    }

    Ok(DockerResponse { status, body })
}

#[cfg(unix)]
fn decode_chunked(body: &[u8]) -> Result<Vec<u8>> {
    let mut decoded = Vec::new();
    let mut idx = 0;

    loop {
        let line_end = body[idx..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|pos| idx + pos)
            .context("Invalid chunked Docker response")?;
        let size_text = std::str::from_utf8(&body[idx..line_end])?;
        let size_text = size_text.split(';').next().unwrap_or(size_text).trim();
        let size = usize::from_str_radix(size_text, 16)?;
        idx = line_end + 2;

        if size == 0 {
            break;
        }
        if idx + size > body.len() {
            bail!("Invalid chunked Docker response size");
        }

        decoded.extend_from_slice(&body[idx..idx + size]);
        idx += size + 2;
        if idx > body.len() {
            bail!("Invalid chunked Docker response terminator");
        }
    }

    Ok(decoded)
}

#[cfg(unix)]
fn bind_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

#[derive(Debug, Deserialize)]
struct CreateContainerResponse {
    #[serde(rename = "Id")]
    id: String,
}

#[derive(Debug, Deserialize)]
struct InspectContainerResponse {
    #[serde(rename = "NetworkSettings")]
    network_settings: NetworkSettings,
}

#[derive(Debug, Deserialize)]
struct NetworkSettings {
    #[serde(rename = "Ports")]
    ports: std::collections::HashMap<String, Option<Vec<PortBinding>>>,
}

#[derive(Debug, Deserialize)]
struct PortBinding {
    #[serde(rename = "HostPort")]
    host_port: String,
}
