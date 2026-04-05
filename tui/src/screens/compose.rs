use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::widgets::TextArea;
use crate::widgets::TextInput;

#[derive(Clone, Copy)]
pub struct ComposeDraw<'a> {
    pub field: usize,
    pub from: &'a TextInput,
    pub to: &'a TextInput,
    pub cc: &'a TextInput,
    pub bcc: &'a TextInput,
    pub subject: &'a TextInput,
    pub body: &'a TextArea,
    pub transport_label: &'a str,
}

pub fn draw(f: &mut Frame, area: Rect, c: ComposeDraw<'_>) {
    let block = Block::default().borders(Borders::ALL).title("Compose");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(inner);

    draw_field(f, inner_chunks[0], "From", &c.from.text, c.field == 0);
    draw_field(f, inner_chunks[1], "To", &c.to.text, c.field == 1);
    draw_field(f, inner_chunks[2], "Cc", &c.cc.text, c.field == 2);
    draw_field(f, inner_chunks[3], "Bcc", &c.bcc.text, c.field == 3);
    draw_field(
        f,
        inner_chunks[4],
        "Subject",
        &c.subject.text,
        c.field == 4,
    );
    draw_field(
        f,
        inner_chunks[5],
        "Transport",
        c.transport_label,
        c.field == 5,
    );

    let body_text = c.body.as_text();
    let body_style = if c.field == 6 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let body_p = Paragraph::new(body_text.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Body — Tab · Ctrl+S send · Esc")
                .border_style(body_style),
        );
    f.render_widget(body_p, inner_chunks[6]);
}

fn draw_field(f: &mut Frame, area: Rect, label: &str, value: &str, focused: bool) {
    let style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let p = Paragraph::new(value.to_string())
        .block(Block::default().borders(Borders::ALL).title(label).border_style(style));
    f.render_widget(p, area);
}
