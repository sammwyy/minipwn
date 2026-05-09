use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::tui::app::{App, ModalState, ModalItem, ModalCallback};
use crate::config::{Provider, save_global_config};
use super::Command;

pub struct ProviderCommand {}

#[async_trait]
impl Command for ProviderCommand {
    fn name(&self) -> &str { "provider" }
    fn aliases(&self) -> Vec<&str> { vec!["providers"] }
    fn description(&self) -> &str { "Change AI provider" }
    fn usage(&self) -> &str { "/provider [id]" }

    async fn execute(&self, app: &mut App, args: &[&str]) -> Result<String> {
        if !args.is_empty() && !args[0].is_empty() {
            let provider_id = args[0];
            if let Some(p) = Provider::from_str(provider_id) {
                app.provider = p;
                app.global_config.provider = provider_id.to_string();
                save_global_config(&app.global_config)?;
                return Ok(format!("Provider changed to {}", provider_id));
            } else {
                return Ok(format!("Provider '{}' not found", provider_id));
            }
        }

        // Open modal
        let providers = vec!["openai", "openrouter", "custom"];
        let items = providers.into_iter()
            .map(|p| ModalItem { id: p.to_string(), label: p.to_string() })
            .collect();

        app.modal = Some(ModalState {
            title: "Select Provider".to_string(),
            items,
            selected: 0,
            filter: String::new(),
            callback: ModalCallback(Arc::new(|app, id| {
                if let Some(p) = Provider::from_str(&id) {
                    app.provider = p;
                    app.global_config.provider = id.clone();
                    let _ = save_global_config(&app.global_config);
                    app.bubbles.push(crate::tui::app::Bubble {
                        role: "assistant".to_string(),
                        content: format!("Provider changed to {}", id),
                        is_ephemeral: true,
                    });
                }
            })),
        });

        Ok(String::new())
    }
}
