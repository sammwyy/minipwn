//! Application state and main event loop.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::time::Duration;

use crate::ai::{AiClient, ChatMsg};
use crate::config::{
    ChatMessage, Provider, SavedWorker, Secrets, WorkersList, WorkspaceMeta, append_message, load_chat, load_workers_list, load_workspace_meta,
    save_workers_list,
};
use crate::config::load_system_prompt;
use crate::tools::{ExecutionMode, extract_tool_call};
use crate::worker::client::WorkerClient;

use super::commands::handle_command;
use super::render::render_ui;
use super::worker_select::{WorkerChoice, worker_select_screen};

/// A displayed chat bubble.
#[derive(Debug, Clone)]
pub struct Bubble {
    pub role: String, // "user" | "assistant" | "tool"
    pub content: String,
}

/// Main application state.
pub struct App {
    pub chat_id: String,
    pub bubbles: Vec<Bubble>,
    pub input: String,
    pub cursor: usize,
    pub status: String,
    pub provider: Provider,
    pub secrets: Secrets,
    pub meta: WorkspaceMeta,
    pub execution_mode: ExecutionMode,
    pub is_thinking: bool,
    pub scroll_offset: u16,
    pub input_history: Vec<String>,
    pub input_history_pos: usize,
}

impl App {
    fn workspace_path() -> std::path::PathBuf {
        std::env::current_dir().unwrap_or_default().join(".minipwn")
    }
}

/// Main TUI event loop.
pub async fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    // Show worker selection screen
    let workers = load_workers_list().unwrap_or_default();
    let choice = worker_select_screen(terminal, &workers).await?;

    let execution_mode = build_execution_mode(choice, &workers).await?;

    // Load workspace state
    let meta = load_workspace_meta().unwrap_or_default();
    let secrets = Secrets::load().unwrap_or_default();
    let provider = Provider::from_str(&meta.provider).unwrap_or(Provider::OpenAI);

    // Load recent chat history (last 10 messages)
    let session = load_chat(&meta.current_chat).unwrap_or_else(|_| crate::config::ChatSession {
        id: meta.current_chat.clone(),
        messages: vec![],
    });

    let bubbles: Vec<Bubble> = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|m| Bubble {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let chat_id = meta.current_chat.clone();

    let mut app = App {
        chat_id,
        bubbles,
        input: String::new(),
        cursor: 0,
        status: String::new(),
        provider,
        secrets,
        meta,
        execution_mode,
        is_thinking: false,
        scroll_offset: 0,
        input_history: vec![],
        input_history_pos: 0,
    };

    loop {
        // Draw frame
        terminal.draw(|f| render_ui(f, &app))?;

        // Poll for input with timeout so we can handle async
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Ctrl-C / Ctrl-Q: quit
                if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }

                match key.code {
                    KeyCode::Enter => {
                        let input = app.input.trim().to_string();
                        if input.is_empty() {
                            continue;
                        }

                        // Save to input history
                        if app.input_history.last().map(|s: &String| s.as_str()) != Some(&input) {
                            app.input_history.push(input.clone());
                        }
                        app.input_history_pos = app.input_history.len();
                        app.input.clear();
                        app.cursor = 0;

                        if input.starts_with('/') {
                            // Handle slash command
                            let result = handle_command(&mut app, &input).await;
                            app.status = result;
                        } else {
                            // Send message to AI
                            send_message(&mut app, terminal, &input).await?;
                        }
                    }
                    KeyCode::Char(c) => {
                        app.input.insert(app.cursor, c);
                        app.cursor += 1;
                    }
                    KeyCode::Backspace => {
                        if app.cursor > 0 {
                            app.cursor -= 1;
                            app.input.remove(app.cursor);
                        }
                    }
                    KeyCode::Delete => {
                        if app.cursor < app.input.len() {
                            app.input.remove(app.cursor);
                        }
                    }
                    KeyCode::Left => {
                        if app.cursor > 0 {
                            app.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if app.cursor < app.input.len() {
                            app.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        app.cursor = 0;
                    }
                    KeyCode::End => {
                        app.cursor = app.input.len();
                    }
                    KeyCode::Up => {
                        if !app.input_history.is_empty() && app.input_history_pos > 0 {
                            app.input_history_pos -= 1;
                            app.input = app.input_history[app.input_history_pos].clone();
                            app.cursor = app.input.len();
                        }
                    }
                    KeyCode::Down => {
                        if app.input_history_pos < app.input_history.len() {
                            app.input_history_pos += 1;
                            if app.input_history_pos == app.input_history.len() {
                                app.input.clear();
                                app.cursor = 0;
                            } else {
                                app.input = app.input_history[app.input_history_pos].clone();
                                app.cursor = app.input.len();
                            }
                        }
                    }
                    KeyCode::PageUp => {
                        app.scroll_offset = app.scroll_offset.saturating_add(5);
                    }
                    KeyCode::PageDown => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(5);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Send a user message, call the AI, process any tool calls, and update state.
async fn send_message(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    content: &str,
) -> Result<()> {
    // Add user bubble
    app.bubbles.push(Bubble {
        role: "user".to_string(),
        content: content.to_string(),
    });
    append_message(
        &app.chat_id,
        ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
        },
    )?;

    // Build AI client
    let ai_client = match AiClient::from_secrets(&app.secrets, &app.provider) {
        Ok(c) => c,
        Err(e) => {
            app.status = format!("AI error: {}", e);
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: format!("Error: {}. Use /provider and /apikey to configure.", e),
            });
            return Ok(());
        }
    };

    // Build message list: system prompt + all bubbles
    let system_prompt = build_system_prompt(app).await;
    let mut messages = vec![ChatMsg {
        role: "system".to_string(),
        content: system_prompt,
    }];

    for bubble in &app.bubbles {
        if bubble.role != "tool" {
            messages.push(ChatMsg {
                role: bubble.role.clone(),
                content: bubble.content.clone(),
            });
        }
    }

    // Agentic loop: AI → tool call → result → AI (up to 10 iterations)
    app.is_thinking = true;
    terminal.draw(|f| render_ui(f, app))?;

    let mut iterations = 0;
    let max_iterations = 10;

    loop {
        iterations += 1;
        if iterations > max_iterations {
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: "[Max tool iterations reached]".to_string(),
            });
            break;
        }

        let reply = match ai_client.complete(messages.clone()).await {
            Ok(r) => r,
            Err(e) => {
                app.status = format!("API error: {}", e);
                app.bubbles.push(Bubble {
                    role: "assistant".to_string(),
                    content: format!("API error: {}", e),
                });
                break;
            }
        };

        // Add assistant reply to message history
        messages.push(ChatMsg {
            role: "assistant".to_string(),
            content: reply.clone(),
        });

        // Check for a tool call in the reply
        if let Some(tool_call) = extract_tool_call(&reply) {
            // Show the AI message with tool invocation
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: reply.clone(),
            });
            append_message(
                &app.chat_id,
                ChatMessage {
                    role: "assistant".to_string(),
                    content: reply.clone(),
                    timestamp: chrono::Utc::now(),
                },
            )?;

            terminal.draw(|f| render_ui(f, app))?;

            // Execute the tool
            let result = app.execution_mode.execute(&tool_call).await;
            let result_text = format!(
                "[Tool: {}] {}\n{}",
                tool_call.tool,
                if result.success { "OK" } else { "FAILED" },
                result.output
            );

            app.bubbles.push(Bubble {
                role: "tool".to_string(),
                content: result_text.clone(),
            });

            // Feed result back to AI
            messages.push(ChatMsg {
                role: "user".to_string(),
                content: format!("Tool result:\n{}", result_text),
            });

            terminal.draw(|f| render_ui(f, app))?;
        } else {
            // No tool call — final response
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: reply.clone(),
            });
            append_message(
                &app.chat_id,
                ChatMessage {
                    role: "assistant".to_string(),
                    content: reply,
                    timestamp: chrono::Utc::now(),
                },
            )?;
            break;
        }
    }

    app.is_thinking = false;
    app.scroll_offset = 0;
    Ok(())
}

/// Build the system prompt, injecting worker OS info if available.
async fn build_system_prompt(app: &App) -> String {
    let base = load_system_prompt();

    let worker_info = match &app.execution_mode {
        ExecutionMode::Remote { client, .. } => match client.get_info().await {
            Ok(info) => format!(
                "Remote worker: {} ({}), hostname: {}, cwd: {}",
                info.os, info.arch, info.hostname, info.cwd
            ),
            Err(_) => "Remote worker (info unavailable)".to_string(),
        },
        ExecutionMode::Local { .. } => format!(
            "Local execution: {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    };

    base.replace("{{WORKER_INFO}}", &worker_info)
}

/// Build the execution mode from the worker selection.
async fn build_execution_mode(
    choice: WorkerChoice,
    workers: &WorkersList,
) -> Result<ExecutionMode> {
    let workspace = std::env::current_dir()?
        .join(".minipwn")
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    match choice {
        WorkerChoice::NoWorker => Ok(ExecutionMode::Local { workspace }),
        WorkerChoice::Saved(idx) => {
            let w = &workers.workers[idx];
            let client = WorkerClient::new(&w.url, &w.secret);
            Ok(ExecutionMode::Remote { client, workspace })
        }
        WorkerChoice::New { url, secret, name } => {
            // Save the new worker
            let mut list = load_workers_list().unwrap_or_default();
            list.workers.push(SavedWorker {
                name,
                url: url.clone(),
                secret: secret.clone(),
            });
            let _ = save_workers_list(&list);

            let client = WorkerClient::new(&url, &secret);
            Ok(ExecutionMode::Remote { client, workspace })
        }
    }
}
