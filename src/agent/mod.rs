//! The agentic loop: drive the LLM, run tool calls, feed results back.
//!
//! This module is deliberately free of any TUI dependency. It talks to the
//! outside world only through the [`AgentUi`] trait, so the same loop can be
//! exercised headlessly in tests or reused by a different frontend.

mod format;
mod ui;

pub use format::{command_text, format_elapsed, pretty_tool_name, summarize_output, system_prompt};
pub use ui::AgentUi;

use std::time::Instant;

use anyhow::Result;

use crate::ai::{AiClient, ChatMsg};
use crate::config::{ChatMessage, append_message};
use crate::tools::{extract_tool_call, strip_tool_call};
use crate::worker::Worker;

/// Everything a single turn needs that is not part of the UI.
pub struct TurnContext<'a> {
    /// Client for the configured LLM provider.
    pub ai: &'a AiClient,
    /// Backend that executes tool calls.
    pub worker: &'a dyn Worker,
    /// Chat id to persist messages under.
    pub chat_id: &'a str,
    /// Maximum number of LLM round-trips before bailing out.
    pub max_iterations: usize,
}

/// How a turn finished.
#[derive(Debug, Clone, Copy, Default)]
pub struct TurnOutcome {
    /// Whether the user cancelled the turn (via [`AgentUi::wait_cancel`]).
    pub cancelled: bool,
}

/// Run one agent turn: repeatedly query the LLM, execute any tool it calls, and
/// feed the result back until it produces a final answer (or limits are hit).
///
/// `messages` is the full prompt context (system prompt + prior turns); the
/// caller owns assembling it from whatever its message history looks like.
pub async fn run_turn(
    ui: &mut dyn AgentUi,
    ctx: TurnContext<'_>,
    mut messages: Vec<ChatMsg>,
) -> Result<TurnOutcome> {
    ui.set_thinking(true);
    ui.redraw()?;

    let mut cancelled = false;
    let mut iterations = 0;

    'turn: loop {
        iterations += 1;
        if iterations > ctx.max_iterations {
            ui.assistant(
                "[Max tool iterations reached. Type 'Continue' to keep going.]".to_string(),
                false,
            );
            break;
        }

        // Race the LLM request against a user cancellation.
        let reply = tokio::select! {
            res = ctx.ai.complete(messages.clone()) => res,
            _ = ui.wait_cancel() => {
                ui.set_status("Generation cancelled by user".to_string());
                ui.assistant("[Generation cancelled]".to_string(), true);
                cancelled = true;
                break 'turn;
            }
        };

        let reply = match reply {
            Ok(reply) => reply,
            Err(e) => {
                ui.set_status(format!("API error: {}", e));
                ui.assistant(format!("API error: {}", e), false);
                break 'turn;
            }
        };

        // Estimate tokens (very rough: chars / 4).
        let sent_tokens = messages.iter().map(|m| m.content.len()).sum::<usize>() / 4;
        let recv_tokens = reply.len() / 4;
        ui.record_tokens((sent_tokens + recv_tokens) as u64);

        messages.push(ChatMsg {
            role: "assistant".to_string(),
            content: reply.clone(),
        });

        let Some(tool_call) = extract_tool_call(&reply) else {
            // No tool call — this is the final answer.
            ui.assistant(reply.clone(), false);
            persist_assistant(ctx.chat_id, reply)?;
            break 'turn;
        };

        // Show any prose that accompanied the tool call, then persist the
        // full reply (JSON included) so history can replay it.
        let stripped = strip_tool_call(&reply);
        if !stripped.is_empty() {
            ui.assistant(stripped, false);
        }
        persist_assistant(ctx.chat_id, reply)?;
        ui.redraw()?;

        let nice = pretty_tool_name(&tool_call.tool);
        let cmd_text = command_text(&tool_call);

        let handle = ui.tool_begin(format!("{}: {}\nRunning...", nice, cmd_text));
        ui.redraw()?;

        // Race tool execution against cancellation too, so ESC works while a
        // (async) tool is in flight, not just while the model is thinking.
        let start = Instant::now();
        let result = tokio::select! {
            res = ctx.worker.execute(&tool_call) => res,
            _ = ui.wait_cancel() => {
                ui.tool_update(handle, format!("{}: {}\nCancelled", nice, cmd_text));
                ui.set_status("Generation cancelled by user".to_string());
                cancelled = true;
                break 'turn;
            }
        };
        let elapsed = start.elapsed();

        let full_result = format!(
            "[Tool: {}] {}\n{}",
            tool_call.tool,
            if result.success { "OK" } else { "FAILED" },
            result.output
        );
        let status_prefix = if result.success { "Success:" } else { "Error:" };
        ui.tool_update(
            handle,
            format!(
                "{}: {}\n{} {} ({})",
                nice,
                cmd_text,
                status_prefix,
                summarize_output(&result.output),
                format_elapsed(elapsed),
            ),
        );

        messages.push(ChatMsg {
            role: "user".to_string(),
            content: format!("Tool result:\n{}", full_result),
        });
        ui.redraw()?;
    }

    ui.set_thinking(false);
    ui.redraw()?;
    Ok(TurnOutcome { cancelled })
}

fn persist_assistant(chat_id: &str, content: String) -> Result<()> {
    append_message(
        chat_id,
        ChatMessage {
            role: "assistant".to_string(),
            content,
            timestamp: chrono::Utc::now(),
        },
    )
}
