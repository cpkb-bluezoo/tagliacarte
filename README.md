# Tagliacarte

A desktop/mobile messaging client with a Rust core and Flutter interface. Cross-platform (Android, iOS, Linux, macOS), low latency, standards-based, local-first.

**Work in progress.**

Screenshot

## Protocols and storage


| Layer                  | Status                                                                                                                                                                                 |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Mail retrieval**     | **IMAP4rev2** — persistent session, IDLE-friendly streaming; **SORT** when the server advertises it. **POP3** — single-folder (INBOX) model; connect per operation.                    |
| **Mail transport**     | **SMTP** — `MAIL`/`RCPT`/`DATA` or **BDAT** when **CHUNKING** is advertised; **DSN** `NOTIFY` when supported.                                                                          |
| **Authentication**     | **SASL** for mail protocols: **PLAIN**, **LOGIN**, **CRAM-MD5**, **SCRAM-SHA-256**; **XOAUTH2** for OAuth2-based provider sign-in (see below).                                         |
| **Message format**     | MIME, RFC 5322                                                                                                                                                                         |
| **Local storage**      | Maildir+, mbox                                                                                                                                                                         |
| **Conversation-style** | **Nostr** — DMs via WebSocket relays (NIP-04 / NIP-17), chat-style UI. **Matrix** — rooms over HTTP; same list/session model as mail, but **still needs reliability and UX bugfixes**. |
| **Usenet**             | **NNTP** — `LIST` / `LIST ACTIVE` for **all** newsgroups (subscribed-folder / client-side filtering and IMAP **`LSUB`**-style parity are planned). Message rows use **`OVER`** (overview); opening a message uses **`ARTICLE`** (full RFC 822, i.e. complete headers + body for MIME parsing). **Not yet validated end-to-end** in daily use. |


### Connection security (TLS)

How the TCP connection is protected is independent of the mailbox protocol. Rustls is used for TLS; the trust store follows the usual platform + Mozilla root pattern (see `core/src/net.rs`). Typical **implicit TLS** ports are **993** (IMAP), **995** (POP3), **465** (SMTP), **563** (NNTP). On **cleartext** ports, the client upgrades the same socket when configured to do so:


| Upgrade command     | Protocol         | Typical cleartext port (examples) |
| ------------------- | ---------------- | --------------------------------- |
| **STARTTLS**        | IMAP, SMTP, NNTP | 143, 587, 119                     |
| **STLS** (RFC 2595) | POP3             | 110                               |


For **POP3**, the store uses **implicit TLS** on **995** (`pop3s`); on **110** it issues `**CAPA`**, requires `**STLS**` in the capability list, then upgrades before `USER`/`PASS`.

When **opportunistic TLS** is enabled on a **plain** TCP connection (the default for IMAP 143, SMTP 587, NNTP 119, POP3 110), the client **probes server capabilities** (`CAPABILITY` / EHLO / `CAPABILITIES` / POP3 `CAPA`) and **must** see the corresponding upgrade (`STARTTLS` or `STLS`). If the upgrade is missing, or the server rejects the command, or the TLS handshake fails, the client **errors and does not authenticate** over cleartext. Turning opportunistic TLS off is only for debugging and allows cleartext auth where the server accepts it.

### Gmail and Microsoft / Exchange


| Provider                                          | How Tagliacarte talks to it                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Google Gmail**                                  | Dedicated URIs `**gmail://`** (mailbox) and `**gmail+smtp://**` (send) target Google’s **IMAP** and **SMTP** endpoints with **XOAUTH2**. There is no separate Gmail REST API for mail storage in the current design; behaviour matches generic IMAP/SMTP with OAuth2 tokens.                                     |
| **Microsoft 365 / Outlook.com / Exchange Online** | `**graph://`** (mailbox) and `**graph+send://**` (send) use the **Microsoft Graph** mail API where integrated — treat as **experimental**. **On-premises Exchange** (or any host offering standard mail ports) is configured as ordinary **IMAP** / **SMTP** (and POP3 if you use it), with the TLS modes above. |


OAuth token handling and paging differ by backend; see `ARCHITECTURE.md` for session behaviour, sorting, and Graph vs IMAP trade-offs.

## Technologies


| Component         | Technology                           |
| ----------------- | ------------------------------------ |
| Core              | Rust                                 |
| App logic         | Rust (`tagliacarte_app`)             |
| UI                | Flutter (Dart)                       |
| Rust/Dart interop | `flutter_rust_bridge`                |
| Build             | Cargo + Flutter orchestrated by Make |
| Licence           | GPLv3                                |


## Streaming architecture and minimal latency

Tagliacarte is designed around an end-to-end streaming, event-driven model so that content reaches the screen with minimal latency, even for large messages.

1. **Protocol layer** -- raw RFC 822 bytes are delivered from the server in chunks as they arrive.
2. **MIME parser** (`core/src/mime/`) -- a push-based, non-blocking parser. Each chunk is fed to `MimeParser::receive()`; complete lines are processed immediately and handler events (`start_entity`, `content_type`, `body_content`, `end_entity`, ...) fire synchronously. Base64 and quoted-printable transfer encodings are decoded incrementally with partial-quantum buffering.
3. **App layer** (`app/`) -- `tagliacarte_app` translates protocol events into high-level UI/view-model events for Flutter.
4. **UI** (`flutter_ui/`) -- Riverpod state reacts to Rust events. Plain-text and HTML content are rendered incrementally.

The result: the top of a message is visible while the rest is still streaming from the server.

## Internationalisation and localisation

Tagliacarte is fully internationalised. All user-facing strings are keyed 
(lowercase, dot-separated, e.g. `accounts. add_imap`) and support ICU MessageFormat where 
needed.

Translations are provided for **10 locales**: 
English, French, German, Spanish, Italian, 
Portuguese, Greek, Russian, Chinese 
(Simplified), and Japanese.

The Flutter UI uses **gen-l10n** with ARB files under `flutter_ui/lib/src/l10n/` (e.g. `app_en.arb`). Further locales can be added by introducing additional ARB files and enabling them in `l10n.yaml`. After editing ARBs, run `flutter gen-l10n` from `flutter_ui/` with **no** command-line overrides (paths and class names come from `l10n.yaml`). Flutter may print that CLI options are ignored when `l10n.yaml` is present; that is normal and means generation succeeded.

## Layout

- `**core/`** -- Rust crate: `Store` / `Folder` / `Transport` traits; IMAP, POP3, SMTP, NNTP, Maildir, mbox, MIME, SASL; Nostr, Matrix, Graph (and related) protocol modules.
- `**app/**` -- Rust app/controller layer consumed by Flutter (`frb_api`, session, config XML).
- `**flutter_ui/**` -- Flutter application (folder sidebar, message list, message detail, compose, chat-style view for Nostr/Matrix, settings).
- `**icons/**` -- Extra icon sources (SVG); the Flutter app bundles icons from `flutter_ui/assets/icons/`.

## Roadmap (future work)

Planned directions (not commitments or ordering):

1. S/MIME and GPG/PGP
2. Contacts database in the app, with integration with platform address books
3. IRC and XMPP providers
4. Calendar and tasks — CalDAV-style calendar and to-do support, Google Calendar and Microsoft Exchange integration
5. Rich-text / HTML editing in compose

## Build

**Prerequisites:** Rust (via [rustup](https://rustup.rs)), Flutter SDK.

```bash
make                # Rust app crate (release) + flutter pub get
make build-app      # build Rust app crate (release)
make flutter-run    # run Flutter app
make flutter-test   # run Flutter tests
make test           # run Cargo tests
make clean          # clean all build artefacts
```

To run integration tests:

```bash
make test-integration
```

---

**Licence:** GPLv3. See [COPYING](COPYING).

**Author:** Chris Burdess