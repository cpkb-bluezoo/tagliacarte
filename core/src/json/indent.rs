/*
 * indent.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Indentation config for JSON writer (same as jsonparser IndentConfig).

/// Indentation character and count per level for pretty-printed JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentConfig {
    indent_char: char,
    indent_count: usize,
}

impl IndentConfig {
    /// `indent_char` must be ' ' or '\t'; `indent_count` must be positive.
    pub fn new(indent_char: char, indent_count: usize) -> Option<Self> {
        if (indent_char != ' ' && indent_char != '\t') || indent_count == 0 {
            return None;
        }
        Some(Self {
            indent_char,
            indent_count,
        })
    }

    pub fn indent_char(&self) -> char {
        self.indent_char
    }

    pub fn indent_count(&self) -> usize {
        self.indent_count
    }

    pub fn tabs() -> Self {
        Self {
            indent_char: '\t',
            indent_count: 1,
        }
    }

    pub fn spaces2() -> Self {
        Self::spaces(2)
    }

    pub fn spaces4() -> Self {
        Self::spaces(4)
    }

    pub fn spaces(count: usize) -> Self {
        Self {
            indent_char: ' ',
            indent_count: count,
        }
    }

    /// Return the indent string for the given depth (newline + repeated indent).
    pub fn indent_for_depth(&self, depth: usize) -> String {
        let n = self.indent_count * depth;
        let mut s = String::with_capacity(1 + n);
        s.push('\n');
        for _ in 0..n {
            s.push(self.indent_char);
        }
        s
    }
}
