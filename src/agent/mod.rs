//! The agentic loop: drive the LLM, run tool calls, feed results back.
//!
//! This module is deliberately free of any TUI dependency. It talks to the
//! outside world only through the [`AgentUi`] trait, so the same loop can be
//! exercised headlessly in tests or reused by a different frontend.

mod format;
mod ui;

pub use format::{command_text, format_elapsed, pretty_tool_name, summarize_output, system_prompt};
pub use ui::AgentUi;

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::ai::{AiClient, ChatMsg, StreamPiece};
use crate::config::{ChatMessage, append_message};
use crate::tools::{extract_tool_call, strip_tool_call};
use crate::worker::Worker;

/// How often, while awaiting the model or a tool, we pause to pump user input
/// (so Esc-to-cancel and message queueing stay responsive).
const INPUT_TICK: Duration = Duration::from_millis(40);

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
    /// Whether the user cancelled the turn (via [`AgentUi::poll_input`]).
    pub cancelled: bool,
}

/// Run one agent turn: repeatedly query the LLM, execute any tool it calls, and
/// feed the result back until it produces a final answer (or limits are hit).
///
/// `messages` is the full prompt context (system prompt + prior turns); the
/// caller owns assembling it from whatever its message history looks like.
pub async fn run_turn(
    ui: &mut (dyn AgentUi + Send),
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

        // Stream the model's reply into a live bubble. The returned String is
        // the answer content only (reasoning excluded); cancellation is polled
        // between chunks so Esc works mid-stream.
        let stream_handle = ui.stream_begin();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamPiece>();
        let mut completion = std::pin::pin!(ctx.ai.complete_stream(messages.clone(), tx));

        let reply = 'stream: loop {
            tokio::select! {
                res = &mut completion => {
                    while let Ok(piece) = rx.try_recv() {
                        ui.stream_push(stream_handle, &piece);
                    }
                    ui.redraw()?;
                    break 'stream Some(res);
                }
                Some(piece) = rx.recv() => {
                    ui.stream_push(stream_handle, &piece);
                    ui.redraw()?;
                }
                _ = tokio::time::sleep(INPUT_TICK) => {}
            }
            if ui.poll_input().await? {
                break 'stream None;
            }
        };

        let reply = match reply {
            Some(Ok(reply)) => reply,
            Some(Err(e)) => {
                ui.stream_end(stream_handle, Some(format!("API error: {}", e)));
                ui.set_status(format!("API error: {}", e));
                break 'turn;
            }
            None => {
                ui.stream_end(stream_handle, Some("[Generation cancelled]".to_string()));
                ui.set_status("Generation cancelled by user".to_string());
                cancelled = true;
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
            // No tool call — finalize the streamed bubble as the answer.
            ui.stream_end(stream_handle, Some(reply.clone()));
            persist_assistant(ctx.chat_id, reply)?;
            break 'turn;
        };

        // The reply was a tool call: replace the streamed bubble with just the
        // accompanying prose (drop it entirely if there was none), then persist
        // the full reply (JSON included) so history can replay it.
        let stripped = strip_tool_call(&reply);
        ui.stream_end(
            stream_handle,
            (!stripped.is_empty()).then_some(stripped),
        );
        persist_assistant(ctx.chat_id, reply)?;
        ui.redraw()?;

        let nice = pretty_tool_name(&tool_call.tool);
        let cmd_text = command_text(&tool_call);

        let handle = ui.tool_begin(format!("{}: {}\nRunning...", nice, cmd_text));
        ui.redraw()?;

        // Race tool execution against cancellation too, so Esc works while a
        // (async) tool is in flight, not just while the model is thinking.
        let start = Instant::now();
        let mut exec = std::pin::pin!(ctx.worker.execute(&tool_call));
        let result = 'exec: loop {
            tokio::select! {
                res = &mut exec => break 'exec Some(res),
                _ = tokio::time::sleep(INPUT_TICK) => {}
            }
            if ui.poll_input().await? {
                break 'exec None;
            }
        };
        let result = match result {
            Some(result) => result,
            None => {
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

        ui.tool_finish(handle, cmd_text.clone(), result.success, result.output.clone());

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
