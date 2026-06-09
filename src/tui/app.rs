//! Application state and main event loop.

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::time::Duration;

use crate::ai::{AiClient, ChatMsg};
use crate::commands::CommandRegistry;
use crate::config::load_system_prompt;
use crate::config::{
    ChatMessage, GlobalConfig, Provider, SavedWorker, Secrets, WorkersList, WorkspaceMeta,
    WorkspaceStats, add_tokens, append_message, init_config_dirs, load_chat, load_global_config,
    load_workers_list, load_workspace_meta, load_workspace_stats, save_workers_list,
};
use crate::tools::{ExecutionMode, extract_tool_call};
use crate::tui::theme::{Theme, ThemeRegistry};
use crate::worker::client::WorkerClient;

use super::render::render_ui;
use super::worker_select::{WorkerChoice, worker_select_screen};

/// A displayed chat bubble.
#[derive(Debug, Clone)]
pub struct Bubble {
    pub role: String, // "user" | "assistant" | "tool"
    pub content: String,
    pub is_ephemeral: bool,
}

#[derive(Debug, Clone)]
pub struct ModalItem {
    pub id: String,
    pub label: String,
}

use std::sync::Arc;

pub struct ModalCallback(pub Arc<dyn Fn(&mut App, String) + Send + Sync>);

impl std::fmt::Debug for ModalCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModalCallback")
    }
}

impl Clone for ModalCallback {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[derive(Debug, Clone)]
pub struct ModalState {
    pub title: String,
    pub items: Vec<ModalItem>,
    pub selected: usize,
    pub filter: String,
    pub callback: ModalCallback,
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
    pub stats: WorkspaceStats,
    pub global_config: GlobalConfig,
    pub theme: Theme,
    pub theme_registry: ThemeRegistry,
    pub execution_mode: ExecutionMode,
    pub is_thinking: bool,
    pub scroll_offset: u16,
    pub input_history: Vec<String>,
    pub input_history_pos: usize,
    pub suggestions: Vec<String>,
    pub modal: Option<ModalState>,
}

impl App {}

/// Main TUI event loop.
pub async fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    // Initialize global config
    init_config_dirs()?;
    let global_config = load_global_config().unwrap_or_default();

    // Setup themes
    let theme_registry = ThemeRegistry::load();
    let theme = theme_registry
        .get(&global_config.theme)
        .cloned()
        .unwrap_or_else(|| theme_registry.get("dracula").unwrap().clone());

    // Show worker selection screen
    let workers = load_workers_list().unwrap_or_default();
    let choice = worker_select_screen(terminal, &workers, &theme).await?;
    let workers = load_workers_list().unwrap_or_default();

    let execution_mode = build_execution_mode(choice, &workers, terminal, &theme).await?;

    // Load workspace state
    let meta = load_workspace_meta().unwrap_or_default();
    let stats = load_workspace_stats().unwrap_or_default();
    let secrets = Secrets::load().unwrap_or_default();
    let provider = Provider::from_str(&global_config.provider).unwrap_or(Provider::OpenAI);

    // Load recent chat history (last 10 messages)
    let session = load_chat(&meta.current_chat).unwrap_or_else(|_| crate::config::ChatSession {
        id: meta.current_chat.clone(),
        messages: vec![],
    });

    let mut bubbles = Vec::new();
    let history_messages: Vec<_> = session.messages.iter().rev().take(20).rev().collect();

    for m in history_messages {
        if m.role == "assistant" {
            let stripped = crate::tools::strip_tool_call(&m.content);
            if !stripped.is_empty() {
                bubbles.push(Bubble {
                    role: m.role.clone(),
                    content: stripped,
                    is_ephemeral: false,
                });
            }
        } else if m.role == "user" && m.content.starts_with("Tool result:\n[Tool: ") {
            let rest = &m.content["Tool result:\n[Tool: ".len()..];
            if let Some(bracket_idx) = rest.find(']') {
                let tool_name = &rest[..bracket_idx];
                let nice_tool_name = tool_name
                    .split('_')
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                let rest2 = &rest[bracket_idx + 2..];
                if let Some(newline_idx) = rest2.find('\n') {
                    let status_str = &rest2[..newline_idx];
                    let output = &rest2[newline_idx + 1..];

                    let mut brief = output.replace('\n', " ");
                    if brief.chars().count() > 32 {
                        brief = format!("{}...", brief.chars().take(32).collect::<String>());
                    }

                    let status_prefix = if status_str == "OK" {
                        "Success:"
                    } else {
                        "Error:"
                    };

                    bubbles.push(Bubble {
                        role: "tool".to_string(),
                        content: format!("{}\n{} {}", nice_tool_name, status_prefix, brief),
                        is_ephemeral: false,
                    });
                }
            }
        } else {
            bubbles.push(Bubble {
                role: m.role.clone(),
                content: m.content.clone(),
                is_ephemeral: false,
            });
        }
    }

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
        stats,
        global_config,
        theme,
        theme_registry,
        execution_mode,
        is_thinking: false,
        scroll_offset: 0,
        input_history: vec![],
        input_history_pos: 0,
        suggestions: vec![],
        modal: None,
    };

    loop {
        // Draw frame
        terminal.draw(|f| render_ui(f, &app))?;

        // Poll for input with timeout so we can handle async
        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Event::Key(key) = ev {
                // Ctrl-C / Ctrl-Q: quit
                if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }
                if let Some(modal) = &mut app.modal {
                    match key.code {
                        KeyCode::Esc => {
                            app.modal = None;
                        }
                        KeyCode::Up => {
                            if modal.selected > 0 {
                                modal.selected -= 1;
                            }
                        }
                        KeyCode::Down => {
                            let filtered_count = modal
                                .items
                                .iter()
                                .filter(|i| {
                                    i.label
                                        .to_lowercase()
                                        .contains(&modal.filter.to_lowercase())
                                })
                                .count();
                            if modal.selected + 1 < filtered_count {
                                modal.selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            let filtered: Vec<_> = modal
                                .items
                                .iter()
                                .filter(|i| {
                                    i.label
                                        .to_lowercase()
                                        .contains(&modal.filter.to_lowercase())
                                })
                                .collect();
                            if let Some(item) = filtered.get(modal.selected) {
                                let id = item.id.clone();
                                let callback = modal.callback.clone();
                                app.modal = None;
                                (callback.0)(&mut app, id);
                            }
                        }
                        KeyCode::Char(c) => {
                            modal.filter.push(c);
                            modal.selected = 0;
                        }
                        KeyCode::Backspace => {
                            modal.filter.pop();
                            modal.selected = 0;
                        }
                        _ => {}
                    }
                    continue;
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
                        app.suggestions.clear();

                        if input.starts_with('/') {
                            // Handle modular command
                            let parts: Vec<&str> = input.split_whitespace().collect();
                            let cmd_name = parts[0].trim_start_matches('/');
                            let args = &parts[1..];

                            let registry = CommandRegistry::new();
                            if let Some(cmd) = registry.find(cmd_name) {
                                match cmd.execute(&mut app, cmd_name, args).await {
                                    Ok(result) => {
                                        if !result.is_empty() {
                                            app.bubbles.push(Bubble {
                                                role: "assistant".to_string(),
                                                content: result,
                                                is_ephemeral: true,
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        app.bubbles.push(Bubble {
                                            role: "assistant".to_string(),
                                            content: format!("Command error: {}", e),
                                            is_ephemeral: true,
                                        });
                                    }
                                }
                            } else {
                                app.bubbles.push(Bubble {
                                    role: "assistant".to_string(),
                                    content: format!("Unknown command: /{}", cmd_name),
                                    is_ephemeral: true,
                                });
                            }
                        } else {
                            // Send message to AI
                            send_message(&mut app, terminal, &input).await?;
                        }
                    }
                    KeyCode::Char(c) => {
                        app.input.insert(app.cursor, c);
                        app.cursor += 1;
                        update_suggestions(&mut app);
                    }
                    KeyCode::Backspace => {
                        if app.cursor > 0 {
                            app.cursor -= 1;
                            app.input.remove(app.cursor);
                            update_suggestions(&mut app);
                        }
                    }
                    KeyCode::Delete => {
                        if app.cursor < app.input.len() {
                            app.input.remove(app.cursor);
                            update_suggestions(&mut app);
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
                            update_suggestions(&mut app);
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
                            update_suggestions(&mut app);
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
            } else if let Event::Mouse(mouse) = ev {
                match mouse.kind {
                    event::MouseEventKind::ScrollUp => {
                        app.scroll_offset = app.scroll_offset.saturating_add(3);
                    }
                    event::MouseEventKind::ScrollDown => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(3);
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
        is_ephemeral: false,
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
                is_ephemeral: false,
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
    let max_iterations = app.global_config.max_iterations;

    loop {
        iterations += 1;
        if iterations > max_iterations {
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: "[Max tool iterations reached. Type 'Continue' to keep going.]"
                    .to_string(),
                is_ephemeral: false,
            });
            break;
        }

        let reply_result = tokio::select! {
            res = ai_client.complete(messages.clone()) => Some(res),
            _ = async {
                loop {
                    if crossterm::event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                        if let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read() {
                            if k.code == crossterm::event::KeyCode::Esc ||
                               (k.code == crossterm::event::KeyCode::Char('c') && k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)) {
                                break;
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            } => None,
        };

        let reply = match reply_result {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                app.status = format!("API error: {}", e);
                app.bubbles.push(Bubble {
                    role: "assistant".to_string(),
                    content: format!("API error: {}", e),
                    is_ephemeral: false,
                });
                break;
            }
            None => {
                app.status = "Generation cancelled by user".to_string();
                app.bubbles.push(Bubble {
                    role: "assistant".to_string(),
                    content: "[Generation cancelled]".to_string(),
                    is_ephemeral: true,
                });
                break;
            }
        };

        // Estimate tokens (very rough: chars / 4)
        let sent_tokens = messages.iter().map(|m| m.content.len()).sum::<usize>() / 4;
        let recv_tokens = reply.len() / 4;
        let total = (sent_tokens + recv_tokens) as u64;
        let _ = add_tokens(total);
        app.stats.total_tokens += total;

        // Add assistant reply to message history
        messages.push(ChatMsg {
            role: "assistant".to_string(),
            content: reply.clone(),
        });

        // Check for a tool call in the reply
        if let Some(tool_call) = extract_tool_call(&reply) {
            // Show the AI message with tool invocation (stripped of JSON)
            let stripped_reply = crate::tools::strip_tool_call(&reply);
            if !stripped_reply.is_empty() {
                app.bubbles.push(Bubble {
                    role: "assistant".to_string(),
                    content: stripped_reply,
                    is_ephemeral: false,
                });
            }

            append_message(
                &app.chat_id,
                ChatMessage {
                    role: "assistant".to_string(),
                    content: reply.clone(), // Keep full reply in persistent history
                    timestamp: chrono::Utc::now(),
                },
            )?;

            terminal.draw(|f| render_ui(f, app))?;

            let nice_tool_name = tool_call
                .tool
                .split('_')
                .map(|s| {
                    let mut c = s.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let cmd_text = if tool_call.tool == "shell_exec" {
                tool_call
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                tool_call.args.to_string()
            };

            // Push a "Running..." bubble
            let tool_bubble_idx = app.bubbles.len();
            app.bubbles.push(Bubble {
                role: "tool".to_string(),
                content: format!("{}: {}\nRunning...", nice_tool_name, cmd_text),
                is_ephemeral: false,
            });
            terminal.draw(|f| render_ui(f, app))?;

            // Execute the tool
            let start_time = std::time::Instant::now();
            let result = app.execution_mode.execute(&tool_call).await;
            let elapsed = start_time.elapsed();

            let full_result_text = format!(
                "[Tool: {}] {}\n{}",
                tool_call.tool,
                if result.success { "OK" } else { "FAILED" },
                result.output
            );

            // Create a brief summary for the UI
            let mut brief = result.output.replace('\n', " ");
            if brief.chars().count() > 32 {
                brief = format!("{}...", brief.chars().take(32).collect::<String>());
            }

            let secs = elapsed.as_secs();
            let ms = elapsed.subsec_millis();
            let time_str = format!("{:02}:{:02}.{:03}", secs / 60, secs % 60, ms);

            let status_prefix = if result.success { "Success:" } else { "Error:" };

            app.bubbles[tool_bubble_idx].content = format!(
                "{}: {}\n{} {} ({})",
                nice_tool_name, cmd_text, status_prefix, brief, time_str
            );

            // Feed result back to AI
            messages.push(ChatMsg {
                role: "user".to_string(),
                content: format!("Tool result:\n{}", full_result_text),
            });

            terminal.draw(|f| render_ui(f, app))?;
        } else {
            // No tool call — final response
            app.bubbles.push(Bubble {
                role: "assistant".to_string(),
                content: reply.clone(),
                is_ephemeral: false,
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

    // Clean up ephemeral bubbles when user sends a new message
    // Actually, user wants them to stay until next message or something?
    // "chat bubble que no quede en el historial"
    // I'll keep them in app.bubbles but they won't be saved.
    // When run_app starts, it only loads from disk, so ephemeral bubbles disappear.

    app.is_thinking = false;
    app.scroll_offset = 0;
    Ok(())
}

fn update_suggestions(app: &mut App) {
    if !app.input.starts_with('/') {
        app.suggestions.clear();
        return;
    }

    let cmd_part = app.input.trim_start_matches('/');
    let registry = CommandRegistry::new();
    let commands = registry.commands;

    let mut filtered: Vec<String> = commands
        .iter()
        .filter(|c| {
            c.name().starts_with(cmd_part) || c.aliases().iter().any(|a| a.starts_with(cmd_part))
        })
        .map(|c| format!("{} | {}", c.usage(), c.description()))
        .collect();

    // Optional: Sort or limit
    filtered.sort();
    app.suggestions = filtered;
}

/// Build the system prompt, injecting worker OS info if available.
async fn build_system_prompt(app: &App) -> String {
    let base = load_system_prompt(&app.meta.mode);

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
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
) -> Result<ExecutionMode> {
    let workspace = std::env::current_dir()?
        .join(".minipwn")
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    match choice {
        WorkerChoice::NoWorker => Ok(ExecutionMode::Local { workspace }),
        WorkerChoice::DockerKali => {
            let docker_worker =
                super::worker_select::docker_deploy_screen(terminal, theme, &workspace).await?;
            let client = WorkerClient::new(&docker_worker.url, &docker_worker.secret);
            let validation = client.validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!(
                    "Kali Docker worker validation failed for {}",
                    docker_worker.url
                );
            }

            let mut list = load_workers_list().unwrap_or_default();
            if let Some(existing) = list.workers.iter_mut().find(|w| w.url == docker_worker.url) {
                existing.name = docker_worker.name;
                existing.secret = docker_worker.secret;
            } else {
                list.workers.push(SavedWorker {
                    name: docker_worker.name,
                    url: docker_worker.url.clone(),
                    secret: docker_worker.secret,
                });
            }
            let _ = save_workers_list(&list);

            Ok(ExecutionMode::Remote { client, workspace })
        }
        WorkerChoice::Saved(idx) => {
            let w = &workers.workers[idx];
            let client = WorkerClient::new(&w.url, &w.secret);
            let validation = client.validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!("Worker validation failed for {}", w.url);
            }
            Ok(ExecutionMode::Remote { client, workspace })
        }
        WorkerChoice::New { url, secret, name } => {
            let client = WorkerClient::new(&url, &secret);
            let validation = client.validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!("Worker validation failed for {}", url);
            }

            // Save the new worker
            let mut list = load_workers_list().unwrap_or_default();
            if let Some(existing) = list.workers.iter_mut().find(|w| w.url == url) {
                existing.name = name;
                existing.secret = secret;
            } else {
                list.workers.push(SavedWorker {
                    name,
                    url: url.clone(),
                    secret,
                });
            }
            let _ = save_workers_list(&list);

            Ok(ExecutionMode::Remote { client, workspace })
        }
    }
}
