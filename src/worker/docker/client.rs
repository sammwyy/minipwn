//! Minimal Docker Engine API client speaking HTTP over the Unix socket.
//!
//! Only the handful of endpoints needed to deploy a worker container are
//! implemented: image pull, container create/start, and inspect.

#![cfg(unix)]

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use super::{CONTAINER_PORT, KALI_IMAGE, WORKER_BIN_PATH, WORKSPACE_PATH};

/// A thin client over the Docker daemon Unix socket.
pub struct DockerApi {
    socket_path: &'static str,
}

impl DockerApi {
    pub fn new(socket_path: &'static str) -> Self {
        Self { socket_path }
    }

    /// Pull an image, blocking until the pull completes.
    pub fn pull_image(&self, image: &str) -> Result<()> {
        let path = format!("/images/create?fromImage={}", percent_encode(image));
        let resp = self.request("POST", &path, None)?;
        resp.ensure_success()
            .with_context(|| format!("Docker image pull failed for {}", image))
    }

    /// Create a worker container that mounts the binary and workspace read-only.
    pub fn create_worker_container(
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

    /// Start a previously created container.
    pub fn start_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/start", percent_encode(id));
        let resp = self.request("POST", &path, None)?;
        resp.ensure_success()
            .with_context(|| format!("Docker container start failed for {}", id))
    }

    /// Inspect a container and return the published host port for the worker.
    pub fn inspect_host_port(&self, id: &str) -> Result<u16> {
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

struct DockerResponse {
    status: u16,
    body: Vec<u8>,
}

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
