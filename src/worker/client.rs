//! HTTP client for communicating with a remote worker server.

use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::protocol::{
    ExecResponse, PingResponse, ShellOpenResponse, ShellReadResponse, SimpleResponse,
    ValidateResponse, WorkerInfo,
};
use crate::tools::{ToolCall, ToolResult};

/// Timeout for lightweight health checks (ping / validate / info).
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);
/// Timeout for command/shell execution. Generous, since a single shell command
/// (a scan, a build, a compound pipeline) can legitimately run for minutes.
/// The user can always interrupt an in-flight command with Esc.
const EXEC_TIMEOUT: Duration = Duration::from_secs(1800);

/// Client for a remote MiniPWN worker instance.
#[derive(Clone)]
pub struct WorkerClient {
    pub base_url: String,
    secret: String,
    client: Client,
}

impl WorkerClient {
    /// Create a new worker client.
    pub fn new(base_url: &str, secret: &str) -> Self {
        // Only the *connection* is bounded by a short timeout; per-request
        // timeouts (below) bound how long we wait for a response, so a slow
        // shell command does not get killed at 3s like a health check would.
        let client = Client::builder()
            .connect_timeout(HEALTH_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            secret: secret.to_string(),
            client,
        }
    }

    /// Build an authorized request builder.
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.secret)
    }

    /// Ping the worker to check whether it is alive and the secret is accepted.
    pub async fn ping(&self) -> Result<PingResponse> {
        let resp = self
            .client
            .get(format!("{}/ping", self.base_url))
            .header("Authorization", self.auth_header())
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await?
            .error_for_status()?
            .json::<PingResponse>()
            .await?;
        Ok(resp)
    }

    /// Validate the worker secret and fetch metadata before connecting.
    pub async fn validate(&self) -> Result<ValidateResponse> {
        let resp = self
            .client
            .get(format!("{}/validate", self.base_url))
            .header("Authorization", self.auth_header())
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await?
            .error_for_status()?
            .json::<ValidateResponse>()
            .await?;
        Ok(resp)
    }

    /// Ping the worker and fetch its system info.
    pub async fn get_info(&self) -> Result<WorkerInfo> {
        let resp = self
            .client
            .get(format!("{}/info", self.base_url))
            .header("Authorization", self.auth_header())
            .timeout(HEALTH_TIMEOUT)
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
        let resp: ExecResponse = self
            .client
            .post(format!("{}/exec", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "command": command }))
            .timeout(EXEC_TIMEOUT)
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
        let id = call.args.get("id").and_then(|v| v.as_str());
        let resp: ShellOpenResponse = self
            .client
            .post(format!("{}/shell/open", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .timeout(EXEC_TIMEOUT)
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
        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let input = call
            .args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let resp: SimpleResponse = self
            .client
            .post(format!("{}/shell/write", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id, "input": input }))
            .timeout(EXEC_TIMEOUT)
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
        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let resp: ShellReadResponse = self
            .client
            .post(format!("{}/shell/read", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .timeout(EXEC_TIMEOUT)
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
        let id = call.args.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let resp: SimpleResponse = self
            .client
            .post(format!("{}/shell/close", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&json!({ "id": id }))
            .timeout(EXEC_TIMEOUT)
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
