//! TUI rendering: chat bubbles, input box, status bar.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::app::App;

/// Main render function called every frame.
pub fn render_ui(f: &mut Frame, app: &App) {
    let size = f.size();

    if app.bubbles.is_empty() {
        render_welcome(f, size, app);
    } else {
        render_chat(f, size, app);
    }
}

/// Render the welcome / empty state screen.
fn render_welcome(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    // Centered logo + prompt
    let logo_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  __  __ _       _ ____  _    _ _   _  ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " |  \\/  (_)_ __ (_)  _ \\| |  | | \\ | |",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " | |\\/| | | '_ \\| | |_) | |/\\| |  \\| |",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " | |  | | | | | | |  __/\\  /\\  / |\\  |",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " |_|  |_|_|_| |_|_|_|    \\/  \\/|_| \\_|",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Autonomous Pentesting Agent",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Ask me anything. Type /help for commands.",
            Style::default().fg(Color::Gray),
        )]),
    ];

    let welcome = Paragraph::new(logo_lines);
    f.render_widget(welcome, chunks[0]);

    render_input_box(f, chunks[1], app);
    render_status_bar(f, chunks[2], app);
}

/// Render the full chat view with bubbles.
fn render_chat(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_bubbles(f, chunks[0], app);
    render_input_box(f, chunks[1], app);
    render_status_bar(f, chunks[2], app);
}

/// Render chat bubbles with role-based colors.
fn render_bubbles(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for bubble in &app.bubbles {
        let (prefix, color) = match bubble.role.as_str() {
            "user" => ("You", Color::Cyan),
            "assistant" => ("MiniPWN", Color::Green),
            "tool" => ("Tool", Color::Yellow),
            _ => ("?", Color::White),
        };

        // Role header
        lines.push(Line::from(vec![Span::styled(
            format!(" {} ", prefix),
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        )]));

        // Content lines — wrap manually
        for content_line in bubble.content.lines() {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", content_line),
                Style::default().fg(Color::White),
            )]));
        }

        lines.push(Line::from(""));
    }

    // Thinking indicator
    if app.is_thinking {
        lines.push(Line::from(vec![Span::styled(
            " MiniPWN ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "  Thinking...",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
        lines.push(Line::from(""));
    }

    let content_height = lines.len() as u16;
    let view_height = area.height;

    // Auto-scroll to bottom unless user scrolled up
    let max_scroll = content_height.saturating_sub(view_height);
    let scroll = max_scroll.saturating_sub(app.scroll_offset);

    let paragraph = Paragraph::new(lines)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the text input box.
fn render_input_box(f: &mut Frame, area: Rect, app: &App) {
    let label = if app.is_thinking {
        " Thinking... "
    } else {
        " Message "
    };
    let input_style = if app.is_thinking {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    // Show cursor as a block character at cursor position
    let before = &app.input[..app.cursor];
    let cursor_char = app.input.chars().nth(app.cursor).unwrap_or(' ');
    let after = if app.cursor < app.input.len() {
        &app.input[app.cursor + cursor_char.len_utf8()..]
    } else {
        ""
    };

    let text = Line::from(vec![
        Span::styled(before.to_string(), input_style),
        Span::styled(
            cursor_char.to_string(),
            if !app.is_thinking {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            },
        ),
        Span::styled(after.to_string(), input_style),
    ]);

    let input = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(label)
            .border_style(if app.is_thinking {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            }),
    );
    f.render_widget(input, area);
}

/// Render the bottom status bar.
fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let worker_label = match &app.execution_mode {
        crate::tools::ExecutionMode::Local { .. } => "local".to_string(),
        crate::tools::ExecutionMode::Remote { .. } => "remote".to_string(),
    };

    let status_text = if app.status.is_empty() {
        format!(
            " chat:{} | provider:{} | worker:{} | Ctrl+C quit | PgUp/PgDn scroll",
            app.chat_id,
            app.provider.display_name(),
            worker_label,
        )
    } else {
        format!(" {}", app.status)
    };

    let status =
        Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray).bg(Color::Reset));
    f.render_widget(status, area);
}
