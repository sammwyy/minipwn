use super::Command;
use crate::config::{SavedWorker, load_workers_list, save_workers_list};
use crate::tui::app::App;
use crate::worker::client::WorkerClient;
use anyhow::Result;
use async_trait::async_trait;

pub struct WorkerCommand {}

#[async_trait]
impl Command for WorkerCommand {
    fn name(&self) -> &str {
        "worker"
    }
    fn description(&self) -> &str {
        "Manage remote workers"
    }
    fn usage(&self) -> &str {
        "/worker | list | add <url> <secret> [name] | rename <index|name> <new-name> | delete <index|name>"
    }

    async fn execute(&self, app: &mut App, _name: &str, args: &[&str]) -> Result<String> {
        if args.is_empty() || args[0].is_empty() || args[0] == "list" {
            let list = load_workers_list().unwrap_or_default();
            let mut lines = vec![format!("Current worker: {}", app.worker.display_name())];
            if list.workers.is_empty() {
                lines.push("No saved workers. Use /worker add <url> <secret> [name]".to_string());
            } else {
                lines.push("Saved workers:".to_string());
                for (i, w) in list.workers.iter().enumerate() {
                    let status = WorkerClient::new(&w.url, &w.secret)
                        .ping()
                        .await
                        .map(|resp| {
                            if resp.pong {
                                format!("active {}:{}", resp.worker, resp.port)
                            } else {
                                "offline".to_string()
                            }
                        })
                        .unwrap_or_else(|_| "offline".to_string());
                    lines.push(format!("  {}: {} ({}) [{}]", i, w.name, w.url, status));
                }
            }
            return Ok(lines.join("\n"));
        }

        if args[0] == "add" && args.len() >= 3 {
            let url = args[1].to_string();
            let secret = args[2].to_string();
            let name = args.get(3).unwrap_or(&"worker").to_string();

            let validation = WorkerClient::new(&url, &secret).validate().await?;
            if !validation.ok || !validation.secret_valid {
                return Ok(format!("Worker '{}' validation failed", name));
            }

            let mut list = load_workers_list().unwrap_or_default();
            if let Some(existing) = list.workers.iter_mut().find(|w| w.url == url) {
                existing.name = name.clone();
                existing.secret = secret;
            } else {
                list.workers.push(SavedWorker {
                    name: name.clone(),
                    url,
                    secret,
                });
            }
            save_workers_list(&list)?;

            return Ok(format!(
                "Worker '{}' added and validated ({} {}, secret {} chars)",
                name, validation.info.os, validation.info.arch, validation.secret_len
            ));
        }

        if args[0] == "rename" && args.len() >= 3 {
            let target = args[1];
            let new_name = args[2].to_string();
            let mut list = load_workers_list().unwrap_or_default();
            let Some(idx) = find_worker_index(&list, target) else {
                return Ok(format!("Worker not found: {}", target));
            };

            list.workers[idx].name = new_name.clone();
            save_workers_list(&list)?;
            return Ok(format!("Worker renamed to '{}'", new_name));
        }

        if (args[0] == "delete" || args[0] == "rm") && args.len() >= 2 {
            let target = args[1];
            let mut list = load_workers_list().unwrap_or_default();
            let Some(idx) = find_worker_index(&list, target) else {
                return Ok(format!("Worker not found: {}", target));
            };

            let removed = list.workers.remove(idx);
            save_workers_list(&list)?;
            return Ok(format!("Worker '{}' deleted", removed.name));
        }

        Ok("Invalid worker command. Use /worker list or /worker add".to_string())
    }
}

fn find_worker_index(list: &crate::config::WorkersList, target: &str) -> Option<usize> {
    if let Ok(idx) = target.parse::<usize>() {
        if idx < list.workers.len() {
            return Some(idx);
        }
    }

    list.workers.iter().position(|w| w.name == target)
}
