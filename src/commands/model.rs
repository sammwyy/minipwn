use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::tui::app::{App, ModalState, ModalItem, ModalCallback};
use crate::config::save_workspace_meta;
use super::Command;

pub struct ModelCommand {}

#[async_trait]
impl Command for ModelCommand {
    fn name(&self) -> &str { "model" }
    fn aliases(&self) -> Vec<&str> { vec!["models"] }
    fn description(&self) -> &str { "Change AI model" }
    fn usage(&self) -> &str { "/model [id]" }

    async fn execute(&self, app: &mut App, args: &[&str]) -> Result<String> {
        if !args.is_empty() && !args[0].is_empty() {
            let model_id = args[0];
            let key = format!("{}_MODEL", app.provider.prefix());
            app.secrets.set(&key, model_id)?;
            return Ok(format!("Model changed to {}", model_id));
        }

        // Fetch models from provider
        let mut items = Vec::new();
        if let Ok(client) = crate::ai::AiClient::from_secrets(&app.secrets, &app.provider) {
            app.status = format!("Fetching models from {}...", app.provider.display_name());
            if let Ok(models) = client.list_models().await {
                items = models.into_iter()
                    .map(|m| ModalItem { id: m.to_string(), label: m.to_string() })
                    .collect();
            }
        }

        if items.is_empty() {
            // Fallback list
            let models = vec![
                "gpt-4o", "gpt-4-turbo", "gpt-3.5-turbo",
                "claude-3-opus", "claude-3-sonnet", "claude-3-haiku",
                "gemini-1.5-pro", "gemini-1.5-flash",
                "llama-3-70b", "llama-3-8b",
                "mistral-large", "mixtral-8x7b",
            ];
            items = models.into_iter()
                .map(|m| ModalItem { id: m.to_string(), label: m.to_string() })
                .collect();
        }

        app.modal = Some(ModalState {
            title: "Select Model".to_string(),
            items,
            selected: 0,
            filter: String::new(),
            callback: ModalCallback(Arc::new(|app, id| {
                let key = format!("{}_MODEL", app.provider.prefix());
                let _ = app.secrets.set(&key, &id);
                app.bubbles.push(crate::tui::app::Bubble {
                    role: "assistant".to_string(),
                    content: format!("Model changed to {}", id),
                    is_ephemeral: true,
                });
            })),
        });

        Ok(String::new())
    }
}
