use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use tagliacarte_app::frb_api::FrbAccount;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Accounts,
    Folders,
}

pub fn draw(
    f: &mut Frame,
    area: Rect,
    accounts: &[FrbAccount],
    folders: &[String],
    account_sel: usize,
    folder_sel: usize,
    account_state: &mut ListState,
    folder_state: &mut ListState,
    focus: FocusPane,
    titles: (&str, &str),
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(10)])
        .split(area);

    if accounts.is_empty() {
        account_state.select(None);
    } else {
        account_state.select(Some(account_sel.min(accounts.len() - 1)));
    }
    let acc_items: Vec<ListItem> = accounts
        .iter()
        .map(|a| {
            let line = format!("{} ({})", truncate_label(&a.label, 22), a.backend_type);
            ListItem::new(line)
        })
        .collect();
    let acc_block = Block::default()
        .borders(Borders::ALL)
        .title(titles.0)
        .border_style(if focus == FocusPane::Accounts {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let acc_list = List::new(acc_items)
        .block(acc_block)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));
    f.render_stateful_widget(acc_list, chunks[0], account_state);

    if folders.is_empty() {
        folder_state.select(None);
        f.render_widget(
            Paragraph::new("—").block(Block::default().borders(Borders::ALL).title(titles.1)),
            chunks[1],
        );
    } else {
        folder_state.select(Some(folder_sel.min(folders.len() - 1)));
        let fold_items: Vec<ListItem> = folders
            .iter()
            .map(|n| ListItem::new(n.as_str()))
            .collect();
        let fold_block = Block::default()
            .borders(Borders::ALL)
            .title(titles.1)
            .border_style(if focus == FocusPane::Folders {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        let fold_list = List::new(fold_items)
            .block(fold_block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));
        f.render_stateful_widget(fold_list, chunks[1], folder_state);
    }
}

fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).chain(Some('…')).collect()
    }
}
