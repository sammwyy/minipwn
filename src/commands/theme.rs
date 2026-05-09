use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;
use crate::config::save_global_config;
use super::Command;

pub struct ThemeCommand {}

#[async_trait]
impl Command for ThemeCommand {
    fn name(&self) -> &str { "theme" }
    fn description(&self) -> &str { "Change UI theme" }
    fn usage(&self) -> &str { "/theme <id>" }

    async fn execute(&self, app: &mut App, _name: &str, args: &[&str]) -> Result<String> {
        if args.is_empty() || args[0].is_empty() {
            let themes = app.theme_registry.list();
            let mut list = String::from("Available themes:\n");
            for (id, t) in themes {
                list.push_str(&format!("  - {} (by {})\n", id, t.author));
            }
            return Ok(list);
        }

        let theme_id = args[0];
        if let Some(theme) = app.theme_registry.get(theme_id) {
            app.theme = theme.clone();
            app.global_config.theme = theme_id.to_string();
            let _ = save_global_config(&app.global_config);
            Ok(format!("Theme changed to {}", theme_id))
        } else {
            Ok(format!("Theme '{}' not found", theme_id))
        }
    }
}
