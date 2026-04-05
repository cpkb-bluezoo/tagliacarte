use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Debug, Default)]
pub struct TextArea {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

impl TextArea {
    pub fn from_text(s: &str) -> Self {
        let lines: Vec<String> = if s.is_empty() {
            vec![String::new()]
        } else {
            s.lines().map(|l| l.to_string()).collect()
        };
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            lines,
            row: 0,
            col: 0,
        }
    }

    pub fn as_text(&self) -> String {
        self.lines.join("\n")
    }

    fn ensure_row(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.row >= self.lines.len() {
            self.row = self.lines.len() - 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.ensure_row();
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let line = &mut self.lines[self.row];
                line.insert(self.col, c);
                self.col += 1;
                true
            }
            KeyCode::Enter => {
                let rest: String = self.lines[self.row][self.col..].to_string();
                self.lines[self.row].truncate(self.col);
                self.row += 1;
                self.lines.insert(self.row, rest);
                self.col = 0;
                true
            }
            KeyCode::Backspace => {
                if self.col > 0 {
                    self.lines[self.row].remove(self.col - 1);
                    self.col -= 1;
                } else if self.row > 0 {
                    let cur = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                    self.lines[self.row].push_str(&cur);
                }
                true
            }
            KeyCode::Delete => {
                let line = &mut self.lines[self.row];
                if self.col < line.len() {
                    line.remove(self.col);
                } else if self.row + 1 < self.lines.len() {
                    let next = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next);
                }
                true
            }
            KeyCode::Left => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                }
                true
            }
            KeyCode::Right => {
                let line_len = self.lines[self.row].len();
                if self.col < line_len {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
                true
            }
            KeyCode::Up => {
                if self.row > 0 {
                    self.row -= 1;
                    self.col = self.col.min(self.lines[self.row].len());
                }
                true
            }
            KeyCode::Down => {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = self.col.min(self.lines[self.row].len());
                }
                true
            }
            KeyCode::Home => {
                self.col = 0;
                true
            }
            KeyCode::End => {
                self.col = self.lines[self.row].len();
                true
            }
            _ => false,
        }
    }
}
