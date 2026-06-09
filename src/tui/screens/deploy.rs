//! Live deployment log shown while provisioning a Kali Docker worker.

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use std::io::Stdout;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::tui::theme::Theme;
use crate::worker::DeployedContainer;

/// Drive [`crate::worker::deploy_kali_worker`] while streaming its log to screen.
pub async fn docker_deploy_screen(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    theme: &Theme,
    workspace: &std::path::Path,
) -> Result<DeployedContainer> {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let workspace_display = workspace.to_path_buf();
    let workspace = workspace.to_path_buf();

    let mut deploy_task =
        tokio::spawn(async move { crate::worker::deploy_kali_worker(&workspace, Some(tx)).await });

    let mut logs: Vec<String> = vec!["Preparing Kali Docker worker...".to_string()];
    let mut error: Option<String> = None;

    loop {
        while let Ok(line) = rx.try_recv() {
            logs.push(line);
        }

        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(
                Block::default().style(Style::default().bg(theme.background())),
                size,
            );

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(size);

            let title = Paragraph::new(vec![
                Line::from(Span::styled(
                    " KALI DOCKER DEPLOY ",
                    Style::default()
                        .fg(theme.primary())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    workspace_display.display().to_string(),
                    Style::default().fg(theme.text_dim()),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(title, chunks[0]);

            let display_lines: Vec<Line> = {
                let start = logs.len().saturating_sub(14);
                logs[start..]
                    .iter()
                    .map(|line| {
                        Line::from(Span::styled(
                            line.as_str(),
                            Style::default().fg(theme.text()),
                        ))
                    })
                    .collect()
            };
            let log_block = Paragraph::new(display_lines)
                .block(
                    Block::default()
                        .title("Deploy log")
                        .borders(ratatui::widgets::Borders::ALL),
                )
                .wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(log_block, chunks[1]);

            let footer_text = error
                .as_deref()
                .unwrap_or("Press Esc to abort, wait for the deploy to complete");
            let footer_color = if error.is_some() {
                theme.error()
            } else {
                theme.text_dim()
            };
            let footer = Paragraph::new(vec![Line::from(Span::styled(
                footer_text,
                Style::default().fg(footer_color),
            ))])
            .alignment(Alignment::Center);
            f.render_widget(footer, chunks[2]);
        })?;

        if deploy_task.is_finished() {
            match rx.try_recv() {
                Ok(line) => {
                    logs.push(line);
                    continue;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        tokio::select! {
            maybe = rx.recv() => {
                if let Some(line) = maybe {
                    logs.push(line);
                }
            }
            res = &mut deploy_task => {
                match res {
                    Ok(Ok(worker)) => return Ok(worker),
                    Ok(Err(err)) => {
                        error = Some(err.to_string());
                    }
                    Err(err) => {
                        error = Some(err.to_string());
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(120)) => {}
        }
    }

    match deploy_task.await {
        Ok(Ok(worker)) => Ok(worker),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err.into()),
    }
}
