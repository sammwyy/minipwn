//! Pure formatting helpers shared by the agent loop and the chat history view.

use std::time::Duration;

use crate::config::load_system_prompt;
use crate::tools::ToolCall;
use crate::worker::Worker;

/// Build the system prompt for a mode, injecting the worker's environment info.
pub async fn system_prompt(mode: &str, worker: &dyn Worker) -> String {
    let base = load_system_prompt(mode);
    let info = worker.system_info().await;
    base.replace("{{WORKER_INFO}}", &info)
}

/// Turn a snake_case tool name into a Title Cased label (`shell_exec` → `Shell Exec`).
pub fn pretty_tool_name(tool: &str) -> String {
    tool.split('_')
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The human-facing command text for a tool call (the raw command for shells).
pub fn command_text(call: &ToolCall) -> String {
    if call.tool == "shell_exec" {
        call.args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        call.args.to_string()
    }
}

/// Collapse tool output to a short single-line summary for the status bubble.
pub fn summarize_output(output: &str) -> String {
    let mut brief = output.replace('\n', " ");
    if brief.chars().count() > 32 {
        brief = format!("{}...", brief.chars().take(32).collect::<String>());
    }
    brief
}

/// Format an elapsed duration as `MM:SS.mmm`.
pub fn format_elapsed(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    let ms = elapsed.subsec_millis();
    format!("{:02}:{:02}.{:03}", secs / 60, secs % 60, ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pretty_tool_name_title_cases_segments() {
        assert_eq!(pretty_tool_name("shell_exec"), "Shell Exec");
        assert_eq!(pretty_tool_name("fs_ls"), "Fs Ls");
        assert_eq!(pretty_tool_name("ping"), "Ping");
    }

    #[test]
    fn command_text_prefers_shell_command() {
        let shell = ToolCall {
            tool: "shell_exec".to_string(),
            args: json!({ "command": "ls -la" }),
        };
        assert_eq!(command_text(&shell), "ls -la");

        let fs = ToolCall {
            tool: "fs_read".to_string(),
            args: json!({ "path": "a.txt" }),
        };
        assert_eq!(command_text(&fs), r#"{"path":"a.txt"}"#);
    }

    #[test]
    fn summarize_output_truncates_and_flattens() {
        assert_eq!(summarize_output("a\nb\nc"), "a b c");
        let long = "x".repeat(40);
        let brief = summarize_output(&long);
        assert!(brief.ends_with("..."));
        assert_eq!(brief.chars().count(), 35); // 32 chars + "..."
    }

    #[test]
    fn format_elapsed_pads_fields() {
        assert_eq!(format_elapsed(Duration::from_millis(5)), "00:00.005");
        assert_eq!(format_elapsed(Duration::from_secs(75)), "01:15.000");
    }
}
