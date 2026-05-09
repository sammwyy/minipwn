use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;
use crate::config::Secrets;
use super::Command;

pub struct ApiKeyCommand {}

#[async_trait]
impl Command for ApiKeyCommand {
    fn name(&self) -> &str { "apikey" }
    fn description(&self) -> &str { "Set API key for current provider" }
    fn usage(&self) -> &str { "/apikey <key>" }

    async fn execute(&self, app: &mut App, _name: &str, args: &[&str]) -> Result<String> {
        if args.is_empty() || args[0].is_empty() {
            return Ok("Usage: /apikey <key>".to_string());
        }

        let key = args[0];
        let mut secrets = Secrets::load().unwrap_or_default();
        secrets.set_key(&app.provider, key);
        secrets.save()?;
        app.secrets = secrets;

        Ok(format!("API key updated for {}", app.provider.display_name()))
    }
}
