use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;
use super::Command;

pub struct ClearCommand {}

#[async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str { "clear" }
    fn description(&self) -> &str { "Clear chat bubbles" }
    fn usage(&self) -> &str { "/clear" }

    async fn execute(&self, app: &mut App, _args: &[&str]) -> Result<String> {
        app.bubbles.clear();
        Ok("Chat cleared".to_string())
    }
}
