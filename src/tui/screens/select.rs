//! Execution-environment selection screen shown on startup.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use std::io::Stdout;
use std::time::Duration;

use crate::config::{WorkersList, save_workers_list};
use crate::tui::theme::Theme;
use crate::worker::client::WorkerClient;
use crate::worker::discovery::{self, DiscoveredWorker};

use super::WorkerChoice;
use super::forms::{prompt_new_worker, prompt_worker_name};

#[derive(Clone)]
enum SelectItem {
    Local,
    Saved { idx: usize, active: bool },
    DockerKali,
    SavedHeader,
    Spacer,
    Header(String),
    Discovered(usize),
    Disabled(String),
    AddNew,
}

/// Blocking worker selection screen. Returns the user's choice.
pub async fn worker_select_screen(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    workers: &WorkersList,
    theme: &Theme,
) -> Result<WorkerChoice> {
    let mut workers = workers.clone();
    let (mut saved_status, mut discovered, mut items) = build_screen_state(&workers).await;

    let mut state = ListState::default();
    state.select(Some(0));
    let mut status_message = String::new();

    loop {
        terminal.draw(|f| {
            let size = f.size();

            // Fill background
            f.render_widget(
                Block::default().style(Style::default().bg(theme.background())),
                size,
            );

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(7),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(size);

            // Centered layout for list
            let list_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ])
                .split(chunks[1]);

            // Logo (Monochromatic)
            let logo = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "█▀▄▀█ █ █▄░█ █ █▀█ █░█░█ █▄░█",
                    Style::default().fg(theme.primary()),
                )),
                Line::from(Span::styled(
                    "█░▀░█ █ █░▀█ █ █▀▀ ▀▄▀▄▀ █░▀█",
                    Style::default().fg(theme.primary()),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  ◈  Select Execution Environment  ◈  ",
                    Style::default()
                        .fg(theme.secondary())
                        .add_modifier(Modifier::ITALIC),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(logo, chunks[0]);

            // Worker list
            let list_items: Vec<ListItem> = items
                .iter()
                .map(|item| {
                    let line = match item {
                        SelectItem::Local => Line::from(vec![
                            Span::styled("  ◈  ", Style::default().fg(theme.text_dim())),
                            Span::styled(
                                "No worker (local mode)",
                                Style::default().fg(theme.text()),
                            ),
                        ]),
                        SelectItem::DockerKali => Line::from(vec![
                            Span::styled("  ◈  ", Style::default().fg(theme.text_dim())),
                            Span::styled("Use Kali on Docker", Style::default().fg(theme.text())),
                            Span::styled(
                                " [mounts binary + workspace read-only]",
                                Style::default().fg(theme.text_dim()),
                            ),
                        ]),
                        SelectItem::Saved { idx, active } => {
                            let w = &workers.workers[*idx];
                            let status = if *active { "active" } else { "offline" };
                            let status_color = if *active {
                                theme.success()
                            } else {
                                theme.error()
                            };
                            Line::from(vec![
                                Span::styled("  ◈  ", Style::default().fg(theme.text_dim())),
                                Span::styled(w.name.as_str(), Style::default().fg(theme.text())),
                                Span::styled(
                                    format!(" ({}) ", w.url),
                                    Style::default().fg(theme.text_dim()),
                                ),
                                Span::styled(
                                    format!("[{}]", status),
                                    Style::default().fg(status_color),
                                ),
                            ])
                        }
                        SelectItem::SavedHeader => Line::from(vec![
                            Span::styled("     ", Style::default()),
                            Span::styled(
                                "Saved workers",
                                Style::default()
                                    .fg(theme.primary())
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        SelectItem::Spacer => Line::from(""),
                        SelectItem::Header(label) => Line::from(vec![
                            Span::styled("     ", Style::default()),
                            Span::styled(
                                label.as_str(),
                                Style::default()
                                    .fg(theme.primary())
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        SelectItem::Discovered(idx) => {
                            let w = &discovered[*idx];
                            Line::from(vec![
                                Span::styled("  ◇  ", Style::default().fg(theme.secondary())),
                                Span::styled(w.name.as_str(), Style::default().fg(theme.text())),
                                Span::styled(
                                    format!(" ({}) ", w.url),
                                    Style::default().fg(theme.text_dim()),
                                ),
                                Span::styled(
                                    format!(
                                        "[{} {} | secret {} chars]",
                                        w.os, w.arch, w.secret_len
                                    ),
                                    Style::default().fg(theme.secondary()),
                                ),
                            ])
                        }
                        SelectItem::Disabled(label) => Line::from(vec![
                            Span::styled("     ", Style::default()),
                            Span::styled(label.as_str(), Style::default().fg(theme.text_dim())),
                        ]),
                        SelectItem::AddNew => Line::from(vec![
                            Span::styled("  ◈  ", Style::default().fg(theme.text_dim())),
                            Span::styled("Add new worker...", Style::default().fg(theme.text())),
                        ]),
                    };

                    ListItem::new(vec![line]).style(Style::default().bg(theme.background()))
                })
                .collect();

            let list = List::new(list_items)
                .highlight_style(
                    Style::default()
                        .fg(theme.primary())
                        .bg(theme.surface())
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(" ❯ ");

            f.render_stateful_widget(list, list_layout[1], &mut state);

            // Help
            let status_color = if status_message.is_empty() {
                theme.text_dim()
            } else {
                theme.error()
            };
            let help = Paragraph::new(vec![
                Line::from(Span::styled(
                    status_message.as_str(),
                    Style::default().fg(status_color),
                )),
                Line::from(vec![
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("↑↓", Style::default().fg(theme.text())),
                    Span::styled(" Navigate   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("Enter", Style::default().fg(theme.text())),
                    Span::styled(" Select   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("R", Style::default().fg(theme.text())),
                    Span::styled(" Rename   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("D", Style::default().fg(theme.text())),
                    Span::styled(" Delete   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("Esc", Style::default().fg(theme.text())),
                    Span::styled(" Quit", Style::default().fg(theme.text_dim())),
                ]),
            ])
            .alignment(Alignment::Center);
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up => {
                        move_selection(&mut state, &items, -1);
                    }
                    KeyCode::Down => {
                        move_selection(&mut state, &items, 1);
                    }
                    KeyCode::Enter => {
                        let idx = state.selected().unwrap_or(0);
                        match &items[idx] {
                            SelectItem::Local => return Ok(WorkerChoice::NoWorker),
                            SelectItem::DockerKali => return Ok(WorkerChoice::DockerKali),
                            SelectItem::Saved { idx, .. } => {
                                let worker = &workers.workers[*idx];
                                let client = WorkerClient::new(&worker.url, &worker.secret);
                                match client.validate().await {
                                    Ok(validation) if validation.ok && validation.secret_valid => {
                                        return Ok(WorkerChoice::Saved(*idx));
                                    }
                                    Ok(_) => {
                                        status_message =
                                            format!("Validation failed for {}", worker.url);
                                    }
                                    Err(err) => {
                                        status_message = format!(
                                            "Validation failed for {}: {}",
                                            worker.url, err
                                        );
                                    }
                                }
                            }
                            SelectItem::Discovered(discovered_idx) => {
                                let worker = &discovered[*discovered_idx];
                                return prompt_new_worker(
                                    terminal,
                                    theme,
                                    Some(worker.url.clone()),
                                    Some(worker.name.clone()),
                                )
                                .await;
                            }
                            SelectItem::AddNew => {
                                return prompt_new_worker(terminal, theme, None, None).await;
                            }
                            SelectItem::Header(_)
                            | SelectItem::Disabled(_)
                            | SelectItem::SavedHeader
                            | SelectItem::Spacer => {}
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        let idx = state.selected().unwrap_or(0);
                        if let SelectItem::Saved { idx, .. } = items[idx] {
                            let current = workers.workers[idx].name.clone();
                            match prompt_worker_name(terminal, theme, &current).await? {
                                Some(name) => {
                                    workers.workers[idx].name = name;
                                    save_workers_list(&workers)?;
                                    items = build_items(&workers, &saved_status, &discovered);
                                    status_message = "Worker renamed".to_string();
                                }
                                None => {
                                    status_message.clear();
                                }
                            }
                        } else {
                            status_message = "Select a saved worker to rename".to_string();
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        let idx = state.selected().unwrap_or(0);
                        if let SelectItem::Saved { idx, .. } = items[idx] {
                            let name = workers.workers[idx].name.clone();
                            workers.workers.remove(idx);
                            if idx < saved_status.len() {
                                saved_status.remove(idx);
                            }
                            save_workers_list(&workers)?;
                            let rebuilt = build_screen_state(&workers).await;
                            saved_status = rebuilt.0;
                            discovered = rebuilt.1;
                            items = rebuilt.2;
                            state.select(Some(0));
                            status_message = format!("Worker '{}' deleted", name);
                        } else {
                            status_message = "Select a saved worker to delete".to_string();
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(WorkerChoice::NoWorker);
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn build_screen_state(
    workers: &WorkersList,
) -> (Vec<bool>, Vec<DiscoveredWorker>, Vec<SelectItem>) {
    let saved_status = ping_saved_workers(workers).await;
    let has_active_worker = saved_status.iter().any(|active| *active);
    let discovered = if has_active_worker {
        Vec::new()
    } else {
        discovery::discover(Duration::from_millis(900))
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|d| {
                !workers
                    .workers
                    .iter()
                    .any(|w| normalize_url(&w.url) == normalize_url(&d.url))
            })
            .collect()
    };
    let items = build_items(workers, &saved_status, &discovered);
    (saved_status, discovered, items)
}

async fn ping_saved_workers(workers: &WorkersList) -> Vec<bool> {
    let mut handles = Vec::new();

    for (idx, worker) in workers.workers.iter().cloned().enumerate() {
        handles.push(tokio::spawn(async move {
            let client = WorkerClient::new(&worker.url, &worker.secret);
            let active = client.ping().await.map(|resp| resp.pong).unwrap_or(false);
            (idx, active)
        }));
    }

    let mut status = vec![false; workers.workers.len()];
    for handle in handles {
        if let Ok((idx, active)) = handle.await {
            if let Some(slot) = status.get_mut(idx) {
                *slot = active;
            }
        }
    }
    status
}

fn build_items(
    workers: &WorkersList,
    saved_status: &[bool],
    discovered: &[DiscoveredWorker],
) -> Vec<SelectItem> {
    let mut items = vec![SelectItem::Local];
    items.push(SelectItem::DockerKali);
    items.push(SelectItem::AddNew);
    items.push(SelectItem::SavedHeader);

    for idx in 0..workers.workers.len() {
        items.push(SelectItem::Saved {
            idx,
            active: saved_status.get(idx).copied().unwrap_or(false),
        });
    }

    if saved_status.iter().all(|active| !*active) {
        items.push(SelectItem::Spacer);
        items.push(SelectItem::Header("Auto discovery".to_string()));
        if discovered.is_empty() {
            items.push(SelectItem::Disabled(
                "No LAN workers discovered".to_string(),
            ));
        } else {
            for idx in 0..discovered.len() {
                items.push(SelectItem::Discovered(idx));
            }
        }
    }

    items
}

fn move_selection(state: &mut ListState, items: &[SelectItem], direction: isize) {
    if items.is_empty() {
        return;
    }

    let mut idx = state.selected().unwrap_or(0);
    for _ in 0..items.len() {
        idx = if direction < 0 {
            if idx == 0 { items.len() - 1 } else { idx - 1 }
        } else {
            (idx + 1) % items.len()
        };

        if is_selectable(&items[idx]) {
            state.select(Some(idx));
            return;
        }
    }
}

fn is_selectable(item: &SelectItem) -> bool {
    matches!(
        item,
        SelectItem::Local
            | SelectItem::DockerKali
            | SelectItem::AddNew
            | SelectItem::Saved { .. }
            | SelectItem::Discovered(_)
    )
}

fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}
