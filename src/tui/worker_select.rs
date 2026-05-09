//! Worker selection screen shown on startup.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, BorderType},
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

            // Logo
            let logo = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "  __  __ _       _ ____  _    _ _   _  ",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    " |  \\/  (_)_ __ (_)  _ \\| |  | | \\ | |",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    " | |\\/| | | '_ \\| | |_) | |/\\| |  \\| |",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    " | |  | | | | | | |  __/\\  /\\  / |\\  |",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    " |_|  |_|_|_| |_|_|_|    \\/  \\/|_| \\_|",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )]),
            ])
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default());
            f.render_widget(logo, chunks[0]);

            // Worker list
            let list_items: Vec<ListItem> =
                items.iter().map(|i| ListItem::new(i.as_str()).style(Style::default().fg(theme.text()))).collect();

            let list = List::new(list_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(theme.primary()))
                        .title(Span::styled(" Select Worker ", Style::default().fg(theme.primary()).add_modifier(Modifier::BOLD))),
                )
                .highlight_style(
                    Style::default()
                        .fg(theme.background())
                        .bg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(list, chunks[1], &mut state);

            // Help
            let help = Paragraph::new("  [↑↓] Navigate   [Enter] Select   [Ctrl+C] Quit")
                .style(Style::default().fg(theme.text_dim())).alignment(ratatui::layout::Alignment::Center);
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

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(size);

            let (url_style, sec_style, name_style) = match field {
                Field::Url => (
                    Style::default().fg(theme.secondary()),
                    Style::default().fg(theme.text_dim()),
                    Style::default().fg(theme.text_dim()),
                ),
                Field::Secret => (
                    Style::default().fg(theme.text_dim()),
                    Style::default().fg(theme.secondary()),
                    Style::default().fg(theme.text_dim()),
                ),
                Field::Name => (
                    Style::default().fg(theme.text_dim()),
                    Style::default().fg(theme.text_dim()),
                    Style::default().fg(theme.secondary()),
                ),
            };

            let url_input = Paragraph::new(format!("URL: {}", url))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Worker URL "))
                .style(url_style);
            f.render_widget(url_input, chunks[0]);

            let sec_input = Paragraph::new(format!("Secret: {}", secret))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Secret "))
                .style(sec_style);
            f.render_widget(sec_input, chunks[1]);

            let name_input = Paragraph::new(format!("Name: {}", name))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Name "))
                .style(name_style);
            f.render_widget(name_input, chunks[2]);

            let help = Paragraph::new("[Enter] Next field / Confirm   [Esc] Cancel")
                .style(Style::default().fg(theme.text_dim()));
            f.render_widget(help, chunks[3]);
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
