//! TUI rendering: minimalist, centered chat bubbles with fixed-width backgrounds.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use super::app::App;

/// Main render function called every frame.
pub fn render_ui(f: &mut Frame, app: &App) {
    let area = f.size();
    
    // No global background as requested

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Chat area
            Constraint::Length(if !app.suggestions.is_empty() { 3 } else { 2 }), // Floating Input + suggestions
            Constraint::Length(1), // Minimal Status bar
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
    let title_span = Span::styled(
        format!(" ▣ MINIPWN v0.1.0 "),
        Style::default().fg(app.theme.primary()).add_modifier(Modifier::BOLD)
    );
    
    let chat_span = Span::styled(
        format!(" • {} ", app.chat_id),
        Style::default().fg(app.theme.text_dim())
    );

    let tokens_span = Span::styled(
        format!(" • {} [T] ", app.stats.total_tokens),
        Style::default().fg(app.theme.secondary())
    );

    let p = Paragraph::new(Line::from(vec![title_span, chat_span, tokens_span]))
        .alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_welcome(f: &mut Frame, area: Rect, app: &App) {
    let logo_lines = vec![
        Line::from(""),
        Line::from(Span::styled("█▀▄▀█ █ █▄░█ █ █▀█ █░█░█ █▄░█", Style::default().fg(app.theme.primary()))),
        Line::from(Span::styled("█░▀░█ █ █░▀█ █ █▀▀ ▀▄▀▄▀ █░▀█", Style::default().fg(app.theme.primary()))),
        Line::from(""),
        Line::from(Span::styled("  ◈  Autonomous Pentesting ◈  ", Style::default().fg(app.theme.secondary()).add_modifier(Modifier::ITALIC))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Type ", Style::default().fg(app.theme.text_dim())),
            Span::styled("/help", Style::default().fg(app.theme.primary())),
            Span::styled(" to see available commands.", Style::default().fg(app.theme.text_dim())),
        ]),
    ];

    let welcome = Paragraph::new(logo_lines).alignment(Alignment::Center);
    
    let vertical_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Percentage(30),
        ])
        .split(area);

    f.render_widget(welcome, vertical_center[1]);
}

fn render_bubbles(f: &mut Frame, area: Rect, app: &App) {
    // Center the conversation
    let chat_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(area);

    let inner_area = chat_layout[1];
    
    // We need to calculate the scroll manually since we are rendering multiple widgets
    let mut total_lines: Vec<Line> = Vec::new();
    let mut bubble_info = Vec::new();

    for bubble in &app.bubbles {
        let (icon, role_name, color, text_color) = if bubble.role == "tool" {
            ("◆", "TOOL RESULT", if bubble.content.contains("OK") { app.theme.success() } else { app.theme.error() }, app.theme.text())
        } else if bubble.role == "user" {
            ("◇", "YOU", app.theme.user_bubble(), app.theme.text())
        } else if bubble.is_ephemeral {
            ("»", "COMMAND", app.theme.secondary(), app.theme.text_dim())
        } else {
            ("◈", "MINIPWN", app.theme.assistant_bubble(), app.theme.text())
        };

        let width = inner_area.width as usize;

        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                Span::styled(format!("{} ", role_name), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled("─".repeat(width.saturating_sub(role_name.len() + icon.len() + 3)), Style::default().fg(app.theme.surface())),
            ]),
            Line::from(Span::styled(" ".repeat(width), Style::default().bg(app.theme.surface()))),
        ];

        let opacity = if bubble.is_ephemeral { Modifier::ITALIC } else { Modifier::empty() };
        for content_line in bubble.content.lines() {
            let mut line_content = format!("    {}", content_line);
            if line_content.len() < width {
                line_content.push_str(&" ".repeat(width - line_content.len()));
            } else {
                line_content = line_content[..width].to_string(); // Truncate if too long for now
            }

            lines.push(Line::from(vec![
                Span::styled(line_content, Style::default().fg(text_color).bg(app.theme.surface()).add_modifier(opacity)),
            ]));
        }
        lines.push(Line::from(Span::styled(" ".repeat(width), Style::default().bg(app.theme.surface()))));
        lines.push(Line::from(""));

        bubble_info.push(lines);
    }

    if app.is_thinking {
        bubble_info.push(vec![
            Line::from(vec![
                Span::styled(" ◈ ", Style::default().fg(app.theme.assistant_bubble())),
                Span::styled("MINIPWN is thinking", Style::default().fg(app.theme.assistant_bubble()).add_modifier(Modifier::ITALIC)),
                Span::styled(" ...", Style::default().fg(app.theme.assistant_bubble())),
            ]),
            Line::from(""),
        ]);
    }

    // Flatten to count total lines for scrolling
    let all_lines: Vec<Line> = bubble_info.iter().flatten().cloned().collect();
    let content_height = all_lines.len() as u16;
    let view_height = inner_area.height;
    let max_scroll = content_height.saturating_sub(view_height);
    let scroll = max_scroll.saturating_sub(app.scroll_offset);

    // Render using a single Paragraph to ensure background fills width correctly
    let paragraph = Paragraph::new(all_lines)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner_area);
}

fn render_input_box(f: &mut Frame, area: Rect, app: &App) {
    let input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

    let inner_area = input_layout[1];

    let chunks = if !app.suggestions.is_empty() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Suggestions
                Constraint::Length(1), // Minimal input line
            ])
            .split(inner_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(inner_area)
    };

    if !app.suggestions.is_empty() {
        let suggestion_line = Line::from(vec![
            Span::styled(" ◈ ", Style::default().fg(app.theme.secondary())),
            Span::styled(app.suggestions.join("  "), Style::default().fg(app.theme.text_dim())),
        ]);
        f.render_widget(Paragraph::new(suggestion_line), chunks[0]);
    }

    let prompt = if app.is_thinking { " ▣ " } else { " ❯ " };
    let before = &app.input[..app.cursor];
    let cursor_char = app.input.chars().nth(app.cursor).unwrap_or(' ');
    let after = if app.cursor < app.input.len() {
        let next = app.cursor + cursor_char.len_utf8();
        if next <= app.input.len() { &app.input[next..] } else { "" }
    } else { "" };

    let input_line = Line::from(vec![
        Span::styled(prompt, Style::default().fg(app.theme.primary()).add_modifier(Modifier::BOLD)),
        Span::styled(before.to_string(), Style::default().fg(app.theme.text())),
        Span::styled(
            cursor_char.to_string(),
            if !app.is_thinking {
                Style::default().bg(app.theme.text()) // Use default BG for cursor contrast
            } else {
                Style::default()
            },
        ),
        Span::styled(after.to_string(), Style::default().fg(app.theme.text())),
    ]);

    f.render_widget(Paragraph::new(input_line), chunks[if !app.suggestions.is_empty() { 1 } else { 1 }]);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode = match &app.execution_mode {
        crate::tools::ExecutionMode::Local { .. } => "◈ LOCAL",
        crate::tools::ExecutionMode::Remote { .. } => "◈ REMOTE",
    };

    let status_line = Line::from(vec![
        Span::styled(format!("  {}  ", mode), Style::default().fg(app.theme.primary()).bg(app.theme.surface())),
        Span::styled(format!("  {}  ", app.provider.display_name().to_uppercase()), Style::default().fg(app.theme.text_dim())),
        Span::styled(format!("  {}  ", app.theme.name.to_uppercase()), Style::default().fg(app.theme.text_dim())),
        Span::styled("  press / for commands  ", Style::default().fg(app.theme.text_dim()).add_modifier(Modifier::ITALIC)),
    ]);

    let p = Paragraph::new(status_line).alignment(Alignment::Right);
    f.render_widget(p, area);
}
