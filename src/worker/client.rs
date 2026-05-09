//! HTTP client for communicating with a remote worker server.

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

use crate::tools::{ToolCall, ToolResult};

/// Client for a remote MiniPWN worker instance.
#[derive(Clone)]
pub struct WorkerClient {
    pub base_url: String,
    secret: String,
    client: Client,
}

/// System information returned from the worker /info endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct WorkerInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub hostname: String,
    pub cwd: String,
}

impl WorkerClient {
    /// Create a new worker client.
    pub fn new(base_url: &str, secret: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            secret: secret.to_string(),
            client: Client::new(),
        }
    }

    /// Build an authorized request builder.
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.secret)
    }

    /// Ping the worker and fetch its system info.
    pub async fn get_info(&self) -> Result<WorkerInfo> {
        let resp = self
            .client
            .get(format!("{}/info", self.base_url))
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .error_for_status()?
            .json::<WorkerInfo>()
            .await?;
        Ok(resp)
    }

    /// Execute a tool call on the remote worker.
    pub async fn execute_tool(&self, call: &ToolCall) -> Result<ToolResult> {
        match call.tool.as_str() {
            "shell_exec" => {
                let command = call
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.exec_command(command).await
            }
            "shell_open" => self.shell_open(call).await,
            "shell_write" => self.shell_write(call).await,
            "shell_read" => self.shell_read(call).await,
            "shell_close" => self.shell_close(call).await,
            _ => Ok(ToolResult::err(&call.tool, "Unsupported remote tool")),
        }
    }

    async fn exec_command(&self, command: &str) -> Result<ToolResult> {
        #[derive(serde::Deserialize)]
        struct ExecResp {
            stdout: String,
            stderr: String,
            exit_code: i32,
        }

        let resp: ExecResp = self
            .client
            .post(format!("{}/exec", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "command": command }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let output = if resp.stderr.is_empty() {
            format!("exit={}\n{}", resp.exit_code, resp.stdout)
        } else {
            format!(
                "exit={}\nstdout:\n{}\nstderr:\n{}",
                resp.exit_code, resp.stdout, resp.stderr
            )
        };

        Ok(ToolResult::ok("shell_exec", output.trim().to_string()))
    }

    async fn shell_open(&self, call: &ToolCall) -> Result<ToolResult> {
        #[derive(serde::Deserialize)]
        struct Resp {
            id: String,
            error: Option<String>,
        }

        let id = call.args.get("id").and_then(|v| v.as_str());
        let resp: Resp = self
            .client
            .post(format!("{}/shell/open", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(err) = resp.error {
            Ok(ToolResult::err("shell_open", err))
        } else {
            Ok(ToolResult::ok(
                "shell_open",
                format!("Session opened: {}", resp.id),
            ))
        }
    }

    async fn shell_write(&self, call: &ToolCall) -> Result<ToolResult> {
        #[derive(serde::Deserialize)]
        struct Resp {
            ok: bool,
            message: Option<String>,
        }

        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let input = call
            .args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let resp: Resp = self
            .client
            .post(format!("{}/shell/write", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id, "input": input }))
            .send()
            .await?
            .json()
            .await?;

        if resp.ok {
            Ok(ToolResult::ok("shell_write", "Written"))
        } else {
            Ok(ToolResult::err(
                "shell_write",
                resp.message.unwrap_or_default(),
            ))
        }
    }

    async fn shell_read(&self, call: &ToolCall) -> Result<ToolResult> {
        #[derive(serde::Deserialize)]
        struct Resp {
            output: String,
            error: Option<String>,
        }

        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let resp: Resp = self
            .client
            .post(format!("{}/shell/read", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .send()
            .await?
            .json()
            .await?;

        if let Some(err) = resp.error {
            Ok(ToolResult::err("shell_read", err))
        } else {
            Ok(ToolResult::ok("shell_read", resp.output))
        }
    }

    async fn shell_close(&self, call: &ToolCall) -> Result<ToolResult> {
        #[derive(serde::Deserialize)]
        struct Resp {
            ok: bool,
            message: Option<String>,
        }

        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let resp: Resp = self
            .client
            .post(format!("{}/shell/close", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .send()
            .await?
            .json()
            .await?;

        if resp.ok {
            Ok(ToolResult::ok("shell_close", "Session closed"))
        } else {
            Ok(ToolResult::err(
                "shell_close",
                resp.message.unwrap_or_default(),
            ))
        }
    }
}
