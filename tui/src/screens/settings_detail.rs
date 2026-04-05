//! Read-only account / transport detail panes under Settings.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use tagliacarte_app::frb_api::{FrbAccount, FrbTransport};
use tagliacarte_app::mail_kind::{is_imap_like_store, is_nostr_store};

pub fn account_body(acc: &FrbAccount) -> String {
    let mut lines: Vec<String> = vec![
        format!("id: {}", acc.id),
        format!("label: {}", acc.label),
        format!("type: {}", acc.backend_type),
    ];
    if let Some(h) = acc.attrs.get("host") {
        lines.push(format!("host: {h}"));
    }
    if let Some(p) = acc.attrs.get("port") {
        lines.push(format!("port: {p}"));
    }
    if let Some(p) = acc.attrs.get("path") {
        lines.push(format!("path: {p}"));
    }
    if let Some(np) = acc.attrs.get("npub") {
        lines.push(format!("npub: {np}"));
    }
    if let Some(ru) = acc.lists.get("relayUrls") {
        lines.push(format!("relays: {}", ru.len()));
        for (i, u) in ru.iter().take(6).enumerate() {
            lines.push(format!("  {}. {u}", i + 1));
        }
        if ru.len() > 6 {
            lines.push(format!("  … (+{} more)", ru.len() - 6));
        }
    }
    lines.push(String::new());
    if is_nostr_store(acc.backend_type.as_str()) {
        lines.push("[e] Set Nostr secret (nsec or 64-char hex)".to_string());
    }
    if is_imap_like_store(acc.backend_type.as_str()) {
        lines.push("[m] Set mail password (username + password)".to_string());
    }
    lines.push("[Esc] Back to account list".to_string());
    lines.join("\n")
}

pub fn transport_body(t: &FrbTransport) -> String {
    [
        format!("id: {}", t.id),
        format!("type: {}", t.transport_type),
        format!("display: {}", t.display_name),
        format!("host: {}", t.host),
        format!("port: {}", t.port),
        format!("security: {}", t.security),
        format!("default From: {}", t.default_from),
        String::new(),
        "[p] Set SMTP password (uses default From as username if empty)".to_string(),
        "[Esc] Back to transport list".to_string(),
    ]
    .join("\n")
}

pub fn draw_account(f: &mut Frame, area: Rect, acc: &FrbAccount) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Account detail")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let p = Paragraph::new(account_body(acc))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(p, inner);
}

pub fn draw_transport(f: &mut Frame, area: Rect, t: &FrbTransport) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Outgoing transport detail")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let p = Paragraph::new(transport_body(t))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(p, inner);
}

/// Nostr nsec entry overlay (obscured line is only cosmetic; we still draw bullets for privacy).
pub fn draw_nostr_nsec_dialog(f: &mut Frame, area: Rect, account_id: &str, input_display: &str, error: &str) {
    let block_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(area);
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(block_area[1])[1];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Nostr secret — {account_id} "))
        .title_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(mid);
    f.render_widget(Clear, mid);
    f.render_widget(block, mid);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .split(inner);

    let hint = Paragraph::new(
        "Paste nsec1… or 64-char hex. Stored in the credential vault (same as GUI). Enter save · Esc cancel.",
    )
    .wrap(Wrap { trim: true });
    f.render_widget(hint, chunks[0]);

    let masked: String = if input_display.is_empty() {
        "(empty)".to_string()
    } else {
        "*".repeat(input_display.chars().count().min(64))
    };
    let field = Paragraph::new(format!("> {masked}"))
        .style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(field, chunks[1]);

    let err_color = if error.is_empty() {
        Color::DarkGray
    } else {
        Color::Red
    };
    let err_p = Paragraph::new(if error.is_empty() { " " } else { error }).style(Style::default().fg(err_color));
    f.render_widget(err_p, chunks[2]);

    let footer = Paragraph::new("Enter confirm · Esc cancel").alignment(Alignment::Center);
    f.render_widget(footer, chunks[3]);
}

/// SMTP / IMAP-style two-field overlay (compact).
pub fn draw_mail_credential_dialog(
    f: &mut Frame,
    area: Rect,
    title: &str,
    username: &str,
    password_masked: &str,
    field: usize,
    error: &str,
) {
    let block_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(55),
            Constraint::Percentage(25),
        ])
        .split(area);
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(12),
            Constraint::Percentage(76),
            Constraint::Percentage(12),
        ])
        .split(block_area[1])[1];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .title_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(mid);
    f.render_widget(Clear, mid);
    f.render_widget(block, mid);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner);

    let u_style = if field == 0 {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let p_style = if field == 1 {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(format!("Username: {username}")).style(u_style),
        chunks[1],
    );
    let pw_show = if password_masked.is_empty() {
        "(empty)".to_string()
    } else {
        "*".repeat(password_masked.chars().count().min(48))
    };
    f.render_widget(
        Paragraph::new(format!("Password: {pw_show}")).style(p_style),
        chunks[2],
    );

    let err_color = if error.is_empty() {
        Color::DarkGray
    } else {
        Color::Red
    };
    f.render_widget(
        Paragraph::new(if error.is_empty() { " " } else { error }).style(Style::default().fg(err_color)),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new("Tab fields · Enter save · Esc cancel").alignment(Alignment::Center),
        chunks[4],
    );
}
