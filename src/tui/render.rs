//! TUI rendering: minimalist, centered chat bubbles with fixed-width backgrounds.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use pulldown_cmark::{Parser, Event, Tag, TagEnd};

use super::app::App;
use crate::tui::theme::Theme;

/// Main render function called every frame.
pub fn render_ui(f: &mut Frame, app: &App) {
    let area = f.size();

    // No global background as requested

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Chat area
            Constraint::Length(if !app.suggestions.is_empty() {
                app.suggestions.len() as u16 + 3
            } else {
                1
            }), // Floating Input + suggestions
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

    if app.modal.is_some() {
        render_modal(f, app);
    }
}

fn parse_markdown<'a>(text: &'a str, theme: &Theme) -> Vec<Line<'a>> {
    let parser = Parser::new(text);
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut current_style = Style::default().fg(theme.text());
    let mut tag_stack = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => {
                tag_stack.push(tag.clone());
                match tag {
                    Tag::Strong => current_style = current_style.add_modifier(Modifier::BOLD),
                    Tag::Emphasis => current_style = current_style.add_modifier(Modifier::ITALIC),
                    Tag::Strikethrough => current_style = current_style.add_modifier(Modifier::CROSSED_OUT),
                    Tag::Link { .. } => current_style = current_style.fg(theme.primary()).add_modifier(Modifier::UNDERLINED),
                    Tag::Heading { .. } => current_style = current_style.add_modifier(Modifier::BOLD).fg(theme.primary()),
                    Tag::BlockQuote(_) => current_style = current_style.fg(theme.text_dim()).add_modifier(Modifier::ITALIC),
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                tag_stack.pop();
                current_style = Style::default().fg(theme.text());
                for tag in &tag_stack {
                    match tag {
                        Tag::Strong => current_style = current_style.add_modifier(Modifier::BOLD),
                        Tag::Emphasis => current_style = current_style.add_modifier(Modifier::ITALIC),
                        Tag::Strikethrough => current_style = current_style.add_modifier(Modifier::CROSSED_OUT),
                        Tag::Link { .. } => current_style = current_style.fg(theme.primary()).add_modifier(Modifier::UNDERLINED),
                        Tag::Heading { .. } => current_style = current_style.add_modifier(Modifier::BOLD).fg(theme.primary()),
                        Tag::BlockQuote(_) => current_style = current_style.fg(theme.text_dim()).add_modifier(Modifier::ITALIC),
                        _ => {}
                    }
                }
                
                match tag_end {
                    TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => {
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                current_line.push(Span::styled(t.to_string(), current_style));
            }
            Event::Code(t) => {
                current_line.push(Span::styled(format!(" {} ", t), current_style.bg(theme.surface()).fg(theme.secondary())));
            }
            Event::SoftBreak | Event::HardBreak => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
            }
            _ => {}
        }
    }
    
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    
    lines
}

fn wrap_line<'a>(line: Line<'a>, max_width: usize) -> Vec<Line<'a>> {
    let mut wrapped = Vec::new();
    let mut current_spans = Vec::new();
    let mut current_width = 0;

    for span in line.spans {
        let content = span.content.as_ref();
        let words = content.split_inclusive(' ');

        for word in words {
            let word_width = word.chars().count();
            
            if current_width + word_width > max_width && current_width > 0 {
                wrapped.push(Line::from(current_spans));
                current_spans = Vec::new();
                current_width = 0;
            }

            // Handle very long words by force-breaking
            let mut word_rest = word;
            while word_rest.chars().count() > max_width {
                let (head, tail) = word_rest.split_at(max_width);
                wrapped.push(Line::from(vec![Span::styled(head.to_string(), span.style)]));
                word_rest = tail;
            }

            if !word_rest.is_empty() {
                current_spans.push(Span::styled(word_rest.to_string(), span.style));
                current_width += word_rest.chars().count();
            }
        }
    }

    if !current_spans.is_empty() {
        wrapped.push(Line::from(current_spans));
    }

    wrapped
}

fn render_modal(f: &mut Frame, app: &App) {
    let modal = app.modal.as_ref().unwrap();
    let area = f.size();

    let num_items = modal.items.len();
    let show_search = num_items > 5;

    let mut inner_height = num_items as u16;
    if show_search {
        inner_height += 2;
    }
    inner_height += 1; // Help line

    let total_height = inner_height + 2; // Borders
    let max_height = (area.height * 80 / 100) as u16;
    let final_height = std::cmp::min(total_height, max_height);

    let modal_area = centered_rect_fixed_height(60, final_height, area);

    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary()))
        .title(Span::styled(
            format!(" {} ", modal.title),
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(app.theme.background()));

    let constraints = if show_search {
        vec![
            Constraint::Length(2), // Search
            Constraint::Min(0),    // List
            Constraint::Length(1), // Help
        ]
    } else {
        vec![
            Constraint::Min(0),    // List
            Constraint::Length(1), // Help
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .margin(1)
        .split(modal_area);

    let mut list_chunk_idx = 0;

    if show_search {
        let filter_text = Line::from(vec![
            Span::styled(" Search: ", Style::default().fg(app.theme.text_dim())),
            Span::styled(modal.filter.clone(), Style::default().fg(app.theme.text())),
            Span::styled("█", Style::default().fg(app.theme.primary())),
        ]);
        f.render_widget(Paragraph::new(filter_text), chunks[0]);
        list_chunk_idx = 1;
    }

    let help_chunk_idx = list_chunk_idx + 1;

    // Items
    let filtered_items: Vec<_> = modal
        .items
        .iter()
        .filter(|i| {
            i.label
                .to_lowercase()
                .contains(&modal.filter.to_lowercase())
        })
        .collect();

    let list_items: Vec<ListItem> = filtered_items
        .iter()
        .map(|i| ListItem::new(i.label.as_str()).style(Style::default().fg(app.theme.text())))
        .collect();

    let mut state = ListState::default();
    state.select(Some(modal.selected));

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ❯ ");

    f.render_stateful_widget(list, chunks[list_chunk_idx], &mut state);

    let help = Paragraph::new(" [Enter] Select   [Esc] Cancel   [↑↓] Navigate ")
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.theme.text_dim()));
    f.render_widget(help, chunks[help_chunk_idx]);

    f.render_widget(block, modal_area);
}

fn centered_rect_fixed_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let vertical_pad = r.height.saturating_sub(height) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vertical_pad),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_title_bar(f: &mut Frame, area: Rect, app: &App) {
    let title_span = Span::styled(
        format!(" ▣ MINIPWN v0.1.0 "),
        Style::default()
            .fg(app.theme.primary())
            .add_modifier(Modifier::BOLD),
    );

    let chat_span = Span::styled(
        format!(" • {} ", app.chat_id),
        Style::default().fg(app.theme.text_dim()),
    );

    let mode_color = if app.meta.mode == "safe" {
        app.theme.success()
    } else {
        app.theme.error()
    };
    let mode_span = Span::styled(
        format!(" • {} ", app.meta.mode.to_uppercase()),
        Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
    );

    let tokens_span = Span::styled(
        format!(" • {} [T] ", app.stats.total_tokens),
        Style::default().fg(app.theme.secondary()),
    );

    let p = Paragraph::new(Line::from(vec![
        title_span,
        chat_span,
        mode_span,
        tokens_span,
    ]))
    .alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_welcome(f: &mut Frame, area: Rect, app: &App) {
    let logo_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "█▀▄▀█ █ █▄░█ █ █▀█ █░█░█ █▄░█",
            Style::default().fg(app.theme.primary()),
        )),
        Line::from(Span::styled(
            "█░▀░█ █ █░▀█ █ █▀▀ ▀▄▀▄▀ █░▀█",
            Style::default().fg(app.theme.primary()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ◈  Autonomous Pentesting ◈  ",
            Style::default()
                .fg(app.theme.secondary())
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Type ", Style::default().fg(app.theme.text_dim())),
            Span::styled("/help", Style::default().fg(app.theme.primary())),
            Span::styled(
                " to see available commands.",
                Style::default().fg(app.theme.text_dim()),
            ),
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
    let mut bubble_info = Vec::new();

    for bubble in &app.bubbles {
        let (icon, role_name, color, text_color) = if bubble.role == "tool" {
            let color = if bubble.content.contains("Success:") {
                app.theme.success()
            } else if bubble.content.contains("Error:") {
                app.theme.error()
            } else {
                app.theme.secondary()
            };
            ("◆", "TOOL RESULT", color, app.theme.text_dim())
        } else if bubble.role == "user" {
            ("◇", "YOU", app.theme.user_bubble(), app.theme.text())
        } else if bubble.is_ephemeral {
            ("»", "COMMAND", app.theme.secondary(), app.theme.text_dim())
        } else {
            (
                "◈",
                "MINIPWN",
                app.theme.assistant_bubble(),
                app.theme.text(),
            )
        };

        let max_bubble_width = (inner_area.width.saturating_sub(20)) as usize;
        let is_user = bubble.role == "user";

        let max_text_width = max_bubble_width.saturating_sub(4); // -1 for border, -3 for left/right padding

        let mut wrapped_lines = Vec::new();
        let logical_lines = parse_markdown(&bubble.content, &app.theme);
        for line in logical_lines {
            wrapped_lines.extend(wrap_line(line, max_text_width));
        }

        let max_line_len = wrapped_lines
            .iter()
            .map(|l| l.width())
            .max()
            .unwrap_or(0);
        let title_len = role_name.chars().count() + icon.chars().count() + 3;

        let mut actual_bubble_width = std::cmp::max(title_len, max_line_len + 3);
        actual_bubble_width = std::cmp::min(actual_bubble_width, max_bubble_width);

        let margin_spaces = inner_area.width as usize - actual_bubble_width;
        let left_margin = if is_user { margin_spaces } else { 0 };

        let left_padding = Span::raw(" ".repeat(left_margin));

        let mut lines = vec![Line::from(vec![
            left_padding.clone(),
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(
                format!("{} ", role_name),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "─".repeat(actual_bubble_width.saturating_sub(title_len)),
                Style::default().fg(app.theme.surface()),
            ),
        ])];

        let opacity = if bubble.is_ephemeral {
            Modifier::ITALIC
        } else {
            Modifier::empty()
        };
        let border_span = Span::styled("▌", Style::default().fg(color).bg(app.theme.background()));

        for wrapped_line in wrapped_lines {
            let line_width = wrapped_line.width();
            let mut spans = vec![
                left_padding.clone(),
                border_span.clone(),
                Span::styled(" ", Style::default().bg(app.theme.background())),
            ];
            
            for mut span in wrapped_line.spans {
                span.style = span.style.bg(app.theme.background()).add_modifier(opacity);
                spans.push(span);
            }

            if line_width < actual_bubble_width.saturating_sub(2) {
                spans.push(Span::styled(
                    " ".repeat(actual_bubble_width.saturating_sub(2) - line_width),
                    Style::default().bg(app.theme.background()),
                ));
            }

            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));

        bubble_info.push(lines);
    }

    if app.is_thinking {
        bubble_info.push(vec![
            Line::from(vec![
                Span::styled(" ◈ ", Style::default().fg(app.theme.assistant_bubble())),
                Span::styled(
                    "MINIPWN is thinking",
                    Style::default()
                        .fg(app.theme.assistant_bubble())
                        .add_modifier(Modifier::ITALIC),
                ),
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
    let actual_scroll_offset = (app.scroll_offset as u16).min(max_scroll);
    let scroll = max_scroll.saturating_sub(actual_scroll_offset);

    // Render using a single Paragraph to ensure background fills width correctly
    let paragraph = Paragraph::new(all_lines)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner_area);

    if max_scroll > 0 {
        let scrollbar = ratatui::widgets::Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));

        let mut scrollbar_state = ratatui::widgets::ScrollbarState::default()
            .content_length(max_scroll as usize)
            .position(scroll as usize);

        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
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
                Constraint::Length(app.suggestions.len() as u16 + 2), // Suggestions + Borders
                Constraint::Length(1),                                // Minimal input line
            ])
            .split(inner_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner_area)
    };

    if !app.suggestions.is_empty() {
        let mut list_items = Vec::new();
        for suggestion in &app.suggestions {
            list_items.push(ListItem::new(Line::from(vec![
                Span::styled(" ◈ ", Style::default().fg(app.theme.secondary())),
                Span::styled(suggestion.clone(), Style::default().fg(app.theme.text())),
            ])));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.surface()));

        let list = List::new(list_items).block(block);
        f.render_widget(list, chunks[0]);
    }

    let prompt = if app.is_thinking { " ▣ " } else { " ❯ " };
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

    let input_line = Line::from(vec![
        Span::styled(
            prompt,
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        ),
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

    let input_para = Paragraph::new(input_line).block(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(app.theme.surface())),
    );

    f.render_widget(
        input_para,
        chunks[if !app.suggestions.is_empty() { 1 } else { 1 }],
    );
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let worker_label = match &app.execution_mode {
        crate::tools::ExecutionMode::Local { .. } => "◈ LOCAL".to_string(),
        crate::tools::ExecutionMode::Remote { client, .. } => {
            let host = client.base_url.trim_start_matches("http://").trim_start_matches("https://").split(':').next().unwrap_or("REMOTE");
            format!("◈ REMOTE ({})", host)
        }
    };

    let provider_name = app.provider.display_name().to_uppercase();
    let model_name = app
        .secrets
        .model(&app.provider)
        .unwrap_or("GPT-4O-MINI")
        .to_uppercase();

    // Left: [PROVIDER] [MODEL]
    let left_side = Line::from(vec![
        Span::styled(
            format!(" {} ", provider_name),
            Style::default()
                .fg(app.theme.background())
                .bg(app.theme.secondary())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", model_name),
            Style::default()
                .fg(app.theme.background())
                .bg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    // Right: [Theme] [Hint] [WorkerLabel]
    let right_side = Line::from(vec![
        Span::styled(
            format!("  {}  ", app.theme.name.to_uppercase()),
            Style::default().fg(app.theme.text_dim()),
        ),
        Span::styled(
            "  press / for commands  ",
            Style::default()
                .fg(app.theme.text_dim())
                .add_modifier(Modifier::ITALIC),
        ),
        Span::styled(
            format!("  {}  ", worker_label),
            Style::default()
                .fg(app.theme.primary())
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    f.render_widget(Paragraph::new(left_side).alignment(Alignment::Left), area);
    f.render_widget(Paragraph::new(right_side).alignment(Alignment::Right), area);
}
