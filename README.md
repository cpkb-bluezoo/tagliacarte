# Tagliacarte

A desktop messaging client with a Rust core, a webview interface (wry) and a terminal interface. Cross-platform (Linux, macOS, Windows), low latency, standards-based, event-driven.

**Work in progress.**

![Tagliacarte screenshot](Screenshot.png)

## Features

- TLS (1.3, 1.2) support throughout
- SASL authentication mechanisms usable by all protocols:
  - `PLAIN`
  - `LOGIN`
  - `CRAM-MD5`
  - `SCRAM-SHA-256`
  - `XOAUTH2`
- OAuth2: PKCE auth code + refresh; Google + Microsoft bearer token store
- platform or local encrypted credentials storage
- IMAP4rev2
  - `STARTTLS` support
  - subscribed folders support
  - `UIDVALIDITY` + `EXISTS` for list invalidation
  - `UID SORT` (RFC 5256) when `SORT` advertised
  - `FETCH` / `UID FETCH`: streamed tuned MIME fragments
  - uses UIDs for efficiency if possible
  - mark/expunge or move to trash behaviour
  - idle support
  - second authenticated session for mail-body HTTPS: interleaved `FETCH` literals → chunked HTTP for WebView (cid: URLs in HTML body content)
- SMTP
  - `STARTTLS` support
  - use `BDAT` when `CHUNKING` possible
  - `DSN` + `NOTIFY=`
- POP3
  - `STLS` support
- NNTP
  - `STARTTLS` support
  - .newsrc newsgroup subscription
  - newsgroup browsing
- Maildir+ hierarchical local storage
- mbox single mailbox local storage
- Microsoft Graph API
- Gmail REST API
  - HTTP/2 parallel GET
- Nostr direct messages
  - WebSocket relays
  - NIP-04 kind 4
  - NIP-17 gift wraps (kind 1059)
  - NIP-65 / NIP-42 / kind 10050 relay discovery
- Matrix
  - Client-Server r0.3+
  - `sync`
  - per-room `messages`
  - E2EE hooks
- MIME
  - push MIME parser, low latency UI event pipeline
  - incremental Base64/QP
  - full RFC5322 push parser
- JSON
  - push JSON parser, low latency UI event pipeline
- unified email compose/reply/forward
  - plain or rich text editor
  - quoted content handling
  - attachment handling
  - optional outgoing OpenPGP / S/MIME sign or encrypt
- contacts database
  - CardDAV integration
  - platform address book integration
- i18n: 10 ARB locales (`l10n/`), shared by the GUI and the TUI

Session behaviour, event streams and backend-specific list strategies: `ARCHITECTURE.md`. The graphical interface is described in `gui/README.md`.

| Component         | Technology                           |
| ----------------- | ------------------------------------ |
| Core              | Rust                                 |
| App logic         | Rust (`tagliacarte_app`)             |
| GUI               | Rust (`gui/`, wry + tao) and a web UI of plain ES modules (`gui/ui/`) |
| Terminal UI       | Rust (`tui/` — ratatui + crossterm)  |
| Build             | Cargo, with Make for convenience     |
| Licence           | GPLv3                                |


### Terminal UI

Build: `make build-tui` (binary: `target/release/tagliacarte`). Run: `make run-tui`, or `cargo run -p tagliacarte --release -- /path/to/config.xml`.

Uses the same `config.xml` as the GUI (same default data directory as `tagliacarte_core::config::tagliacarte_data_dir` on each platform, or override `TAGLIACARTE_DATA_DIR` / `TAGLIACARTE_CONFIG_DIR`). Strings are generated at build time from the same ARB files as the GUI (`tui/build.rs` reads `l10n/app_*.arb`).

## Streaming architecture and minimal latency

Tagliacarte is designed around an end-to-end streaming, event-driven model so that content reaches the screen with minimal latency, even for large messages.

1. **Protocol layer** -- raw RFC 822 bytes are delivered from the server in chunks as they arrive.
2. **MIME parser** (`core/src/mime/`) -- a push-based, non-blocking parser. Each chunk is fed to `MimeParser::receive()`; complete lines are processed immediately and handler events (`start_entity`, `content_type`, `body_content`, `end_entity`, ...) fire synchronously. Base64 and quoted-printable transfer encodings are decoded incrementally with partial-quantum buffering.
3. **App layer** (`app/`) -- `tagliacarte_app` translates protocol events into high-level UI events.
4. **UI** (`gui/`, `tui/`) -- the page (or the terminal) reacts to the events. Plain-text and HTML content are rendered incrementally.

The result: the top of a message is visible while the rest is still streaming from the server.

## Internationalisation and localisation

Tagliacarte is fully internationalised. All user-facing strings are keyed 
(lowercase, dot-separated, e.g. `accounts. add_imap`) and support ICU MessageFormat where 
needed.

Translations are provided for **10 locales**: 
English, French, German, Spanish, Italian, 
Portuguese, Greek, Russian, Chinese 
(Simplified), and Japanese.

The string tables are the ARB files in `l10n/` (e.g. `app_en.arb`). The GUI serves them to the page as they are, and the terminal client compiles them in. Further locales are added by adding another `app_<lang>.arb` file.

Both pick up changed ARBs on the next `cargo build`; there is no separate l10n step.

## Layout

- **`core/`** - Rust crate: `Store` / `Folder` / `Transport` traits; IMAP, POP3, SMTP, NNTP, Maildir, mbox, MIME, SASL; HTTP/Graph, Gmail REST, Nostr, Matrix.
- **`app/`** - Rust app/controller layer used by both interfaces (`frb_api`, session, config XML).
- **`gui/`** - the graphical application: a wry window (`gui/src/`) serving a web UI (`gui/ui/`) with the folder sidebar, message list, reader, compose, chat-style view for Nostr/Matrix, calendar and settings. See `gui/README.md`.
- **`tui/`** - the terminal application.
- **`l10n/`** - the ARB string tables.
- **`icons/`** - Icon sources (the macOS application icon).

## Roadmap (future work)

Planned directions (not commitments or ordering):

1. IRC and XMPP providers
2. Calendar and tasks — CalDAV-style calendar and to-do support, Google Calendar and Microsoft Exchange integration

## Build

**Prerequisites:** Rust (via [rustup](https://rustup.rs)). Outgoing **S/MIME** and **OpenPGP** use in-process Rust crates (vendored OpenSSL for PKCS#7, rPGP for OpenPGP).

```bash
make                # tagliacarte_app and the GUI (release)
make run-gui        # run the GUI, serving the web UI live from gui/ui
make bundle-gui     # macOS: target/bundle/Tagliacarte.app
make run-tui        # run the terminal UI
make test           # run Cargo tests
make clean          # clean all build artefacts
```

The GUI's own end-to-end test (`tagliacarte-gui --selftest`) is described in `gui/README.md`.

---

**Licence:** GPLv3. See [COPYING](COPYING).

**Author:** Chris Burdess