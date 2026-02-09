/*
 * date_time.rs
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

//! RFC 5322 date-time parsing (section 3.3).

use chrono::{DateTime, FixedOffset};

/// Parse an RFC 5322 date-time string (e.g. "Fri, 21 Nov 1997 09:55:06 -0600").
/// Returns None on parse failure.
pub fn parse_rfc5322_date(value: &str) -> Option<DateTime<FixedOffset>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc2822(value).ok().or_else(|| parse_obsolete_date(value))
}

/// Obsolete formats: 2-digit year, optional seconds, legacy zone names.
fn parse_obsolete_date(value: &str) -> Option<DateTime<FixedOffset>> {
    let value = value.trim();
    let value = convert_obsolete_timezones(value);
    let value = convert_two_digit_year(&value);
    // Try without seconds first
    if let Some(dt) = chrono::DateTime::parse_from_str(&value, "%d %b %Y %H:%M %z").ok() {
        return Some(dt);
    }
    chrono::DateTime::parse_from_str(&value, "%d %b %Y %H:%M:%S %z").ok()
}

/// Convert 2-digit year to 4-digit (RFC 5322 4.5: 00-49 -> 2000-2049, 50-99 -> 1950-1999).
/// Only replace a 2-digit token that follows a month abbreviation (e.g. "Nov 99").
fn convert_two_digit_year(s: &str) -> String {
    const MONTHS: &[&str] = &[
        " Jan ", " Feb ", " Mar ", " Apr ", " May ", " Jun ",
        " Jul ", " Aug ", " Sep ", " Oct ", " Nov ", " Dec ",
    ];
    let mut s = s.to_string();
    for month in MONTHS {
        for (i, _) in s.match_indices(month) {
            let after = i + month.len();
            if after + 2 <= s.len()
                && s.as_bytes()[after].is_ascii_digit()
                && s.as_bytes()[after + 1].is_ascii_digit()
                && (after + 2 == s.len() || !s.as_bytes()[after + 2].is_ascii_digit())
            {
                let yy = (s.as_bytes()[after] - b'0') * 10 + (s.as_bytes()[after + 1] - b'0');
                let full = if yy <= 49 {
                    2000 + yy as u32
                } else {
                    1900 + yy as u32
                };
                s.replace_range(after..after + 2, &full.to_string());
                break;
            }
        }
    }
    s
}

fn convert_obsolete_timezones(s: &str) -> String {
    let s = s.replace(" GMT ", " +0000 ");
    let s = s.replace(" UT ", " +0000 ");
    let s = s.replace(" UTC ", " +0000 ");
    let s = s.replace(" EST ", " -0500 ");
    let s = s.replace(" EDT ", " -0400 ");
    let s = s.replace(" CST ", " -0600 ");
    let s = s.replace(" CDT ", " -0500 ");
    let s = s.replace(" MST ", " -0700 ");
    let s = s.replace(" MDT ", " -0600 ");
    let s = s.replace(" PST ", " -0800 ");
    let s = s.replace(" PDT ", " -0700 ");
    s
}
