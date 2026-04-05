use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::bridge::MessageSummary;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    messages: &[MessageSummary],
    selected: usize,
    list_state: &mut ListState,
    title: &str,
) {
    if messages.is_empty() {
        list_state.select(None);
    } else {
        list_state.select(Some(selected.min(messages.len() - 1)));
    }
    let items: Vec<ListItem> = messages
        .iter()
        .map(|m| {
            let read = if m.is_read { " " } else { "*" };
            let from = truncate(&m.from, 24);
            let subj = truncate(&m.subject, 48);
            let date = m
                .date_ms
                .map(|ms| format_date(ms))
                .unwrap_or_default();
            let line = format!("{read} {from:<24} {subj:<48} {date}");
            let style = if !m.is_read {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(Style::default().bg(Color::Blue));

    f.render_stateful_widget(list, area, list_state);
}

fn truncate(s: &str, max_chars: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if s.width() <= max_chars {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_chars.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

fn format_date(ms: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let Some(d) = UNIX_EPOCH.checked_add(Duration::from_millis(ms as u64)) else {
        return String::new();
    };
    let datetime: chrono::DateTime<chrono::Local> = d.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}
