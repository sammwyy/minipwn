//! TUI rendering: chat bubbles, input box, status bar.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, BorderType},
};

use super::app::App;

/// Main render function called every frame.
pub fn render_ui(f: &mut Frame, app: &App) {
    let area = f.size();
    
    // Fill background
    let bg_block = Block::default().style(Style::default().bg(app.theme.background()));
    f.render_widget(bg_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Chat area
            Constraint::Length(if !app.suggestions.is_empty() { 4 } else { 3 }), // Input box + suggestions
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_title_bar(f, chunks[0], app);
    
    if app.bubbles.is_empty() {
        render_welcome(f, chunks[1], app);
    } else {
        render_bubbles(f, chunks[1], app);
    }

    render_input_box(f, chunks[2], app);
    render_status_bar(f, chunks[3], app);
}

fn render_title_bar(f: &mut Frame, area: Rect, app: &App) {
    let title = format!(" MINIPWN v0.1.0 — {} ", app.chat_id);
    let p = Paragraph::new(Line::from(vec![
        Span::styled(title, Style::default().fg(app.theme.background()).bg(app.theme.primary()).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" [Mode: {}] ", match &app.execution_mode {
            crate::tools::ExecutionMode::Local { .. } => "Local",
            crate::tools::ExecutionMode::Remote { .. } => "Remote",
        }), Style::default().fg(app.theme.primary()).bg(app.theme.surface())),
        Span::styled(format!(" [Tokens: {}] ", app.stats.total_tokens), Style::default().fg(app.theme.secondary()).bg(app.theme.surface())),
    ])).style(Style::default().bg(app.theme.surface()));
    f.render_widget(p, area);
}

/// Render the welcome / empty state screen.
fn render_welcome(f: &mut Frame, area: Rect, app: &App) {
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Percentage(30),
        ])
        .split(area);

    let logo_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  __  __ _       _ ____  _    _ _   _  ",
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " |  \\/  (_)_ __ (_)  _ \\| |  | | \\ | |",
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " | |\\/| | | '_ \\| | |_) | |/\\| |  \\| |",
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " | |  | | | | | | |  __/\\  /\\  / |\\  |",
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " |_|  |_|_|_| |_|_|_|    \\/  \\/|_| \\_|",
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Autonomous Pentesting Agent",
            Style::default().fg(app.theme.text_dim()),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Ask me anything. Type /help for commands.",
            Style::default().fg(app.theme.text()),
        )]),
    ];

    let welcome = Paragraph::new(logo_lines).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(welcome, center_chunks[1]);
}

/// Render chat bubbles with role-based colors.
fn render_bubbles(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for bubble in &app.bubbles {
        if bubble.role == "tool" {
            // Render tool result beautifully
            let success = bubble.content.contains("OK");
            let color = if success { app.theme.success() } else { app.theme.error() };

            lines.push(Line::from(vec![Span::styled(
                " 🛠 TOOL RESULT ",
                Style::default().fg(app.theme.background()).bg(color).add_modifier(Modifier::BOLD),
            )]));
            
            let block_style = Style::default().fg(color);
            lines.push(Line::from(vec![Span::styled(" ╭──────────────────────────────────", block_style)]));
            for content_line in bubble.content.lines() {
                 lines.push(Line::from(vec![
                    Span::styled(" │ ", block_style),
                    Span::styled(content_line, Style::default().fg(app.theme.text())),
                 ]));
            }
            lines.push(Line::from(vec![Span::styled(" ╰──────────────────────────────────", block_style)]));
            lines.push(Line::from(""));
            continue;
        }

        let (prefix, color) = match bubble.role.as_str() {
            "user" => (" YOU ", app.theme.user_bubble()),
            "assistant" => (" MINIPWN ", app.theme.assistant_bubble()),
            _ => (" ? ", app.theme.text()),
        };

        let style_bg = if bubble.is_ephemeral {
            app.theme.surface()
        } else {
            app.theme.background()
        };

        // Role header
        lines.push(Line::from(vec![Span::styled(
            prefix,
            Style::default()
                .fg(app.theme.background())
                .bg(color)
                .add_modifier(Modifier::BOLD),
        )]));

        // Content lines
        for content_line in bubble.content.lines() {
            // Highlight tool calls within assistant messages
            if bubble.role == "assistant" && content_line.contains("<tool_call>") {
                 lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(content_line, Style::default().fg(app.theme.secondary()).bg(style_bg).add_modifier(Modifier::BOLD)),
                 ]));
            } else {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", content_line),
                    Style::default().fg(app.theme.text()).bg(style_bg),
                )]));
            }
        }

        lines.push(Line::from(""));
    }

    // Thinking indicator
    if app.is_thinking {
        lines.push(Line::from(vec![Span::styled(
            " MINIPWN ",
            Style::default()
                .fg(app.theme.background())
                .bg(app.theme.assistant_bubble())
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "  Thinking...",
            Style::default()
                .fg(app.theme.text_dim())
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
        .block(Block::default().style(Style::default().bg(app.theme.background())))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the text input box and suggestions.
fn render_input_box(f: &mut Frame, area: Rect, app: &App) {
    let chunks = if !app.suggestions.is_empty() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Suggestions
                Constraint::Length(3), // Input box
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area)
    };

    if !app.suggestions.is_empty() {
        let suggestion_line = Line::from(vec![
            Span::styled(" Suggestions: ", Style::default().fg(app.theme.text_dim())),
            Span::styled(app.suggestions.join("  "), Style::default().fg(app.theme.secondary()).add_modifier(Modifier::BOLD)),
        ]);
        f.render_widget(Paragraph::new(suggestion_line), chunks[0]);
    }

    let input_area = if !app.suggestions.is_empty() { chunks[1] } else { chunks[1] };

    let label = if app.is_thinking {
        " Thinking... "
    } else {
        " Message "
    };

    // Show cursor as a block character at cursor position
    let before = &app.input[..app.cursor];
    let cursor_char = app.input.chars().nth(app.cursor).unwrap_or(' ');
    let after = if app.cursor < app.input.len() {
        let next = app.cursor + cursor_char.len_utf8();
        if next <= app.input.len() {
             &app.input[next..]
        } else {
            ""
        }
    } else {
        ""
    };

    let text = Line::from(vec![
        Span::styled(before.to_string(), Style::default().fg(app.theme.text())),
        Span::styled(
            cursor_char.to_string(),
            if !app.is_thinking {
                Style::default().fg(app.theme.background()).bg(app.theme.text())
            } else {
                Style::default()
            },
        ),
        Span::styled(after.to_string(), Style::default().fg(app.theme.text())),
    ]);

    let input = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(label, Style::default().fg(app.theme.title()).add_modifier(Modifier::BOLD)))
            .border_style(Style::default().fg(app.theme.border())),
    ).style(Style::default().bg(app.theme.background()));
    
    f.render_widget(input, input_area);
}

/// Render the bottom status bar.
fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let status_text = if app.status.is_empty() {
        format!(
            " chat:{} | provider:{} | theme:{} | Ctrl+C quit | PgUp/PgDn scroll",
            app.chat_id,
            app.provider.display_name(),
            app.theme.name,
        )
    } else {
        format!(" {}", app.status)
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(app.theme.text_dim()).bg(app.theme.surface()));
    f.render_widget(status, area);
}
