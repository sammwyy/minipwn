//! Slash command handler for TUI input.

use crate::config::Provider;
use crate::config::{
    SavedWorker, censor_key, clear_chat, list_chats, load_workers_list, save_workers_list, save_workspace_meta,
    save_global_config,
};

use super::app::App;

/// Handle a `/command` input from the user. Returns a status message.
pub async fn handle_command(app: &mut App, input: &str) -> String {
    let parts: Vec<&str> = input.trim_start_matches('/').splitn(3, ' ').collect();
    let cmd = parts[0].to_lowercase();

    match cmd.as_str() {
        "help" => cmd_help(),
        "provider" => cmd_provider(app, &parts[1..]),
        "apikey" => cmd_apikey(app, &parts[1..]),
        "model" => cmd_model(app, &parts[1..]).await,
        "chat" => cmd_chat(app, &parts[1..]),
        "clear" => cmd_clear(app),
        "worker" => cmd_worker(app, &parts[1..]),
        "theme" => cmd_theme(app, &parts[1..]),
        _ => format!("Unknown command: /{cmd}. Type /help for available commands."),
    }
}

fn cmd_help() -> String {
    "/provider                 List providers or select one\n\
     /provider <id>            Set active provider (openai|openrouter|custom)\n\
     /apikey                   Show censored API key\n\
     /apikey <key>             Set API key for current provider\n\
     /model                    List available models\n\
     /model <id>               Set active model\n\
     /chat                     Show current and available chats\n\
     /chat <id>                Switch to chat by ID\n\
     /clear                    Clear current chat history\n\
     /worker                   List saved workers\n\
     /worker add <url> <sec>   Add a worker\n\
     /theme                    List themes\n\
     /theme <id>               Set theme"
        .to_string()
}

fn cmd_provider(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        return format!(
            "Providers: openai, openrouter, custom\nCurrent: {}",
            app.provider.display_name()
        );
    }

    let id = args[0];
    match Provider::from_str(id) {
        Some(p) => {
            app.provider = p.clone();
            app.global_config.provider = id.to_lowercase();
            let _ = save_global_config(&app.global_config);
            format!("Provider set to: {}", p.display_name())
        }
        None => format!(
            "Unknown provider '{}'. Choose: openai, openrouter, custom",
            id
        ),
    }
}

fn cmd_apikey(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        let key = app.secrets.api_key(&app.provider).unwrap_or("(not set)");
        if key == "(not set)" {
            return format!("{} API key: (not set)", app.provider.display_name());
        }
        return format!(
            "{} API key: {}",
            app.provider.display_name(),
            censor_key(key)
        );
    }

    let key = args[0];
    let key_name = format!("{}_SECRETKEY", app.provider.prefix());
    match app.secrets.set(&key_name, key) {
        Ok(_) => format!("API key set for {}", app.provider.display_name()),
        Err(e) => format!("Error saving API key: {}", e),
    }
}

async fn cmd_model(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        match crate::ai::AiClient::from_secrets(&app.secrets, &app.provider) {
            Ok(client) => match client.list_models().await {
                Ok(models) => {
                    let current = app.secrets.model(&app.provider).unwrap_or("(not set)");
                    format!("Current: {}\n{}", current, models.join("\n"))
                }
                Err(e) => format!("Failed to list models: {}", e),
            },
            Err(e) => format!("Client error: {}", e),
        }
    } else {
        let model_id = args[0];
        let key_name = format!("{}_MODEL", app.provider.prefix());
        match app.secrets.set(&key_name, model_id) {
            Ok(_) => format!("Model set to: {}", model_id),
            Err(e) => format!("Error saving model: {}", e),
        }
    }
}

fn cmd_chat(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        let chats = list_chats().unwrap_or_default();
        return format!(
            "Current chat: {}\nAvailable: {}",
            app.chat_id,
            if chats.is_empty() {
                "(none)".to_string()
            } else {
                chats.join(", ")
            }
        );
    }

    let id = args[0];
    let session = crate::config::load_chat(id).unwrap_or_else(|_| crate::config::ChatSession {
        id: id.to_string(),
        messages: vec![],
    });

    app.chat_id = id.to_string();
    app.meta.current_chat = id.to_string();
    let _ = save_workspace_meta(&app.meta);

    app.bubbles = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|m| super::app::Bubble {
            role: m.role.clone(),
            content: m.content.clone(),
            is_ephemeral: false,
        })
        .collect();

    format!("Switched to chat: {}", id)
}

fn cmd_clear(app: &mut App) -> String {
    match clear_chat(&app.chat_id) {
        Ok(_) => {
            app.bubbles.clear();
            format!("Chat {} cleared", app.chat_id)
        }
        Err(e) => format!("Error clearing chat: {}", e),
    }
}

fn cmd_worker(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        let list = load_workers_list().unwrap_or_default();
        let current = match &app.execution_mode {
            crate::tools::ExecutionMode::Local { .. } => "local".to_string(),
            crate::tools::ExecutionMode::Remote { client, .. } => client.base_url.clone(),
        };

        let mut lines = vec![format!("Current worker: {}", current)];
        if list.workers.is_empty() {
            lines.push("No saved workers. Use /worker add <url> <secret> [name]".to_string());
        } else {
            lines.push("Saved workers:".to_string());
            for (i, w) in list.workers.iter().enumerate() {
                lines.push(format!("  {}: {} ({})", i, w.name, w.url));
            }
        }
        return lines.join("\n");
    }

    if args[0] == "add" {
        if args.len() < 3 {
            return "Usage: /worker add <url> <secret> [name]".to_string();
        }
        let url = args[1];
        let secret = args[2];
        let name = if args.len() > 3 { args[3] } else { "worker" };

        let mut list = load_workers_list().unwrap_or_default();
        list.workers.push(SavedWorker {
            name: name.to_string(),
            url: url.to_string(),
            secret: secret.to_string(),
        });
        match save_workers_list(&list) {
            Ok(_) => format!("Worker '{}' added at {}", name, url),
            Err(e) => format!("Error saving worker: {}", e),
        }
    } else {
        format!("Unknown subcommand: /worker {}", args[0])
    }
}

fn cmd_theme(app: &mut App, args: &[&str]) -> String {
    if args.is_empty() || args[0].is_empty() {
        let themes = app.theme_registry.list();
        let mut list = vec![format!("Current theme: {}", app.global_config.theme)];
        list.push("Available themes:".to_string());
        for (id, theme) in themes {
            list.push(format!("  - {} (by {})", id, theme.author));
        }
        return list.join("\n");
    }

    let id = args[0];
    match app.theme_registry.get(id) {
        Some(theme) => {
            app.theme = theme.clone();
            app.global_config.theme = id.to_string();
            let _ = save_global_config(&app.global_config);
            format!("Theme set to: {}", theme.name)
        }
        None => format!("Theme '{}' not found.", id),
    }
}
