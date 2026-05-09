use anyhow::Result;
use async_trait::async_trait;
use crate::commands::Command;
use crate::tui::app::{App, ModalItem, ModalState, ModalCallback};
use std::sync::Arc;

pub struct ModeCommand {}

#[async_trait]
impl Command for ModeCommand {
    fn name(&self) -> &str { "mode" }
    fn aliases(&self) -> Vec<&str> { vec!["safe", "weaponized"] }
    fn description(&self) -> &str { "Change operation mode (Safe vs Weaponized)" }
    fn usage(&self) -> &str { "/mode | /safe | /weaponized" }

    async fn execute(&self, app: &mut App, name: &str, _args: &[&str]) -> Result<String> {
        // Direct set via alias
        if name == "safe" || name == "weaponized" {
            app.meta.mode = name.to_string();
            let _ = crate::config::save_workspace_meta(&app.meta);
            return Ok(format!("Mode set to: {}", name));
        }

        let items = vec![
            ModalItem {
                id: "safe".to_string(),
                label: "Safe: Passive reconnaissance and non-intrusive scanning only. No exploit attempts.".to_string(),
            },
            ModalItem {
                id: "weaponized".to_string(),
                label: "Weaponized: Active exploitation, brute forcing, and intrusive security testing.".to_string(),
            },
        ];

        let mut selected = 0;
        if let Some(pos) = items.iter().position(|i| i.id == app.meta.mode) {
            selected = pos;
        }

        app.modal = Some(ModalState {
            title: "Select Operation Mode".to_string(),
            items,
            selected,
            filter: String::new(),
            callback: ModalCallback(Arc::new(|app_ref: &mut App, selected_id: String| {
                app_ref.meta.mode = selected_id.clone();
                let _ = crate::config::save_workspace_meta(&app_ref.meta);
                app_ref.bubbles.push(crate::tui::app::Bubble {
                    role: "assistant".to_string(),
                    content: format!("Mode set to: {}", selected_id),
                    is_ephemeral: true,
                });
            })),
        });

        Ok(String::new())
    }
}
