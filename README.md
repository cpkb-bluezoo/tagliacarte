# Tagliacarte

A desktop/mobile messaging client with a Rust core and Flutter interface. Cross-platform (Android, iOS, Linux, macOS), low latency, standards-based, local-first.

**Work in progress.**

![Screenshot](Screenshot.png)

## Protocols and storage

| Layer | Supported |
|-------|-----------|
| **Mail retrieval** | IMAP4rev2 (STARTTLS, implicit TLS), POP3 (implicit TLS) |
| **Mail transport** | SMTP (STARTTLS, implicit TLS, BDAT/CHUNKING) |
| **Authentication** | SASL: PLAIN, LOGIN, CRAM-MD5, SCRAM-SHA-256 |
| **Message format** | MIME, RFC 5322 |
| **Local storage** | Maildir+, mbox |
| **Future** | Nostr DMs (WebSocket relays), Matrix (HTTP) |

## Technologies

| Component | Technology |
|-----------|------------|
| Core | Rust |
| App logic | Rust (`tagliacarte_app`) |
| UI | Flutter (Dart) |
| Rust/Dart interop | `flutter_rust_bridge` |
| Build | Cargo + Flutter orchestrated by Make |
| Licence | GPLv3 |

## Streaming architecture and minimal latency

Tagliacarte is designed around an end-to-end streaming, event-driven model so that content reaches the screen with minimal latency, even for large messages.

1. **Protocol layer** -- raw RFC 822 bytes are delivered from the server in chunks as they arrive.
2. **MIME parser** (`core/src/mime/`) -- a push-based, non-blocking parser. Each chunk is fed to `MimeParser::receive()`; complete lines are processed immediately and handler events (`start_entity`, `content_type`, `body_content`, `end_entity`, ...) fire synchronously. Base64 and quoted-printable transfer encodings are decoded incrementally with partial-quantum buffering.
3. **App layer** (`app/`) -- `tagliacarte_app` translates protocol events into high-level UI/view-model events for Flutter.
4. **UI** (`flutter_ui/`) -- Riverpod state reacts to Rust events. Plain-text and HTML content are rendered incrementally.

The result: the top of a message is visible while the rest is still streaming from the server.

## Internationalisation and localisation

Tagliacarte is fully internationalised. All user-facing strings are keyed 
(lowercase, dot-separated, e.g. `accounts.
add_imap`) and support ICU MessageFormat where 
needed.

Translations are provided for **10 locales**: 
English, French, German, Spanish, Italian, 
Portuguese, Greek, Russian, Chinese 
(Simplified), and Japanese.

The Flutter UI uses **gen-l10n** with ARB files under `flutter_ui/lib/src/l10n/` (e.g. `app_en.arb`). Further locales can be added by introducing additional ARB files and enabling them in `l10n.yaml`. After editing ARBs, run `flutter gen-l10n` from `flutter_ui/` with **no** command-line overrides (paths and class names come from `l10n.yaml`). Flutter may print that CLI options are ignored when `l10n.yaml` is present; that is normal and means generation succeeded.

## Layout

- **`core/`** -- Rust crate: Store / Folder / Message / Transport traits; IMAP, POP3, SMTP, Maildir, mbox, MIME, SASL.
- **`app/`** -- Rust app/controller layer consumed by Flutter.
- **`flutter_ui/`** -- Flutter application (folder sidebar, message list, message view, compose, settings).
- **`icons/`** -- Extra icon sources (SVG); the Flutter app bundles icons from `flutter_ui/assets/icons/`.

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
