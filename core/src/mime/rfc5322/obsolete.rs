/*
 * obsolete.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Obsolete but recoverable RFC 5322 structures (section 4.5).

/// Types of obsolete structure detected during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObsoleteStructureType {
    ObsoleteDateTimeSyntax,
    ObsoleteAddressSyntax,
    ObsoleteMessageIdSyntax,
}
