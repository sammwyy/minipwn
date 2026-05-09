use anyhow::Result;
use async_trait::async_trait;
use crate::tui::app::App;
use crate::config::{load_workers_list, SavedWorker, save_workers_list};
use super::Command;

pub struct WorkerCommand {}

#[async_trait]
impl Command for WorkerCommand {
    fn name(&self) -> &str { "worker" }
    fn description(&self) -> &str { "Manage remote workers" }
    fn usage(&self) -> &str { "/worker | list | add <url> <secret> [name]" }

    async fn execute(&self, app: &mut App, args: &[&str]) -> Result<String> {
        if args.is_empty() || args[0].is_empty() || args[0] == "list" {
            let list = load_workers_list().unwrap_or_default();
            let current = match &app.execution_mode {
                crate::tools::ExecutionMode::Local { .. } => "local".to_string(),
                crate::tools::ExecutionMode::Remote { client, .. } => client.base_url.clone(),
            };

            let mut lines = vec![format!("Current worker: {}", current)];
            if list.workers.is_empty() {
                lines.push("No saved workers. Use /worker add <url> <secret> [name]".to_string());
            } else {
                lines.push("Saved workers:".to_string());
                for (i, w) in list.workers.iter().enumerate() {
                    lines.push(format!("  {}: {} ({})", i, w.name, w.url));
                }
            }
            return Ok(lines.join("\n"));
        }

        if args[0] == "add" && args.len() >= 3 {
            let url = args[1].to_string();
            let secret = args[2].to_string();
            let name = args.get(3).unwrap_or(&"worker").to_string();

            let mut list = load_workers_list().unwrap_or_default();
            list.workers.push(SavedWorker { name: name.clone(), url, secret });
            save_workers_list(&list)?;

            return Ok(format!("Worker '{}' added", name));
        }

        Ok("Invalid worker command. Use /worker list or /worker add".to_string())
    }
}
