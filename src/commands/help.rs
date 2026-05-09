use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;
use super::{Command, CommandRegistry};

pub struct HelpCommand {}

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str { "help" }
    fn description(&self) -> &str { "Show available commands" }
    fn usage(&self) -> &str { "/help" }

    async fn execute(&self, _app: &mut App, _name: &str, _args: &[&str]) -> Result<String> {
        let registry = CommandRegistry::new();
        let mut help_text = String::from("Available commands:\n\n");
        
        for cmd in &registry.commands {
            help_text.push_str(&format!("{} {} | {}\n", cmd.usage(), "", cmd.description()));
        }

        Ok(help_text)
    }
}
