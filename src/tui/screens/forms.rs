//! Text-entry dialogs for adding and renaming workers.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use std::io::Stdout;

use crate::tui::theme::Theme;

use super::WorkerChoice;

/// Prompt for a worker URL, secret, and name. Returns the resulting choice.
pub async fn prompt_new_worker(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
    default_url: Option<String>,
    default_name: Option<String>,
) -> Result<WorkerChoice> {
    enum Field {
        Url,
        Secret,
        Name,
    }
    let mut url = default_url.unwrap_or_default();
    let mut secret = String::new();
    let mut name = default_name.unwrap_or_default();
    let mut field = if url.is_empty() {
        Field::Url
    } else {
        Field::Secret
    };

    loop {
        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(
                Block::default().style(Style::default().bg(theme.background())),
                size,
            );

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
                Line::from(Span::styled(
                    " ◇ NEW WORKER ◇ ",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "──────────────────",
                    Style::default().fg(theme.surface()),
                )),
            ])
            .alignment(Alignment::Center);
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

            let help = Paragraph::new(vec![Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(theme.primary())),
                Span::styled("Next/Confirm   ", Style::default().fg(theme.text_dim())),
                Span::styled(" [Esc] ", Style::default().fg(theme.primary())),
                Span::styled("Cancel", Style::default().fg(theme.text_dim())),
            ])])
            .alignment(Alignment::Center);
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
                        Field::Url => {
                            url.pop();
                        }
                        Field::Secret => {
                            secret.pop();
                        }
                        Field::Name => {
                            name.pop();
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

/// Prompt to rename a worker, seeded with its current name.
pub async fn prompt_worker_name(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
    current_name: &str,
) -> Result<Option<String>> {
    let mut name = current_name.to_string();

    loop {
        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(
                Block::default().style(Style::default().bg(theme.background())),
                size,
            );

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

            let header = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    " ◇ RENAME WORKER ◇ ",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "────────────────────",
                    Style::default().fg(theme.surface()),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(header, main_chunks[0]);

            let input = Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(theme.primary())),
                Span::styled("NAME: ", Style::default().fg(theme.text_dim())),
                Span::styled(name.clone(), Style::default().fg(theme.text())),
            ]);
            f.render_widget(Paragraph::new(input), content_layout[1]);

            let help = Paragraph::new(vec![Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(theme.primary())),
                Span::styled("Confirm   ", Style::default().fg(theme.text_dim())),
                Span::styled(" [Esc] ", Style::default().fg(theme.primary())),
                Span::styled("Cancel", Style::default().fg(theme.text_dim())),
            ])])
            .alignment(Alignment::Center);
            f.render_widget(help, main_chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Enter => {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            return Ok(Some(trimmed.to_string()));
                        }
                    }
                    KeyCode::Char(c) => name.push(c),
                    KeyCode::Backspace => {
                        name.pop();
                    }
                    _ => {}
                }
            }
        }
    }
}
