use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::tui::app::{App, ModalState, ModalItem, ModalCallback};
use crate::config::{all_providers, provider_from_id, save_global_config};
use super::Command;

pub struct ProviderCommand {}

#[async_trait]
impl Command for ProviderCommand {
    fn name(&self) -> &str { "provider" }
    fn aliases(&self) -> Vec<&str> { vec!["providers"] }
    fn description(&self) -> &str { "Change AI provider" }
    fn usage(&self) -> &str { "/provider [id]" }

    async fn execute(&self, app: &mut App, _name: &str, args: &[&str]) -> Result<String> {
        if !args.is_empty() && !args[0].is_empty() {
            let provider_id = args[0];
            if let Some(p) = provider_from_id(provider_id) {
                app.global_config.provider = p.id().to_string();
                app.provider = p;
                save_global_config(&app.global_config)?;
                return Ok(format!("Provider changed to {}", provider_id));
            } else {
                return Ok(format!("Provider '{}' not found", provider_id));
            }
        }

        // Open modal listing every known provider.
        let items = all_providers()
            .into_iter()
            .map(|p| ModalItem {
                id: p.id().to_string(),
                label: p.display_name().to_string(),
            })
            .collect();

        app.modal = Some(ModalState {
            title: "Select Provider".to_string(),
            items,
            selected: 0,
            filter: String::new(),
            callback: ModalCallback(Arc::new(|app, id| {
                if let Some(p) = provider_from_id(&id) {
                    app.global_config.provider = p.id().to_string();
                    app.provider = p;
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
