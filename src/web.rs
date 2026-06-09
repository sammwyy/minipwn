use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures_util::{stream::StreamExt, SinkExt};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;

use crate::agent::{run_turn, AgentUi, TurnContext};
use crate::ai::{AiClient, StreamPiece};
use crate::config::{
    default_provider, init_config_dirs, load_chat, load_global_config, load_workspace_meta,
    provider_from_id, ChatMessage, Secrets,
};
use crate::worker::LocalWorker;
use anyhow::Result;

async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn style() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../web/style.css"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../web/app.js"),
    )
}

async fn client_js() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../web/client.js"),
    )
}

pub async fn run(mut port: u16) -> Result<()> {
    crate::config::init_workspace()?;
    init_config_dirs()?;

    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(style))
        .route("/app.js", get(app_js))
        .route("/client.js", get(client_js))
        .route("/api", get(ws_handler));

    let listener = loop {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => break l,
            Err(_) => {
                port += 1;
            }
        }
    };

    println!("Web UI running at http://127.0.0.1:{}", port);

    axum::serve(listener, app).await.unwrap();
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket))
}

/// Outcome of a slash command run from the web UI.
struct WebCommandResult {
    output: String,
    /// The chat was cleared; the client should drop its history.
    cleared: bool,
    /// Provider/model changed; the client should resync its selectors.
    config_changed: bool,
}

impl WebCommandResult {
    fn text(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            cleared: false,
            config_changed: false,
        }
    }
}

/// Execute a slash command in the headless web context.
///
/// Commands that only make sense in the terminal (theme pickers, modal-driven
/// worker management) report that they are TUI-only rather than half-working.
fn run_web_command(
    cmd_line: &str,
    global_config: &mut crate::config::GlobalConfig,
    secrets: &mut Secrets,
    meta: &mut crate::config::WorkspaceMeta,
    chat_id: &str,
) -> WebCommandResult {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    let Some(first) = parts.first() else {
        return WebCommandResult::text("");
    };
    let cmd_name = first.strip_prefix('/').unwrap_or(first);
    let args = &parts[1..];

    match cmd_name {
        "help" => {
            let registry = crate::commands::CommandRegistry::new();
            let mut out = String::from("**Available commands**\n");
            for c in registry.commands {
                out.push_str(&format!("- `/{}` — {}\n", c.name(), c.description()));
            }
            WebCommandResult::text(out)
        }
        "usage" => {
            let chat = crate::config::chat_usage(chat_id).unwrap_or_default();
            let global = crate::config::global_usage().unwrap_or_default();
            let fmt = |w: &crate::config::UsageWindows| {
                format!(
                    "24h: {} · 7d: {} · 30d: {}",
                    w.last_24h, w.last_7d, w.last_30d
                )
            };
            WebCommandResult::text(format!(
                "**Token usage**\n- This chat ({}): {}\n- Global: {}",
                chat_id,
                fmt(&chat),
                fmt(&global)
            ))
        }
        "provider" => {
            if let Some(id) = args.first() {
                match provider_from_id(id) {
                    Some(p) => {
                        global_config.provider = p.id().to_string();
                        let _ = crate::config::save_global_config(global_config);
                        WebCommandResult {
                            output: format!("Provider set to **{}**", p.display_name()),
                            cleared: false,
                            config_changed: true,
                        }
                    }
                    None => WebCommandResult::text(format!("Unknown provider: `{}`", id)),
                }
            } else {
                WebCommandResult::text(format!("Current provider: **{}**", global_config.provider))
            }
        }
        "model" => {
            let provider =
                provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
            if let Some(id) = args.first() {
                let _ = secrets.set(&format!("{}_MODEL", provider.prefix()), id);
                WebCommandResult {
                    output: format!("Model set to **{}**", id),
                    cleared: false,
                    config_changed: true,
                }
            } else {
                WebCommandResult::text(format!(
                    "Current model: **{}**",
                    secrets.model(provider.as_ref()).unwrap_or("(default)")
                ))
            }
        }
        "apikey" => {
            let Some(key) = args.first() else {
                return WebCommandResult::text("Usage: `/apikey <key>`");
            };
            let provider =
                provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
            secrets.set_key(provider.as_ref(), key);
            let _ = secrets.save();
            WebCommandResult::text(format!("API key updated for **{}**", provider.display_name()))
        }
        "mode" => {
            if let Some(mode) = args.first() {
                if *mode == "safe" || *mode == "weaponized" {
                    meta.mode = mode.to_string();
                    let _ = crate::config::save_workspace_meta(meta);
                    WebCommandResult::text(format!("Mode set to **{}**", mode))
                } else {
                    WebCommandResult::text("Mode must be `safe` or `weaponized`")
                }
            } else {
                WebCommandResult::text(format!("Current mode: **{}**", meta.mode))
            }
        }
        "clear" => {
            let _ = crate::config::clear_chat(chat_id);
            WebCommandResult {
                output: "Chat cleared.".to_string(),
                cleared: true,
                config_changed: false,
            }
        }
        "chat" => {
            let chats = crate::config::list_chats().unwrap_or_default();
            WebCommandResult::text(format!(
                "Current chat: **{}**\nAvailable: {}",
                chat_id,
                if chats.is_empty() {
                    "(none)".to_string()
                } else {
                    chats.join(", ")
                }
            ))
        }
        "theme" | "worker" => {
            WebCommandResult::text(format!("`/{}` is only available in the TUI.", cmd_name))
        }
        other => WebCommandResult::text(format!("Unknown command: `/{}`", other)),
    }
}

struct WsAgentUi {
    tx: mpsc::UnboundedSender<serde_json::Value>,
    rx_cancel: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<()>>>,
}

#[async_trait::async_trait]
impl AgentUi for WsAgentUi {
    fn assistant(&mut self, text: String, ephemeral: bool) {
        let _ = self.tx.send(serde_json::json!({
            "type": "assistant",
            "text": text,
            "ephemeral": ephemeral,
        }));
    }

    fn stream_begin(&mut self) -> usize {
        let handle = rand::random::<usize>();
        let _ = self.tx.send(serde_json::json!({
            "type": "stream_begin",
            "handle": handle,
        }));
        handle
    }

    fn stream_push(&mut self, handle: usize, piece: &StreamPiece) {
        let (ptype, text) = match piece {
            StreamPiece::Reasoning(t) => ("reasoning", t),
            StreamPiece::Answer(t) => ("answer", t),
        };
        let _ = self.tx.send(serde_json::json!({
            "type": "stream_push",
            "handle": handle,
            "piece_type": ptype,
            "text": text,
        }));
    }

    fn stream_end(&mut self, handle: usize, text: Option<String>) {
        let _ = self.tx.send(serde_json::json!({
            "type": "stream_end",
            "handle": handle,
            "text": text,
        }));
    }

    fn tool_begin(&mut self, content: String) -> usize {
        let handle = rand::random::<usize>();
        let _ = self.tx.send(serde_json::json!({
            "type": "tool_begin",
            "handle": handle,
            "content": content,
        }));
        handle
    }

    fn tool_update(&mut self, handle: usize, content: String) {
        let _ = self.tx.send(serde_json::json!({
            "type": "tool_update",
            "handle": handle,
            "content": content,
        }));
    }

    fn tool_finish(&mut self, handle: usize, cmd: String, success: bool, output: String) {
        let _ = self.tx.send(serde_json::json!({
            "type": "tool_finish",
            "handle": handle,
            "cmd": cmd,
            "success": success,
            "output": output,
        }));
    }

    fn record_tokens(&mut self, count: u64) {
        let _ = self.tx.send(serde_json::json!({
            "type": "record_tokens",
            "count": count,
        }));
    }

    fn set_thinking(&mut self, thinking: bool) {
        let _ = self.tx.send(serde_json::json!({
            "type": "set_thinking",
            "thinking": thinking,
        }));
    }

    fn set_status(&mut self, status: String) {
        let _ = self.tx.send(serde_json::json!({
            "type": "set_status",
            "status": status,
        }));
    }

    fn redraw(&mut self) -> Result<()> {
        Ok(())
    }

    async fn poll_input(&mut self) -> Result<bool> {
        let mut rx = self.rx_cancel.lock().await;
        if let Ok(_) = rx.try_recv() {
            return Ok(true);
        }
        Ok(false)
    }
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let (tx_ws, mut rx_ws) = mpsc::unbounded_channel::<serde_json::Value>();
    let (tx_action, mut rx_action) = mpsc::unbounded_channel::<serde_json::Value>();
    let (tx_cancel, rx_cancel) = mpsc::unbounded_channel::<()>();

    tokio::spawn(async move {
        while let Some(val) = rx_ws.recv().await {
            if sender.send(Message::Text(val.to_string())).await.is_err() {
                break;
            }
        }
    });

    let rx_cancel = Arc::new(tokio::sync::Mutex::new(rx_cancel));

    tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if json["action"] == "cancel" {
                        let _ = tx_cancel.send(());
                    } else {
                        let _ = tx_action.send(json);
                    }
                }
            }
        }
    });

    let mut ui = WsAgentUi {
        tx: tx_ws.clone(),
        rx_cancel: rx_cancel.clone(),
    };

    let mut global_config = load_global_config().unwrap_or_default();
    let mut meta = load_workspace_meta().unwrap_or_default();
    let mut secrets = Secrets::load().unwrap_or_default();

    // Get list of providers that have a (non-empty) API key configured.
    let mut available_providers = vec![];
    for p in crate::config::all_providers() {
        if secrets
            .api_key(p.as_ref())
            .filter(|k| !k.is_empty())
            .is_some()
        {
            available_providers.push(serde_json::json!({
                "id": p.id(),
                "name": p.display_name(),
            }));
        }
    }

    let workspace = std::env::current_dir()
        .unwrap_or_default()
        .join(".minipwn")
        .parent()
        .unwrap_or(&std::env::current_dir().unwrap_or_default())
        .to_path_buf();

    let worker = Arc::new(LocalWorker::new(workspace));
    let chat_id = meta.current_chat.clone();

    let session = load_chat(&chat_id).unwrap_or_else(|_| crate::config::ChatSession {
        id: chat_id.clone(),
        messages: vec![],
    });

    let active_provider = provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
    let active_model = secrets.model(active_provider.as_ref()).unwrap_or("gpt-4o-mini").to_string();

    let registry = crate::commands::CommandRegistry::new();
    let mut commands = vec![];
    for c in registry.commands {
        commands.push(serde_json::json!({
            "name": c.name(),
            "aliases": c.aliases(),
            "description": c.description(),
            "usage": c.usage(),
        }));
    }

    let _ = tx_ws.send(serde_json::json!({
        "type": "init",
        "chat_id": chat_id,
        "history": session.messages,
        "provider": global_config.provider,
        "model": active_model,
        "providers": available_providers,
        "commands": commands,
    }));

    while let Some(action) = rx_action.recv().await {
        let action_str = action["action"].as_str().unwrap_or("");
        match action_str {
            "send" => {
                if let Some(input) = action["message"].as_str() {
                    let input = input.to_string();
                    crate::config::append_message(
                        &chat_id,
                        ChatMessage {
                            role: "user".to_string(),
                            content: input.clone(),
                            timestamp: chrono::Utc::now(),
                        },
                    )
                    .unwrap();

                    let provider = provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
                    let ai_client = match AiClient::from_secrets(&secrets, provider.as_ref()) {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = tx_ws.send(serde_json::json!({
                                "type": "assistant",
                                "text": format!("Error: {}", e),
                                "ephemeral": true
                            }));
                            let _ = tx_ws.send(serde_json::json!({
                                "type": "turn_end"
                            }));
                            continue;
                        }
                    };

                    let current_session = load_chat(&chat_id).unwrap_or_else(|_| crate::config::ChatSession {
                        id: chat_id.clone(),
                        messages: vec![],
                    });
                    
                    let mut turn_msgs = vec![crate::ai::ChatMsg {
                        role: "system".to_string(),
                        content: crate::agent::system_prompt(&meta.mode, worker.as_ref()).await,
                    }];
                    
                    for m in current_session.messages.iter().take(current_session.messages.len().saturating_sub(1)) {
                        turn_msgs.push(crate::ai::ChatMsg {
                            role: m.role.clone(),
                            content: m.content.clone(),
                        });
                    }
                    
                    turn_msgs.push(crate::ai::ChatMsg {
                        role: "user".to_string(),
                        content: input.clone(),
                    });

                    let ctx = TurnContext {
                        ai: &ai_client,
                        worker: worker.as_ref(),
                        chat_id: &chat_id,
                        max_iterations: global_config.max_iterations,
                    };

                    let _ = run_turn(&mut ui, ctx, turn_msgs).await;
                    
                    let _ = tx_ws.send(serde_json::json!({
                        "type": "turn_end"
                    }));
                }
            }
            "get_models" => {
                if let Some(p_id) = action["provider"].as_str() {
                    if let Some(p) = provider_from_id(p_id) {
                        let tx_ws_clone = tx_ws.clone();
                        let ai_client = AiClient::from_secrets(&secrets, p.as_ref());
                        let p_id_str = p_id.to_string();
                        tokio::spawn(async move {
                            if let Ok(c) = ai_client {
                                if let Ok(models) = c.list_models().await {
                                    let _ = tx_ws_clone.send(serde_json::json!({
                                        "type": "models_list",
                                        "provider": p_id_str,
                                        "models": models,
                                    }));
                                }
                            }
                        });
                    }
                }
            }
            "set_provider" => {
                if let Some(p_id) = action["provider"].as_str() {
                    global_config.provider = p_id.to_string();
                    let _ = crate::config::save_global_config(&global_config);
                }
            }
            "set_model" => {
                if let Some(m_id) = action["model"].as_str() {
                    let p = provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
                    let _ = secrets.set(&format!("{}_MODEL", p.prefix()), m_id);
                }
            }
            "command" => {
                if let Some(cmd_line) = action["command"].as_str() {
                    let result = run_web_command(
                        cmd_line,
                        &mut global_config,
                        &mut secrets,
                        &mut meta,
                        &chat_id,
                    );

                    let _ = tx_ws.send(serde_json::json!({
                        "type": "assistant",
                        "text": result.output,
                        "ephemeral": true,
                    }));

                    if result.cleared {
                        let _ = tx_ws.send(serde_json::json!({ "type": "cleared" }));
                    }
                    if result.config_changed {
                        let provider =
                            provider_from_id(&global_config.provider).unwrap_or_else(default_provider);
                        let _ = tx_ws.send(serde_json::json!({
                            "type": "config_update",
                            "provider": global_config.provider,
                            "model": secrets.model(provider.as_ref()).unwrap_or(""),
                        }));
                    }
                }
            }
            _ => {}
        }
    }
}
