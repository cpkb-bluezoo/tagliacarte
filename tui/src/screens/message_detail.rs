use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::bridge::MessageDetail;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    detail: &MessageDetail,
    body_scroll: usize,
    scrollbar_state: &mut ScrollbarState,
    minimal_headers: bool,
) {
    let header_text = format!(
        "From: {}\nTo: {}\n{}{}Date: {}\nSubject: {}\n",
        detail.from,
        detail.to,
        if minimal_headers {
            String::new()
        } else {
            detail
                .cc
                .as_ref()
                .map(|c| format!("Cc: {c}\n"))
                .unwrap_or_default()
        },
        "",
        detail
            .date_ms
            .map(|ms| {
                use std::time::{Duration, UNIX_EPOCH};
                UNIX_EPOCH
                    .checked_add(Duration::from_millis(ms as u64))
                    .and_then(|d| {
                        let dt: chrono::DateTime<chrono::Local> = d.into();
                        Some(dt.format("%Y-%m-%d %H:%M").to_string())
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        detail.subject,
    );

    let body = detail
        .body_plain
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            detail
                .body_html
                .as_deref()
                .map(strip_html_tags)
        })
        .unwrap_or_else(|| "(No text body)".to_string());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    let header_p = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Message"),
    );
    f.render_widget(header_p, chunks[0]);

    let body_block = Block::default().borders(Borders::ALL).title("Body");
    let inner = body_block.inner(chunks[1]);
    f.render_widget(body_block, chunks[1]);

    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len().max(1);
    let visible = inner.height.saturating_sub(2) as usize;
    let max_scroll = total.saturating_sub(visible);
    let scroll = body_scroll.min(max_scroll);

    let slice: String = lines
        .iter()
        .skip(scroll)
        .take(visible.max(1))
        .copied()
        .collect::<Vec<_>>()
        .join("\n");

    let body_p = Paragraph::new(slice).style(Style::default().fg(Color::White));
    let inner2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    f.render_widget(body_p, inner2[0]);

    *scrollbar_state = ScrollbarState::new(total)
        .viewport_content_length(visible.max(1))
        .position(scroll);

    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        inner2[1],
        scrollbar_state,
    );
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
