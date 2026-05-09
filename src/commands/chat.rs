use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::tui::app::{App, ModalState, ModalItem, ModalCallback};
use crate::config::{list_chats, save_workspace_meta, load_chat};
use super::Command;

pub struct ChatCommand {}

#[async_trait]
impl Command for ChatCommand {
    fn name(&self) -> &str { "chat" }
    fn aliases(&self) -> Vec<&str> { vec!["chats"] }
    fn description(&self) -> &str { "Switch chat session" }
    fn usage(&self) -> &str { "/chat [id]" }

    async fn execute(&self, app: &mut App, args: &[&str]) -> Result<String> {
        if !args.is_empty() && !args[0].is_empty() {
            let chat_id = args[0].to_string();
            app.chat_id = chat_id.clone();
            app.meta.current_chat = chat_id.clone();
            save_workspace_meta(&app.meta)?;
            
            // Reload chat
            let session = load_chat(&chat_id)?;
            app.bubbles = session.messages.iter().rev().take(10).rev().map(|m| crate::tui::app::Bubble {
                role: m.role.clone(),
                content: m.content.clone(),
                is_ephemeral: false,
            }).collect();

            return Ok(format!("Switched to chat {}", chat_id));
        }

        // Open modal
        let mut items = vec![ModalItem { id: "__new__".to_string(), label: "<Create new Chat>".to_string() }];
        let chats = list_chats()?;
        for id in chats {
            items.push(ModalItem { id: id.clone(), label: format!("Chat {}", id) });
        }

        app.modal = Some(ModalState {
            title: "Select Chat".to_string(),
            items,
            selected: 0,
            filter: String::new(),
            callback: ModalCallback(Arc::new(|app, id| {
                let chat_id = if id == "__new__" {
                    uuid::Uuid::new_v4().to_string()[..8].to_string()
                } else {
                    id
                };

                app.chat_id = chat_id.clone();
                app.meta.current_chat = chat_id.clone();
                let _ = save_workspace_meta(&app.meta);
                
                let session = load_chat(&chat_id).unwrap_or_else(|_| crate::config::ChatSession {
                    id: chat_id.clone(),
                    messages: vec![],
                });

                app.bubbles = session.messages.iter().rev().take(10).rev().map(|m| crate::tui::app::Bubble {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    is_ephemeral: false,
                }).collect();

                app.bubbles.push(crate::tui::app::Bubble {
                    role: "assistant".to_string(),
                    content: format!("Switched to chat {}", chat_id),
                    is_ephemeral: true,
                });
            })),
        });

        Ok(String::new())
    }
}
