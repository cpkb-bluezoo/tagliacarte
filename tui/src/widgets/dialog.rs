use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogChoice {
    Yes,
    No,
}

pub struct Dialog {
    pub title: String,
    pub body: String,
    pub focus_yes: bool,
}

impl Dialog {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            focus_yes: true,
        }
    }

    pub fn draw(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.as_str())
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(2),
                Constraint::Length(3),
            ])
            .split(inner);

        let p = Paragraph::new(self.body.as_str())
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);
        f.render_widget(p, chunks[0]);

        let yes_style = if self.focus_yes {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let no_style = if !self.focus_yes {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);
        f.render_widget(
            Paragraph::new("[ Y ] Yes").style(yes_style).alignment(Alignment::Center),
            row[0],
        );
        f.render_widget(
            Paragraph::new("[ N ] No").style(no_style).alignment(Alignment::Center),
            row[1],
        );
    }

    pub fn toggle(&mut self) {
        self.focus_yes = !self.focus_yes;
    }

    pub fn choice(&self) -> DialogChoice {
        if self.focus_yes {
            DialogChoice::Yes
        } else {
            DialogChoice::No
        }
    }
}
