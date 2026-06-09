use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn aliases(&self) -> Vec<&str> { vec![] }
    fn description(&self) -> &str;
    fn usage(&self) -> &str;
    async fn execute(&self, app: &mut App, name: &str, args: &[&str]) -> Result<String>;
}

pub struct CommandRegistry {
    pub commands: Vec<Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: vec![
                Box::new(help::HelpCommand {}),
                Box::new(model::ModelCommand {}),
                Box::new(provider::ProviderCommand {}),
                Box::new(chat::ChatCommand {}),
                Box::new(theme::ThemeCommand {}),
                Box::new(clear::ClearCommand {}),
                Box::new(apikey::ApiKeyCommand {}),
                Box::new(worker::WorkerCommand {}),
                Box::new(mode::ModeCommand {}),
                Box::new(usage::UsageCommand {}),
            ],
        }
    }

    pub fn find(&self, name: &str) -> Option<&dyn Command> {
        self.commands.iter().find(|c| {
            c.name() == name || c.aliases().contains(&name)
        }).map(|c| c.as_ref())
    }
}

pub mod help;
pub mod model;
pub mod provider;
pub mod chat;
pub mod theme;
pub mod clear;
pub mod apikey;
pub mod worker;
pub mod mode;
pub mod usage;
