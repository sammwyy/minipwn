//! Application state and main event loop.

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::time::Duration;

use crate::ai::{AiClient, ChatMsg};
use crate::commands::CommandRegistry;
use crate::config::{
    ChatMessage, GlobalConfig, Provider, SavedWorker, Secrets, WorkersList, WorkspaceMeta,
    WorkspaceStats, add_tokens, append_message, init_config_dirs, load_chat, load_global_config,
    load_workers_list, load_workspace_meta, load_workspace_stats, save_workers_list,
};
use crate::agent::{self, AgentUi};
use crate::tui::theme::{Theme, ThemeRegistry};
use crate::worker::{DockerWorker, LocalWorker, RemoteWorker, Worker};

use super::render::render_ui;
use super::screens::{WorkerChoice, worker_select_screen};

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
    pub worker: Arc<dyn Worker>,
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

    let worker = build_worker(choice, &workers, terminal, &theme).await?;

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
                let nice_tool_name = agent::pretty_tool_name(tool_name);

                let rest2 = &rest[bracket_idx + 2..];
                if let Some(newline_idx) = rest2.find('\n') {
                    let status_str = &rest2[..newline_idx];
                    let output = &rest2[newline_idx + 1..];

                    let brief = agent::summarize_output(output);

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
        worker,
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

/// A ratatui-backed [`AgentUi`]: bridges the agent loop to the [`App`] state
/// and the terminal. This is the only place the agent touches the TUI.
struct TuiAgentUi<'a> {
    app: &'a mut App,
    terminal: &'a mut Terminal<CrosstermBackend<Stdout>>,
}

#[async_trait::async_trait]
impl AgentUi for TuiAgentUi<'_> {
    fn assistant(&mut self, text: String, ephemeral: bool) {
        self.app.bubbles.push(Bubble {
            role: "assistant".to_string(),
            content: text,
            is_ephemeral: ephemeral,
        });
    }

    fn tool_begin(&mut self, content: String) -> usize {
        let idx = self.app.bubbles.len();
        self.app.bubbles.push(Bubble {
            role: "tool".to_string(),
            content,
            is_ephemeral: false,
        });
        idx
    }

    fn tool_update(&mut self, handle: usize, content: String) {
        if let Some(bubble) = self.app.bubbles.get_mut(handle) {
            bubble.content = content;
        }
    }

    fn record_tokens(&mut self, count: u64) {
        let _ = add_tokens(count);
        self.app.stats.total_tokens += count;
    }

    fn set_thinking(&mut self, thinking: bool) {
        self.app.is_thinking = thinking;
    }

    fn set_status(&mut self, status: String) {
        self.app.status = status;
    }

    fn redraw(&mut self) -> Result<()> {
        self.terminal.draw(|f| render_ui(f, self.app))?;
        Ok(())
    }

    async fn wait_cancel(&mut self) {
        loop {
            if crossterm::event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read() {
                    if k.code == crossterm::event::KeyCode::Esc
                        || (k.code == crossterm::event::KeyCode::Char('c')
                            && k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
                    {
                        return;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

/// Send a user message and run one agent turn against it.
async fn send_message(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    content: &str,
) -> Result<()> {
    // Record the user's message.
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

    // Build the AI client (surfacing config errors as a chat bubble).
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

    // Assemble the prompt: system prompt + the non-tool bubbles so far.
    let mut messages = vec![ChatMsg {
        role: "system".to_string(),
        content: agent::system_prompt(&app.meta.mode, app.worker.as_ref()).await,
    }];
    for bubble in &app.bubbles {
        if bubble.role != "tool" {
            messages.push(ChatMsg {
                role: bubble.role.clone(),
                content: bubble.content.clone(),
            });
        }
    }

    // Snapshot the bits the agent needs without borrowing `app` for the turn.
    let worker = Arc::clone(&app.worker);
    let chat_id = app.chat_id.clone();
    let max_iterations = app.global_config.max_iterations;

    let mut ui = TuiAgentUi { app, terminal };
    agent::run_turn(
        &mut ui,
        agent::TurnContext {
            ai: &ai_client,
            worker: worker.as_ref(),
            chat_id: &chat_id,
            max_iterations,
        },
        messages,
    )
    .await?;

    // Ephemeral bubbles stay on screen but are never persisted, so they vanish
    // on the next launch (which only reloads messages from disk).
    ui.app.scroll_offset = 0;
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

/// Build the active [`Worker`] from the startup selection.
async fn build_worker(
    choice: WorkerChoice,
    workers: &WorkersList,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
) -> Result<Arc<dyn Worker>> {
    let workspace = std::env::current_dir()?
        .join(".minipwn")
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    match choice {
        WorkerChoice::NoWorker => Ok(Arc::new(LocalWorker::new(workspace))),
        WorkerChoice::DockerKali => {
            let deployed =
                super::screens::docker_deploy_screen(terminal, theme, &workspace).await?;
            let worker = DockerWorker::new(&deployed, workspace);
            let validation = worker.client().validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!("Kali Docker worker validation failed for {}", deployed.url);
            }

            // Ephemeral: never persisted; the container is torn down on exit.
            Ok(Arc::new(worker))
        }
        WorkerChoice::Saved(idx) => {
            let w = &workers.workers[idx];
            let worker = RemoteWorker::new(w.name.clone(), &w.url, &w.secret, workspace);
            let validation = worker.client().validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!("Worker validation failed for {}", w.url);
            }
            Ok(Arc::new(worker))
        }
        WorkerChoice::New { url, secret, name } => {
            let worker = RemoteWorker::new(name.clone(), &url, &secret, workspace);
            let validation = worker.client().validate().await?;
            if !validation.ok || !validation.secret_valid {
                bail!("Worker validation failed for {}", url);
            }
            remember_worker(&name, &url, &secret);
            Ok(Arc::new(worker))
        }
    }
}

/// Persist a worker to the saved-workers list, updating any existing entry.
fn remember_worker(name: &str, url: &str, secret: &str) {
    let mut list = load_workers_list().unwrap_or_default();
    if let Some(existing) = list.workers.iter_mut().find(|w| w.url == url) {
        existing.name = name.to_string();
        existing.secret = secret.to_string();
    } else {
        list.workers.push(SavedWorker {
            name: name.to_string(),
            url: url.to_string(),
            secret: secret.to_string(),
        });
    }
    let _ = save_workers_list(&list);
}
