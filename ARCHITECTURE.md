# Tagliacarte architecture

This document captures the architectural strategy for Tagliacarte so we can refer back to it during implementation. It covers the core abstractions, the event-driven non-blocking model, folder/conversation/channel semantics, email-specific threads, semantic send/receive, connection reuse, and how Nostr and Matrix fit in.

Current frontend stack: Flutter (`flutter_ui/`) consuming Rust app logic from `tagliacarte_app` (`app/`) and backend logic from `tagliacarte_core` (`core/`). The former Qt/C++ UI (`ui/`) has been removed; **Flutter is the shipping client**.

**Terminology:** We use **folder**, **conversation**, and **channel** interchangeably. They all refer to the same concept: a container of messages (e.g. an email mailbox, a Slack or Discord channel, a Nostr DM with one contact, a Matrix room). **Threads** are email-specific: a thread groups messages within an email folder by subject + References/In-Reply-To. We do not call email threads “conversations”.

---

## 0. Implemented architecture (Flutter + FRB)

This section describes what exists in the tree today. Earlier sections (§1–§12) remain the **target** design; where behaviour differs, it is called out here.

### 0.1 Crates and directories

| Piece | Role |
|--------|------|
| **`core/`** (`tagliacarte_core`) | `Store` / `Folder` traits; IMAP (pipeline + tokio), Maildir, mbox, MIME (`MimeParser`, `extract_structured_body`), config XML I/O. |
| **`app/`** (`tagliacarte_app`) | **flutter_rust_bridge** entrypoints (`frb_api/`), config load/save (`frb_api/config_persist.rs`), mail helpers (`frb_api/frb_mail.rs`). The older `api/` module is internal / parallel API surface, not what Flutter calls. |
| **`flutter_ui/`** | Flutter app: Riverpod, screens (home, message detail, compose, settings), widgets, FRB-generated Dart bindings under `lib/src/rust/`. |
| **`tui/`** (`tagliacarte` binary) | Terminal UI: **ratatui** + **crossterm**, same `frb_api` + `session::start_session_native` as Flutter; ARB strings via `build.rs` from `flutter_ui/lib/src/l10n/`. |

Build orchestration: top-level **Makefile** (`make flutter-run`, `build-app`, `build-tui`, `run-tui`, etc.); Rust dylib loaded from `TAGLIACARTE_RUST_LIB` or bundled path on macOS (see `flutter_ui/lib/main.dart`).

### 0.2 Flutter ↔ Rust: flutter_rust_bridge

- Dart invokes **generated** `frb_*` functions (e.g. list folders, list messages, get message JSON, save config).
- Mail and config types cross the boundary as **JSON strings** or FRB-generated structs (`FrbConfig`, accounts, transports).
- **Store reuse:** `frb_mail` keeps an in-memory cache of open `Store` instances keyed by `(store_uri, credential_lookup_key, use_keychain)` so repeated operations (e.g. list then open message) do not reconnect IMAP every time.

### 0.3 Event model vs what Flutter sees

- **Inside `core/`** the `Store` / `Folder` APIs remain **callback-driven and non-blocking** (request returns immediately; `on_summary`, `on_content_chunk`, `on_complete`, etc.).
- **`frb_mail`** often bridges that to Dart by **aggregating** callbacks into a single result: it uses channels (e.g. `std::sync::mpsc`) and a **recv timeout** to collect all summaries or the full raw message, then serializes JSON and returns. From Dart/Riverpod this appears as an **`async` call** (`FutureProvider`, `await frb…`), which matches Flutter without exposing a C-style callback API for those paths.
- **App session folder list (Flutter):** Does **not** block on one aggregated JSON tree. The UI sends **`refreshFolders`** (or the account loop refreshes on its own); Rust emits **`folderFound`** (account id, folder name, unread) as each folder is known, then **`folderListUpdated`** with the **authoritative** full list so the UI can reconcile (remove stale folders). Flutter may call **`frb_list_mail_folders`** only where a synchronous snapshot is still appropriate (e.g. tooling); the main mail UI uses the session stream.
- **App session message list window (Flutter):** Does **not** block the isolate on **`frb_session_list_messages_window`**. The UI sends **`listMessagesWindow`** with a **`requestId`**; Rust runs the fetch on a worker thread and emits **`messageListWindowStarted`** (total, window metadata), **`messageListRowFound`** per summary, then **`messageListWindowComplete`**. The main list uses that stream; **`frb_session_list_messages_window`** remains for callers that want one JSON payload.
- **Directional goal:** Streaming/incremental UI updates for large bodies could be reintroduced later by extending the bridge; today the UI waits for one JSON payload per operation on most non-session mail calls.
- **App session lifecycle (multi-account):** On startup and on **`frb_session_reload_accounts`**, the Rust **`session`** module diffs configured account ids against the live session map. **New** accounts get a background worker (folder list, then IMAP idle polling or Nostr jobs as applicable). **Removed** accounts are signalled **`disconnected`**, their store cache entries invalidated, and long-running loops stopped via per-account cancel flags. **Changed** connection-related fields for an existing id (host, security maps, backend type, IMAP idle seconds, …) cancel the old worker, invalidate cache, and spawn a fresh loop. The Flutter **account rail** only chooses which account’s folders and lists are shown; it does **not** connect or disconnect stores—connections are session-wide.
- **Account selection (Flutter):** When at least one account exists, **`selectedAccountId`** always references a valid account (first account auto-selected when none was valid; after deleting the selected account, another account is selected). **`AccountSelectionSync`** (root `ProviderScope` child) reconciles selection whenever config reloads.
- **Credentials after saving an account:** For non-local stores, the app nudges the session and may show IMAP / Gmail OAuth / Nostr credential UI when the server reports missing or rejected credentials, without requiring the user to select the account on the home screen first.
- **Outgoing SMTP verify:** **`frb_verify_smtp_transport`** connects, authenticates with saved credentials, sends **QUIT**, and does not send mail. The outgoing transport editor runs this after save (with the same credential retry loop as compose when auth fails).

### 0.4 Mail: folders, list, read, folder management

- **Session-mediated list/detail (Flutter):** The UI passes **`accountId` + folder + sort** into a **`listMessagesWindow`** session command and consumes **`messageListWindow*`** stream events for the message list; **`frb_session_get_folder_message`** and **`frb_session_register_mail_body_store`** remain direct FRB calls for detail / WebView. The Rust **`AppSession`** maps the account to `store_uri` / credential key / keychain flag and delegates to the same `frb_mail` helpers as before. **Nostr** (`nostr:store:`) and **Matrix** (`matrix:store:`) stores are constructed in **`build_store`** so conversation folders use the same windowed list path as mail. **IMAP attachment download** still uses **`frb_fetch_folder_message_part`** with store URI + credential resolved from Flutter config (IMAP-only).
- **List folders:** Session stream: **`folderFound`** per folder + **`folderListUpdated`** (names, **unread per folder**, optional **IMAP hierarchy delimiter**). Direct **`frb_list_mail_folders`** JSON remains for snapshot-style callers.
- **List messages:** JSON array of row fields (id, from, subject, date) for the message list. **Ordering and paging strategy** (IMAP SORT vs full-folder fetch, POP3, Graph, Gmail) is spelled out in **§0.8**.
- **Read message:** `get_folder_message_json` loads raw RFC 822 bytes, runs **`extract_structured_body`**, and builds a detail JSON (envelope fields + `bodyPlain` / `bodyHtml`). If MIME extraction finds no text/html part, the implementation falls back to **UTF-8 text after the first RFC 822 header/body separator** (`\r\n\r\n` or `\n\n`) so Maildir/IMAP messages with unusual `Content-Type` still show a readable body instead of the entire message.
- **HTML body display:** When the detail payload includes HTML, Flutter starts a loopback **HTTPS** server in Rust (`frb_mail_body_set_tls_require_client_cert` → `frb_mail_body_server_init` / `frb_mail_body_register_store` / `frb_mail_body_message_url`) and loads the message in a **`webview_flutter`** widget. The server serves `/view/{storeKey}/{folder}/{messageId}/body` with CSP, optional remote images (`allowRemote` widens `img-src` to `https:`/`http:`), and `cid:` parts via `/cid/…`. **TLS / mTLS:** Rust defaults to **requiring a client certificate** (`set_mail_body_tls_require_client_cert(true)`). The Flutter app calls **`frb_mail_body_set_tls_require_client_cert(false)`** at startup so the WebView can connect without presenting the ephemeral client cert (TLS to loopback remains). Set **`TAGLIACARTE_MAIL_BODY_TLS_NO_CLIENT_CERT=1`** on the process to force the same from the environment. Init JSON includes **`enforcesClientCert`**. **`NavigationDelegate.onSslAuthError`** proceeds for the self-signed ephemeral CA. **IMAP → HTTP streaming:** For the primary display part (`BODY.PEEK[section]`), the IMAP client uses a **phased FETCH** (`begin_fetch_body_peek_section` → `read_streaming_literal_chunk` per read → `finish_streaming_fetch`) so the mail-body handler can **interleave** IMAP reads with **HTTP/1.1 chunked** writes. Bytes flow **CTE decode** (`StreamingCteDecoder`) → **UTF-8 assembly** (`Utf8StreamAssembler`) → **`StreamingCidRewriter`** (only defers output when a `cid:` URL token spans chunk boundaries) → `write_chunk`; **`/cid/…`** image responses use the same phased FETCH with chunked bodies instead of buffering the full part. **Nested `message/rfc822` and full-`BODY[]` fallback** still buffer one message for MIME extraction before responding. Remote HTTP(S) images are blocked by **CSP** when `allowRemote` is off (no server-side `<img>` stripping).
- **Folder management (native mail):** create / rename / delete folder where the store supports it (e.g. IMAP), wired from Flutter.

### 0.5 IMAP message identifiers

- URI form: `imap://{user_at_host}/{mailbox}/{uid}` (see §7).
- **UID parsing** for `UID FETCH` uses the **last** `/`-separated segment as the numeric UID so **mailbox names that contain `/`** remain valid (nested mailboxes).

### 0.6 Configuration persistence

- **XML** (`config.xml`): **single on-disk source** under `<tagliacarte>` (see §12): **stores**, **transports**, **selected-store**, and **application settings** as attributes on `<security>`, `<viewing>`, and `<composing>` (e.g. `use-keychain`, `date-format`, `message-list-sort`, …). Flutter ↔ Rust still passes an `FrbConfig` JSON string over flutter_rust_bridge for load/save; only `config.xml` is written. A legacy **`config.json`** in the same folder is migrated once to XML and removed.

### 0.7 Flutter UI (behavioural summary)

- **Home:** Responsive **compact** (drawer + app bar) vs **wide** (account rail, folder pane, main pane). For **email** stores the main pane is **message list + optional inline detail split**; for **Nostr/Matrix** (`storeKind` conversation mode from session **`AccountConnectionChanged`**) the main pane is **`ChatView`** (same session-backed list provider, chat-style rows + compose stub). **MailToolbar** on desktop. On **macOS**, a **Message** submenu is defined in **`MainMenu.xib`** (alongside the standard Edit / View / Window / Help menus) and calls into Dart via **`MethodChannel` `dev.tagliacarte/mail_menu`** so Flutter’s `PlatformMenuBar` is **not** used (it would replace the entire native menu bar).
- **Actions:** Reply / reply-all / forward / compose require an **outgoing transport** on the account; message-scoped actions (reply family, delete, junk) require a **selected message**. Disabled actions are reflected in the toolbar, overflow menu, and application menu.
- **Message detail:** Separate route on small/narrow layouts; inline pane when “message detail inline” is enabled on desktop. Date formatting in headers follows **View** settings (`date_format` / pattern).
- **Settings:** Multi-tab (accounts, outgoing, security, viewing, …); credential prompts for IMAP when passwords are missing.
- **Compose:** Gated when no transport is configured for the store.

Internationalisation in Flutter uses **ARB / gen-l10n** (`flutter_ui/lib/src/l10n/`). The top-level README may still mention Qt Linguist from the retired Qt UI; the live client is Flutter-only.

### 0.8 Message list: ordering, paging, and store-specific strategy

User-visible **sort** (date / from / subject, asc/desc) must match **real field order**, not assumed mailbox sequence. Strategy by backend:

#### IMAP

1. **If `SORT` is advertised** (RFC 5256): obtain a **sorted UID list** for the folder (criteria mapped from UI sort: e.g. `DATE`, `FROM`, `SUBJECT`, with `REVERSE` as needed). **Paging** is over **ranks in that list** (visible window → slice of UIDs → `UID FETCH` with the existing minimal `HEADER.FIELDS` profile for those UIDs only). Invalidate or refresh the sorted UID list when `UIDVALIDITY` changes or after a deliberate folder resync.
2. **If `SORT` is not advertised:** **Fall back** to a **single pass** that fetches the **minimal header set for all messages** in the folder (same correctness as today; slower on large mailboxes). Client-side sort over that complete summary set. This is acceptable as the compatibility path.

#### Maildir and mbox

Until a **proper local metadata index** exists (SQLite or similar over all messages in the tree/file), behaviour matches the **no-SORT IMAP fallback**: scan/read enough to build **full-folder summaries**, then sort in memory. A future index would enable true lazy paging with `ORDER BY` / keyset queries aligned with UI sort.

#### POP3

Highly constrained: typically **no rich server-side sort**, no folder semantics comparable to IMAP, and often **no stable UID** across sessions the way IMAP UID does. Realistic options are: list messages in **server line order** (often approximate arrival order), optionally **fetch minimal headers for all** when the protocol allows (e.g. TOP or UIDL + per-message fetch) for client sort—**expensive**. Document limitations in UI where needed; do not assume POP3 can match IMAP’s lazy sorted paging without server features.

#### Microsoft Graph (Exchange mailboxes)

The Graph mail API supports **paged listing** (`$top` / `@odata.nextLink`). **Server-side ordering** is controlled by **`$orderby`** on supported properties (exact fields and limits depend on the endpoint version and mailbox type). Plan: use **Graph-native paging** and, when `$orderby` matches the user’s sort, **lazy pages**; if the API cannot express the requested sort, fall back to **fetching a larger window or full set** for client sort (with the same trade-offs as no-SORT IMAP). **Investigate** per-resource docs (`/me/messages`, shared mailboxes, etc.) when implementing.

#### Gmail

Access is **IMAP with XOAUTH2** (no separate REST Gmail API in current plans). Rely on the same rules as **IMAP** above: use **`SORT` when present** for lazy sorted paging; otherwise full minimal-header pass. Google’s IMAP implementation is generally capable enough for this model.

---

## 1. Design principles

1. **One abstraction**: All message sources (email, Nostr DMs, Matrix) implement the same `Store`, `Folder`, `Transport` traits. The UI and FFI are agnostic to the backend; backend-specific behaviour is behind the abstraction.

2. **Folder, conversation, and channel**: We use these terms interchangeably for the same concept (like a channel in Slack or Discord).
   - **Email**: A *folder* (mailbox: INBOX, Sent, …) contains many *messages*. Messages can be presented as a **flat list** or grouped into **threads** (email-specific: subject + References/In-Reply-To). So for email, a folder is the mailbox; **threads** exist only within email folders.
   - **Nostr / Matrix**: A *folder* is one DM conversation or one room—i.e. one folder per contact (Nostr) or per room (Matrix). The folder *is* the conversation/channel; it only has messages. So “list folders” = list of conversations/channels (contacts or rooms).

3. **Semantic send/receive**: The frontend deals only with **structured data**. It does not construct or parse MIME, Nostr JSON, or Matrix wire formats. Send: UI sends typed fields (from, to, subject, body, attachments); the backend produces the wire format. Receive: backend delivers message content as typed data (from, to, date, subject, body_plain, body_html, attachments); optionally raw (e.g. RFC 822) for “view source”.

4. **Connection reuse**: All network clients (SMTP, IMAP, Nostr relays, Matrix HTTP) use a shared pattern: keep the connection alive, reuse it if still alive, close after an idle timeout or reopen as needed. No “new transport per send”; store and transport handles own long-lived clients.

5. **Event-driven, non-blocking**: The UI never blocks waiting for the backend. Every operation that can block (folder list, message list, get message) follows **request → events → completion**. The UI initiates a request; the backend delivers results asynchronously via events; the UI reacts to those events. See §2 (principle), §3 (event model), and §4 (FFI).

---

## 2. Push/event-driven principle

The same principle applies from the smallest data unit (network packets) up to the whole application.

### 2.1 Core rule

**Push data; react to events. Never block when there is any chance of nontrivial latency.**

- The caller **never** blocks waiting for "the full result." The API returns immediately (or as soon as the request is sent). Progress and results are delivered later via **events** and a **completion** callback.
- Data flows **forward**: you push bytes (or send a request), and the system reacts. When there is enough data to form a **complete token** (e.g. a JSON key, a line of text, a header), emit a token. When there are enough tokens/events to form a **complete message** (e.g. one folder, one FETCH response, one relay message), emit that message. Granularity is up to the layer: as fine as individual tokens or as coarse as full application-level messages, but **never** wait for "everything" when you can emit something useful earlier.
- If the connection **pauses** (packets stop arriving for a while), nothing hangs. No thread blocks on read. When more data arrives, push it in and continue emitting events. The application simply reacts to whatever happens, whenever it happens.

### 2.2 Scale: from bytes to application

This principle applies at every layer:

- **Network / I/O**: Read whatever is available; don't block for "a full message" if the protocol allows processing partial input (e.g. line-by-line, or chunked).
- **Parsing**: Push bytes into the parser as they arrive; the parser emits **tokens** or **events** as soon as a complete unit is recognized (e.g. JSON `startObject`, `key`, `stringValue`; or IMAP `* LIST` line). Incomplete units stay in a small buffer; the caller feeds more data when it arrives.
- **Protocol**: Send a command (e.g. SELECT, FETCH); return to the caller immediately; for each response item (e.g. each untagged line, each FETCH block), emit an event; when the response is done, emit a completion/error (e.g. "end select", "end fetch").
- **Application**: Start an operation (e.g. open folder, request message list); return immediately; deliver folder/message events and a final on_complete/on_error from a worker. The UI never blocks; it only reacts to callbacks.

At every layer, **data drives the pipeline**. You never sit blocking on "the rest of the message" when you could be emitting tokens or events and letting the next layer (or the app) react.

### 2.3 Reference implementations (same principle elsewhere)

The same pattern appears in other projects and should be mirrored in Tagliacarte:

- **Plume (../plume)**: Nostr relay WebSocket. JSON arrives in frames. Plume uses a push-style JSON parser (Actson): push frame bytes, pull JSON events in a loop. When enough events form a complete relay message (EVENT, EOSE, NOTICE, OK), that message is sent to the UI via a channel. The async loop uses a read with timeout—if the connection pauses, the loop gets a timeout and continues; no blocking. So: **bytes → JSON events → relay messages → UI**.

- **jsonparser (../jsonparser)**: Java JSON parser with a **push model** via `receive(ByteBuffer)`. The caller pushes chunks as they arrive (e.g. from a socket). The parser keeps state and emits **content-handler events** (e.g. `startObject()`, `key(…)`, `stringValue(…)`) as soon as a **complete token** is recognized. If a token spans chunk boundaries, unconsumed bytes stay in the buffer; the caller compacts, reads more, and calls `receive` again. So: **chunks → tokens → events**; constant memory; no blocking on "full document."

- **Gonzalez (../gonzalez)**: XML parser with the same idea. `receive(ByteBuffer)`; state machine processes what's available; incomplete tokens are buffered; control returns to the caller. **Bytes → tokens → SAX events.** Data-driven: processes whatever is available.

In all three: **data drives the pipeline; the application never blocks waiting for the entire payload when it can react to tokens or events.**

### 2.4 What this means for Tagliacarte

- **IMAP SELECT**: Send SELECT, **return immediately**. Read the server response line-by-line. For each untagged line that represents a SELECT response item (EXISTS, RECENT, FLAGS, UIDVALIDITY, UIDNEXT, etc.), **emit an event**. When the tagged OK (or error) is seen, emit **end of SELECT** and call completion. The **caller** of "open folder" must not block: the open-folder API returns right away; SELECT runs in a **background task**; events and completion are delivered via callbacks from that task.

- **IMAP FETCH (message list)**: Send FETCH, **return immediately**. For each `* FETCH` response, parse the summary and **emit one event**. When the tagged response is seen, emit **end of FETCH** and call completion. The API that "requests message list" returns immediately; FETCH runs in a background task; events and completion come from that task.

- **IMAP FETCH (single message body)**: Send UID FETCH BODY[], **return immediately**. Stream the literal body in **chunks**. Optionally buffer until the first `\r\n\r\n` to parse headers and call metadata callback; then call content_chunk for each chunk. When the literal is fully read, call completion. No blocking until "full message in memory."

- **Folder list**: Send LIST, return immediately; for each `* LIST` line, emit one folder event; when done, completion. The Store API that starts "refresh folders" **returns immediately** and the LIST work runs in a background task, with callbacks from that task.

- **Send (SMTP etc.)**: Start send (metadata, body chunks, attachments) and return immediately; completion is reported via a callback. No blocking the UI on "send finished."

### 2.5 Implementation checklist for conformance

When adding or changing an operation, ensure:

1. **API returns immediately**: The function that starts the operation returns as soon as the request is sent (or the task is spawned). It does **not** wait for the full response.
2. **Work runs in a background task**: The actual I/O and parsing run in a spawned task or thread. The main thread / FFI caller is not blocked.
3. **Events are delivered as data arrives**: For each **complete unit** (one SELECT line, one FETCH response, one body chunk), invoke the appropriate callback. Do not collect all units and then iterate; emit as you parse.
4. **Completion is a callback**: When the operation finishes (success or error), call on_complete/on_error from the background task. The UI marshals to the main thread if needed.
5. **No "batch then emit" at the protocol layer**: Avoid: read entire response into memory, then loop and call callbacks. Prefer: read line/chunk → parse → emit event → repeat. Only buffer the minimum needed for parsing (e.g. until `\r\n\r\n` for header/body split).
6. **Connection pauses are acceptable**: If the underlying read would block (e.g. no data yet), the design must allow the event loop / task to yield so that nothing hangs. The application keeps reacting when data appears.

---

## 3. Event-driven, non-blocking model

### 3.1 Pattern

- **Request**: UI calls a “start” API (e.g. refresh folder list, request message list, request message by id). The call returns **immediately**; the UI does not block.
- **Events trickle back**: The backend emits events as results become available—e.g. one event per folder, one per message summary, then metadata then content for a single message.
- **Completion**: A final event signals completion (or error). The UI stops any “loading” state and, if needed, reconciles (e.g. remove folders that no longer exist).
- **UI role**: The UI only **initiates** requests and **reacts** to events. It never blocks on the backend. When events are delivered from a background thread, the UI must marshal to the main thread before updating widgets.

### 3.2 Folder list

- **Start**: e.g. `refresh_folders(store)` — returns immediately.
- **Events**: `folder_found(FolderInfo)` for each folder; optionally `folder_removed(name)` if the backend can detect removals (e.g. IMAP LIST diff). Events trickle as the backend discovers folders.
- **Completion**: `folder_list_complete()` or `folder_list_error(err)`.
- **UI**: Add new folders to the list as `folder_found` arrives; remove folders on `folder_removed` (if supported); on completion, stop loading and reconcile.

### 3.3 Message list (folder / conversation / channel)

- **Start**: e.g. `request_message_list(folder, start, end)` — returns immediately.
- **Events**: `message_summary(...)` for each item, in order; they trickle as available.
- **Completion**: `message_list_complete()` or `message_list_error(err)`.
- **UI**: Append rows to the list as events arrive; on completion, stop loading.

### 3.4 Get message (message view pane)

- **Start**: e.g. `request_message(folder, message_id)` — returns immediately.
- **Events**: `message_metadata(envelope)` when envelope is ready; then `message_content(body_plain, body_html, attachments)` when body is ready (or a single `message_ready(full_message)` if the backend prefers). This lets the UI show “loading…” then envelope, then body.
- **Completion**: `message_complete()` or `message_error(err)`.
- **UI**: Update the message view pane in response to each event; no blocking.

### 3.5 Send

- Send can remain request/response (or callback on done). The UI only needs “sent” or “error”; no streaming. It can be asynchronous (e.g. completion callback) so the UI does not block.

### 3.6 Backend implementation

- Backend (Rust) uses async (e.g. tokio). Each “start” API spawns a task that does the work and sends events over a channel or invokes registered callbacks. The public API is event-based: register callbacks or subscribe to an event stream, then call “start”. Synchronous “block until done” APIs can remain for tests or simple tools but are **not** the primary path for the UI.

---

## 4. FFI: exposing the event model

**Implemented (Flutter):** See **§0** — interop is **flutter_rust_bridge**, not C callbacks. The **core** still uses the callback pattern internally; **frb_mail** aggregates into JSON-returning calls. A future C/FFI or streaming Dart API could expose the finer-grained event model below.

**Target (e.g. C or native toolkits):**

- **Explicit callbacks**: We use **explicit callbacks per operation type**, not a generic `on_event` sink. The FFI registers distinct callbacks for each kind of event, so the C/UI side has a clear, typed contract and no need to decode a generic event enum or payload.
  - **Folder list**: e.g. `on_folder_found(FolderInfo)`, `on_folder_removed(name)`, `on_complete()` / `on_error(err)`; each with `user_data`.
  - **Message list**: e.g. `on_message_summary(MessageSummary)`, `on_complete()` / `on_error(err)`; with `user_data`.
  - **Get message**: e.g. `on_metadata(envelope)`, `on_content(body_plain, body_html, attachments)`, `on_complete()` / `on_error(err)`; with `user_data`.
- **Start calls**: e.g. `tagliacarte_store_refresh_folders(store)`, `tagliacarte_folder_request_message_list(folder, start, end)`, `tagliacarte_folder_request_message(folder, message_id)`. All return immediately; the registered callbacks are invoked from a backend thread when events occur.
- **Thread safety**: Events may be delivered from a background thread. The UI is responsible for marshalling to the main thread (e.g. signal or post to main queue) before touching UI state.

---

## 5. Semantic send and receive

### 5.1 Send

- **Payload**: Structured only. Fields: from, to (list), cc, subject, body_plain, body_html, attachments (list of blob + filename + content_type). No raw MIME or JSON from the UI.
- **Backend**: Each transport (SMTP, Nostr, Matrix) builds its wire format from the payload. `Transport::send(payload)` in core; FFI exposes one send API with structured parameters.

### 5.2 Receive

- **Structured content**: Message content is always delivered as typed data: envelope (from, to, date, subject) + body_plain, body_html, attachments[]. Optionally raw (e.g. RFC 822) for “view source”.
- **Events**: When the UI requests a message, events carry this structured content (e.g. metadata event, then content event). The UI never parses MIME or Nostr/Matrix formats.
- **Implemented (FRB path):** Raw bytes are parsed in Rust with **`extract_structured_body`**; the Flutter detail view consumes JSON fields. If no `text/plain` or `text/html` part is extracted, **`utf8_body_after_rfc822_headers`** supplies a sensible plain-text fallback (content after the first header/body blank line) before falling back to the full raw string.

---

## 6. Connection reuse

- **Goal**: One persistent client per store/transport where applicable. Keep connection alive, reuse if still alive, close after idle timeout or reopen as needed.
- **Applies to**: SMTP, IMAP, Nostr relay connections, Matrix HTTP client.
- **Behaviour**: Configurable idle timeout; on next use after timeout, reconnect transparently. FFI store/transport handles own the client; no “new transport per send”.

---

## 7. MessageId and identifier schemes

- **Type**: `MessageId` remains an opaque string. Schemes are used consistently so backends and UI can interpret them when needed.
- **Email / host-based**: URI form where the authority is a host: `imap://user@host/mailbox/uid`, `maildir://...`, `mbox://...`, `matrix://host/room_id/event_id`.
- **IMAP (implemented):** The mailbox segment may contain `/` (nested folders). Parsers must take the **UID as the last path segment** after `imap://`, not a fixed `splitn(3, '/')`.
- **Nostr**: Do **not** use `nostr://...` because Nostr events are not tied to a network host; `//` in URLs is for identifying hosts. Use:
  - **`nostr:nevent:...`** (or equivalent) for a single event.
  - **`nostr:dm:<our_pubkey>:<other_pubkey>`** (or similar) for a folder/conversation id.
- **Matrix**: `matrix:room_id:event_id`; folder/conversation id is the room (e.g. room_id).

---

## 8. Unified folder/message API (by backend type)

- **All folders** (conversations/channels): Support `list_messages(folder, range)` and `message_count(folder)`. Exposed via events (message_summary events, then message_list_complete).
- **Email only**: Additionally support **thread** APIs: `list_threads(folder, range)` and `list_messages_in_thread(folder, thread_id, range)`. Threading is email-specific (subject + References/In-Reply-To). The UI can offer a view toggle: “flat messages” vs “by thread”.
- **Nostr/Matrix**: Folder = conversation = channel. No threads; only `list_messages`. The folder list in the UI is the list of conversations/channels (contacts or rooms).

---

## 9. Store and transport kind (for UI)

- The UI needs to know store/transport type to show the right compose form and view options (e.g. Subject only for email; To = pubkey for Nostr, room/MXID for Matrix; thread vs flat view is email-specific).
- Expose `store_kind` and `transport_kind` (e.g. enum: email, nostr, matrix) via traits and FFI.

---

## 10. Nostr and Matrix (extension)

- **Nostr**: Store identity = key pair + relay list. `list_folders()` = one folder (conversation/channel) per DM contact. Each folder is that conversation; `list_messages` returns DMs with that contact. MessageIds: `nostr:nevent:...` for events, `nostr:dm:...` for folder id. Connection reuse for relay WebSockets; semantic send (build kind-4 from payload); event-driven folder/message events.
- **Matrix**: Store identity = homeserver + user + access token. `list_folders()` = list of rooms (one folder per room). Each folder is one conversation/channel (one room, possibly many senders). MessageIds: `matrix://...` for events; room_id for folder. Connection reuse for HTTP; semantic send; event-driven.
- **Security**: Nostr keys: do not log or expose; decrypt only in core. Matrix: token refresh / re-login when needed.

---

## 11. UI behaviour (summary)

- **Folder list**: Initiate refresh → react to folder_found / folder_removed → completion. Left pane shows folders (conversations/channels): mailboxes for email, or contacts/rooms for Nostr/Matrix.
- **Message list**: Open folder → request message list → react to message_summary events → completion. Email only: optional toggle to “by thread” and request thread list then messages in thread.
- **Message view**: User selects message → request message by id → react to metadata then content then complete. No blocking.
- **Compose**: Collect structured fields only; single send API; backend builds wire format. Transport tied to current account (reused connection).

**Flutter (implemented):** Riverpod providers drive folder list, message list, and message detail (`FutureProvider` / `family` keyed by store + folder + id). **Sort order** is user-configurable and stored as **`message_list_sort`** (symbolic string, app-wide). **How sort interacts with paging** (IMAP `SORT` vs full-folder fetch, other stores) is defined in **§0.8**. **Toolbar and menus** disable message actions when nothing is selected and disable send-related actions when the account has no usable outgoing transport. **View** preferences (e.g. date format) apply to rendered headers. Folder tree supports **hierarchy delimiter** when the server reports one (IMAP).

---

## 12. Configuration files

- **Authoritative file**: `config.xml` under the tagliacarte config directory uses root element **`<tagliacarte>`**. **Stores** and **transports** use **attributes** on `store` and `transport` elements (not `<param>` for scalar data in the new format). **UI preferences** from `FrbConfig` are written as attributes on **`security`**, **`viewing`**, and **`composing`** (e.g. `use-keychain`, `resource-policy`, `load-remote-images`, `message-list-sort`). **IMAP delete mode and trash folder** are per-account store attributes (`imap-delete-mode`, `imap-trash-folder-name` on `<store>`), not global prefs. On save, unknown attributes on those three elements are **preserved** so hand-edited keys are not dropped. Legacy **`<config><store …/>`** files (with `id` often equal to a full connection URI) are still read and mapped into the same in-memory model.
- **Parser**: `tagliacarte_core::tagliacarte_config_xml` uses **quick-xml** (event-based). See `core/src/tagliacarte_config_xml.rs` for `load_tagliacarte_config` / `write_tagliacarte_config`.

### 12.1 Example `config.xml` (`<tagliacarte>`)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<tagliacarte>
  <selected-store id="s1"/>
  <security tls-verify="strict" password-storage="keychain"/>
  <viewing date-format="iso"/>
  <composing wrap="plain"/>
  <transports>
    <transport id="t1" type="smtp" display-name="SMTP" host="smtp.example.com"
               port="587" security="starttls"/>
  </transports>
  <stores>
    <store id="s1" type="imap" display-name="Work" username="user@example.com"
           host="imap.example.com" port="993" security="tls">
      <transport ref="t1"/>
    </store>
    <store id="s2" type="maildir" display-name="Local" path="/home/me/Mail"/>
  </stores>
</tagliacarte>
```

- **`<transports>`**: Flat list of empty **`transport`** elements; attributes hold host, port, security, etc.
- **`<stores>`**: Each **`store`** uses attributes per type; **only** allowed child elements are **`transport ref="tN"`** (empty elements). **Document order** of those children is outbound priority (first = default transport).
- **Preserved blocks**: **`selected-store`**, **`security`**, **`viewing`**, **`composing`** keep these names; values use **symbolic strings** (not numeric enum codes) in new files.

### 12.2 Token glossary (symbolic strings)

| Token | Meaning |
|--------|---------|
| **store `type`** | `imap`, `pop3`, `maildir`, `mbox`, `nostr`, `matrix`, `graph`, `gmail`, … |
| **transport `type`** | `smtp` (extend later as needed) |
| **`security` (IMAP/POP3/SMTP)** | `tls` — implicit TLS on the usual port (e.g. 993/995/465); `starttls` — cleartext connect then upgrade; `plain` — no TLS (discouraged); `implicit_tls` — alias for implicit TLS where clarity is needed |
| **`password-storage` (example)** | `keychain`, `file`, … (app-defined; hand-editable symbols) |
| **`date-format` (example)** | `iso`, `locale`, … |
| **`message-list-sort` (`viewing` / FrbConfig)** | Symbolic sort mode for the message list, app-wide: e.g. `date_desc`, `date_asc`, `from_asc`, `from_desc`, `subject_asc`, `subject_desc`. Persisted as the `message-list-sort` attribute on `<viewing>`; not a numeric enum. |

Internal Rust/Dart maps these symbols to enums at parse time. Unknown symbols should produce a clear error (with a safe default for sort where the UI falls back to `date_desc`).

This document should be updated when we make material architectural decisions or add new backends.
