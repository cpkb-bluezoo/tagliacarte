use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Tabs};
use ratatui::Frame;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    tab: usize,
    tab_titles: &[&str],
    lines: &[String],
    list_state: &mut ListState,
    selected_line: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let tabs = Tabs::new(tab_titles.iter().copied())
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .select(tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    if lines.is_empty() {
        list_state.select(None);
    } else {
        list_state.select(Some(selected_line.min(lines.len() - 1)));
    }
    let items: Vec<ListItem> = lines
        .iter()
        .map(|l| ListItem::new(l.as_str()))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("←/→ tabs · Enter edit · Esc back"),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[1], list_state);
}
