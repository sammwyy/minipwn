//! HTTP worker protocol: the request/response bodies for the worker server.

use serde::{Deserialize, Serialize};

/// System information reported by a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub hostname: String,
    pub cwd: String,
}

/// Response to `GET /ping`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub pong: bool,
    pub worker: String,
    pub port: u16,
}

/// Response to `GET /validate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResponse {
    pub ok: bool,
    pub secret_valid: bool,
    pub secret_len: usize,
    pub info: WorkerInfo,
}

/// Body of `POST /exec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    pub command: String,
}

/// Response to `POST /exec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Body of `POST /shell/open`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellOpenRequest {
    pub id: Option<String>,
}

/// Response to `POST /shell/open`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellOpenResponse {
    pub id: String,
    pub error: Option<String>,
}

/// Body of `POST /shell/write`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellWriteRequest {
    pub id: String,
    pub input: String,
}

/// Body of `POST /shell/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellReadRequest {
    pub id: String,
}

/// Response to `POST /shell/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellReadResponse {
    pub output: String,
    pub error: Option<String>,
}

/// Body of `POST /shell/close`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCloseRequest {
    pub id: String,
}

/// Generic ok/error response used by shell write/close.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleResponse {
    pub ok: bool,
    pub message: Option<String>,
}
