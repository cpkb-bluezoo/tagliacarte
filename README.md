# Tagliacarte

Tagliacarte is a desktop messaging client with a Rust core and a Qt 6 (C++/Widgets) interface. It aims to be cross-platform (macOS and Linux first), standards-based, and local-first. Today it works with email (IMAP/POP3/SMTP and local Maildir+ or mbox); the same message and folder model is designed to extend to direct messages (DMs) from protocols such as Nostr and Matrix, so you can have one place for email and DMs.

**This is a work in progress**

## Messages and folders (conversations / channels)

We use **folder**, **conversation**, and **channel** interchangeably—they all mean a container of messages (like a Slack or Discord channel, an email mailbox, a Nostr DM with one contact, or a Matrix room).

- **Messages** are the atomic unit: they have an identity (e.g. message-id, Nostr event id), envelope metadata (from, to, date, subject or equivalent), and a body. The exact shape depends on the source (email is MIME/RFC 5322; DMs have their own formats), but the abstraction is shared.
- **Folders** (conversations/channels) are how messages are grouped. The UI shows a list of folders; opening one shows its messages. For **email**, a folder is a mailbox (INBOX, Sent, …); within an email folder we can optionally group messages by **thread** (email-specific: subject + References/In-Reply-To). For Nostr and Matrix, each folder is one conversation/channel (one contact or one room).

Stores (IMAP, Maildir, and in future Nostr, Matrix) expose **folders**. Each folder has messages; for email we also support a **thread** view within a folder. New backends implement the same Store/Folder/Message/Transport traits and map their native concepts (Nostr DMs, Matrix rooms) into folders and messages.

## Goals

- **Unified model:** One message and folder (conversation/channel) abstraction across email and (future) DMs (Nostr, Matrix). Same UI concepts: folder list, message view, compose. Email adds optional thread view within a folder.
- **Standards-based:** Email via IMAP4rev2, POP3, SMTP; MIME and RFC 5322; Maildir+ and mbox. DMs would follow each protocol’s spec.
- **Clear architecture:** Core logic (stores, folders, messages, transport) in Rust; desktop UI in Qt 6, talking to the core via a thin C FFI. No UI logic in the core.
- **Cross-platform desktop:** Same core and FFI on all platforms; Qt provides a single codebase for macOS and Linux (and potentially Windows).
- **Local-first:** Local Maildir+ and mbox are first-class; indexing and search are intended to work offline. DM backends can follow the same local-first approach (local copy, sync when online).

## Architecture

- **`core/`** — Rust library that defines the main abstractions and implements the protocols and storage:
  - **Store / Folder / Message / Transport** — Unified model for any source that can be mapped to folders and messages. Today: email (IMAP, POP3, SMTP) and local (Maildir, mbox); designed so Nostr DMs, Matrix, etc. can plug in the same way. Email adds thread grouping within a folder.
  - **Email:** IMAP and POP3 for reading, SMTP for sending; SASL (PLAIN, SCRAM-SHA-256), TLS. MIME and RFC 5322 for envelope and body.
  - **Local storage:** Maildir+ and mbox backends; folder index keyed by message ID.
- **`ffi/`** — C API and Rust `cdylib` that expose Store, Folder, and Transport to C/C++. Used by the Qt UI.
- **`ui/`** — Qt 6 (C++/Widgets) application: K-9-style layout (sidebar, folder list, message view, compose). Presents folders and messages regardless of backend; compose and transport are backend-specific. Email folders can be viewed flat or by thread.

Future work: add backends for Nostr DMs and Matrix (and possibly mobile UIs). The core is designed so that new frontends and backends fit the same folder and message paradigm without changing the abstraction.

For the detailed architectural strategy (event-driven non-blocking model, folder/conversation/channel terminology, email threads, semantic send/receive, connection reuse, FFI event model), see **[ARCHITECTURE.md](ARCHITECTURE.md)**.

## Layout

- **`core/`** — Rust crate: Store/Folder/Message/Transport, IMAP, POP3, SMTP, Maildir, mbox, MIME, SASL.
- **`ffi/`** — Rust cdylib and C headers for the core API.
- **`ui/`** — Qt 6 application (sidebar, folders, message view, compose).
- **`icons/`** — App icon sources (e.g. `app-icon.svg`) and generated assets (e.g. `icon.icns`).

## Build

**Prerequisites:** Rust (via [rustup](https://rustup.rs)). The project has a `rust-toolchain.toml` so running `cargo` here uses the stable toolchain automatically (rustup will install it if needed; no global default required).

From the repo root:

- **Everything (Rust + Qt UI):** `make` or `make release`
- **Debug build:** `make debug`
- **Run the app:** `make run`
- **Tests:** `make test`
- **Clean:** `make clean`

**If CMake cannot find Qt 6**, set `QT_PREFIX` to your Qt 6 install:

- **macOS (Homebrew):** `make QT_PREFIX=$(brew --prefix qt@6)`  
  (Install Qt first: `brew install qt@6`.)
- **macOS (Qt online installer):** `make QT_PREFIX=~/Qt/6.x.x/macos` (or the path shown in Qt Maintenance Tool).
- **Linux:** `make QT_PREFIX=/path/to/Qt/6.x/gcc_64` or your distro’s Qt path.

**Prerequisites:** Rust (`rustup default stable`), CMake (≥3.16), Qt 6. See `ui/README.md` for details.

**Installing prerequisites (macOS):**
```bash
brew install cmake qt@6
# Then build with:
make QT_PREFIX=$(brew --prefix qt@6)
```
**Linux (apt):** `sudo apt install cmake qt6-base-dev` (or your distro’s Qt 6 package).

**App icon (macOS):** Run `make icons` to generate `icons/app-icon.png` and `icons/icon.icns` from `icons/app-icon.svg`. Rebuild the UI so the .app bundle uses the icon. Requires ImageMagick, librsvg, or Python cairosvg for SVG→PNG; on macOS, `sips` and `iconutil` (Xcode CLI tools) for `.icns`.

---

**Licence:** GPLv3. See [COPYING](COPYING) for the full text.

**Author:** Chris Burdess
