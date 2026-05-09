//! HTTP request handlers for the worker server.

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::state::{AppState, ShellSession};

/// GET /info — Return worker system information.
pub async fn handle_info(State(state): State<AppState>) -> Json<WorkerInfo> {
    Json(WorkerInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        hostname: hostname(),
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    })
}

/// POST /exec — Execute a one-shot shell command.
pub async fn handle_exec(
    State(_state): State<AppState>,
    Json(req): Json<ExecRequest>,
) -> Json<ExecResponse> {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &req.command]).output()
    } else {
        Command::new("sh").args(["-c", &req.command]).output()
    };

    match output {
        Ok(out) => Json(ExecResponse {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            exit_code: out.status.code().unwrap_or(-1),
        }),
        Err(e) => Json(ExecResponse {
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: -1,
        }),
    }
}

/// POST /shell/open — Open an interactive shell session.
pub async fn handle_shell_open(
    State(state): State<AppState>,
    Json(req): Json<ShellOpenRequest>,
) -> Json<ShellOpenResponse> {
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let shell_cmd: (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("cmd", vec![])
    } else {
        ("bash", vec![])
    };

    let result = Command::new(shell_cmd.0)
        .args(&shell_cmd.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match result {
        Ok(mut child) => {
            let stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let session = ShellSession {
                stdin,
                stdout,
                child,
            };
            let mut shells = state.shells.lock().unwrap();
            shells.insert(id.clone(), Arc::new(Mutex::new(session)));
            Json(ShellOpenResponse { id, error: None })
        }
        Err(e) => Json(ShellOpenResponse {
            id: String::new(),
            error: Some(e.to_string()),
        }),
    }
}

/// POST /shell/write — Write to an interactive shell session.
pub async fn handle_shell_write(
    State(state): State<AppState>,
    Json(req): Json<ShellWriteRequest>,
) -> (StatusCode, Json<SimpleResponse>) {
    let shells = state.shells.lock().unwrap();
    match shells.get(&req.id) {
        Some(session) => {
            let mut sess = session.lock().unwrap();
            match sess.stdin.write_all(req.input.as_bytes()) {
                Ok(_) => (
                    StatusCode::OK,
                    Json(SimpleResponse {
                        ok: true,
                        message: None,
                    }),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SimpleResponse {
                        ok: false,
                        message: Some(e.to_string()),
                    }),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                ok: false,
                message: Some(format!("Session not found: {}", req.id)),
            }),
        ),
    }
}

/// POST /shell/read — Read available output from a shell session.
pub async fn handle_shell_read(
    State(state): State<AppState>,
    Json(req): Json<ShellReadRequest>,
) -> Json<ShellReadResponse> {
    let shells = state.shells.lock().unwrap();
    match shells.get(&req.id) {
        Some(session) => {
            let mut sess = session.lock().unwrap();
            let mut buf = [0u8; 4096];
            match sess.stdout.read(&mut buf) {
                Ok(n) => Json(ShellReadResponse {
                    output: String::from_utf8_lossy(&buf[..n]).to_string(),
                    error: None,
                }),
                Err(e) => Json(ShellReadResponse {
                    output: String::new(),
                    error: Some(e.to_string()),
                }),
            }
        }
        None => Json(ShellReadResponse {
            output: String::new(),
            error: Some(format!("Session not found: {}", req.id)),
        }),
    }
}

/// POST /shell/close — Terminate a shell session.
pub async fn handle_shell_close(
    State(state): State<AppState>,
    Json(req): Json<ShellCloseRequest>,
) -> (StatusCode, Json<SimpleResponse>) {
    let mut shells = state.shells.lock().unwrap();
    match shells.remove(&req.id) {
        Some(session) => {
            let mut sess = session.lock().unwrap();
            let _ = sess.child.kill();
            (
                StatusCode::OK,
                Json(SimpleResponse {
                    ok: true,
                    message: None,
                }),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                ok: false,
                message: Some(format!("Session not found: {}", req.id)),
            }),
        ),
    }
}

// ---- Request / Response types ----

#[derive(Debug, Serialize)]
pub struct WorkerInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub hostname: String,
    pub cwd: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecRequest {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Deserialize)]
pub struct ShellOpenRequest {
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShellOpenResponse {
    pub id: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShellWriteRequest {
    pub id: String,
    pub input: String,
}

#[derive(Debug, Deserialize)]
pub struct ShellReadRequest {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct ShellReadResponse {
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShellCloseRequest {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub ok: bool,
    pub message: Option<String>,
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
