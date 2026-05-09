//! Worker selection screen shown on startup.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use std::io::Stdout;

use crate::config::WorkersList;
use crate::tui::theme::Theme;

/// Result of the worker selection screen.
pub enum WorkerChoice {
    NoWorker,
    Saved(usize),
    New {
        url: String,
        secret: String,
        name: String,
    },
}

/// Blocking worker selection screen. Returns the user's choice.
pub async fn worker_select_screen(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    workers: &WorkersList,
    theme: &Theme,
) -> Result<WorkerChoice> {
    // Build menu items
    let mut items: Vec<String> = vec!["No worker (local mode)".to_string()];
    for w in &workers.workers {
        items.push(format!("{} ({})", w.name, w.url));
    }
    items.push("Add new worker...".to_string());

    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let size = f.size();
            
            // Fill background
            f.render_widget(Block::default().style(Style::default().bg(theme.background())), size);

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
                Line::from(Span::styled("█▀▄▀█ █ █▄░█ █ █▀█ █░█░█ █▄░█", Style::default().fg(theme.primary()))),
                Line::from(Span::styled("█░▀░█ █ █░▀█ █ █▀▀ ▀▄▀▄▀ █░▀█", Style::default().fg(theme.primary()))),
                Line::from(""),
                Line::from(Span::styled("  ◈  Select Execution Environment  ◈  ", Style::default().fg(theme.secondary()).add_modifier(Modifier::ITALIC))),
            ])
            .alignment(Alignment::Center);
            f.render_widget(logo, chunks[0]);

            // Worker list
            let list_items: Vec<ListItem> = items.iter().map(|i| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled("  ◈  ", Style::default().fg(theme.text_dim())),
                        Span::styled(i.as_str(), Style::default().fg(theme.text())),
                    ])
                ]).style(Style::default().bg(theme.background()))
            }).collect();

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
            let help = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("↑↓", Style::default().fg(theme.text())),
                    Span::styled(" Navigate   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("Enter", Style::default().fg(theme.text())),
                    Span::styled(" Select   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" ▣ ", Style::default().fg(theme.primary())),
                    Span::styled("Esc", Style::default().fg(theme.text())),
                    Span::styled(" Quit", Style::default().fg(theme.text_dim())),
                ])
            ])
            .alignment(Alignment::Center);
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some(if i == 0 { items.len() - 1 } else { i - 1 }));
                    }
                    KeyCode::Down => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some((i + 1) % items.len()));
                    }
                    KeyCode::Enter => {
                        let idx = state.selected().unwrap_or(0);
                        if idx == 0 {
                            return Ok(WorkerChoice::NoWorker);
                        } else if idx == items.len() - 1 {
                            return prompt_new_worker(terminal, theme).await;
                        } else {
                            return Ok(WorkerChoice::Saved(idx - 1));
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

async fn prompt_new_worker(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
) -> Result<WorkerChoice> {
    enum Field {
        Url,
        Secret,
        Name,
    }
    let mut url = String::new();
    let mut secret = String::new();
    let mut name = String::new();
    let mut field = Field::Url;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(Block::default().style(Style::default().bg(theme.background())), size);

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(size);

            let content_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ])
                .split(main_chunks[1]);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(2),
                    Constraint::Length(2),
                    Constraint::Min(0),
                ])
                .split(content_layout[1]);

            let header = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(" ◇ NEW WORKER ◇ ", Style::default().fg(theme.primary()).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("──────────────────", Style::default().fg(theme.surface()))),
            ]).alignment(Alignment::Center);
            f.render_widget(header, main_chunks[0]);

            let (url_p, sec_p, name_p) = match field {
                Field::Url => (" ❯ ", "   ", "   "),
                Field::Secret => ("   ", " ❯ ", "   "),
                Field::Name => ("   ", "   ", " ❯ "),
            };

            let url_line = Line::from(vec![
                Span::styled(url_p, Style::default().fg(theme.primary())),
                Span::styled("URL: ", Style::default().fg(theme.text_dim())),
                Span::styled(url.clone(), Style::default().fg(theme.text())),
            ]);
            f.render_widget(Paragraph::new(url_line), chunks[0]);

            let sec_line = Line::from(vec![
                Span::styled(sec_p, Style::default().fg(theme.primary())),
                Span::styled("SECRET: ", Style::default().fg(theme.text_dim())),
                Span::styled("*".repeat(secret.len()), Style::default().fg(theme.text())),
            ]);
            f.render_widget(Paragraph::new(sec_line), chunks[1]);

            let name_line = Line::from(vec![
                Span::styled(name_p, Style::default().fg(theme.primary())),
                Span::styled("NAME: ", Style::default().fg(theme.text_dim())),
                Span::styled(name.clone(), Style::default().fg(theme.text())),
            ]);
            f.render_widget(Paragraph::new(name_line), chunks[2]);

            let help = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" [Enter] ", Style::default().fg(theme.primary())),
                    Span::styled("Next/Confirm   ", Style::default().fg(theme.text_dim())),
                    Span::styled(" [Esc] ", Style::default().fg(theme.primary())),
                    Span::styled("Cancel", Style::default().fg(theme.text_dim())),
                ])
            ]).alignment(Alignment::Center);
            f.render_widget(help, main_chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => return Ok(WorkerChoice::NoWorker),
                    KeyCode::Enter => match field {
                        Field::Url => field = Field::Secret,
                        Field::Secret => field = Field::Name,
                        Field::Name => {
                            let url_val = if url.is_empty() {
                                "http://localhost:10000".to_string()
                            } else {
                                url.clone()
                            };
                            let name_val = if name.is_empty() {
                                "worker".to_string()
                            } else {
                                name.clone()
                            };
                            return Ok(WorkerChoice::New {
                                url: url_val,
                                secret: secret.clone(),
                                name: name_val,
                            });
                        }
                    },
                    KeyCode::Char(c) => match field {
                        Field::Url => url.push(c),
                        Field::Secret => secret.push(c),
                        Field::Name => name.push(c),
                    },
                    KeyCode::Backspace => match field {
                        Field::Url => { url.pop(); }
                        Field::Secret => { secret.pop(); }
                        Field::Name => { name.pop(); }
                    },
                    _ => {}
                }
            }
        }
    }
}
