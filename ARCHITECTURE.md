# Tagliacarte architecture

This document captures the architectural strategy for Tagliacarte so we can refer back to it during implementation. It covers the core abstractions, the event-driven non-blocking model, folder/conversation/channel semantics, email-specific threads, semantic send/receive, connection reuse, and how Nostr and Matrix fit in.

**Terminology:** We use **folder**, **conversation**, and **channel** interchangeably. They all refer to the same concept: a container of messages (e.g. an email mailbox, a Slack or Discord channel, a Nostr DM with one contact, a Matrix room). **Threads** are email-specific: a thread groups messages within an email folder by subject + References/In-Reply-To. We do not call email threads “conversations”.

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

- **Explicit callbacks**: We use **explicit callbacks per operation type**, not a generic `on_event` sink. The FFI registers distinct callbacks for each kind of event, so the C/UI side has a clear, typed contract and no need to decode a generic event enum or payload.
  - **Folder list**: e.g. `on_folder_found(FolderInfo)`, `on_folder_removed(name)`, `on_complete()` / `on_error(err)`; each with `user_data`.
  - **Message list**: e.g. `on_message_summary(MessageSummary)`, `on_complete()` / `on_error(err)`; with `user_data`.
  - **Get message**: e.g. `on_metadata(envelope)`, `on_content(body_plain, body_html, attachments)`, `on_complete()` / `on_error(err)`; with `user_data`.
- **Start calls**: e.g. `tagliacarte_store_refresh_folders(store)`, `tagliacarte_folder_request_message_list(folder, start, end)`, `tagliacarte_folder_request_message(folder, message_id)`. All return immediately; the registered callbacks are invoked from a backend thread when events occur.
- **Thread safety**: Events may be delivered from a background thread. The UI is responsible for marshalling to the main thread (e.g. Qt signal or post to main queue) before touching UI state.

---

## 5. Semantic send and receive

### 5.1 Send

- **Payload**: Structured only. Fields: from, to (list), cc, subject, body_plain, body_html, attachments (list of blob + filename + content_type). No raw MIME or JSON from the UI.
- **Backend**: Each transport (SMTP, Nostr, Matrix) builds its wire format from the payload. `Transport::send(payload)` in core; FFI exposes one send API with structured parameters.

### 5.2 Receive

- **Structured content**: Message content is always delivered as typed data: envelope (from, to, date, subject) + body_plain, body_html, attachments[]. Optionally raw (e.g. RFC 822) for “view source”.
- **Events**: When the UI requests a message, events carry this structured content (e.g. metadata event, then content event). The UI never parses MIME or Nostr/Matrix formats.

---

## 6. Connection reuse

- **Goal**: One persistent client per store/transport where applicable. Keep connection alive, reuse if still alive, close after idle timeout or reopen as needed.
- **Applies to**: SMTP, IMAP, Nostr relay connections, Matrix HTTP client.
- **Behaviour**: Configurable idle timeout; on next use after timeout, reconnect transparently. FFI store/transport handles own the client; no “new transport per send”.

---

## 7. MessageId and identifier schemes

- **Type**: `MessageId` remains an opaque string. Schemes are used consistently so backends and UI can interpret them when needed.
- **Email / host-based**: URI form where the authority is a host: `imap://user@host/mailbox/uid`, `maildir://...`, `mbox://...`, `matrix://host/room_id/event_id`.
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

---

## 12. Configuration files

- **Format**: Use **XML** for all configuration files, not JSON. This applies to app settings, account/store/transport config, and any other on-disk configuration. (JSON remains the right format for wire protocols such as Nostr or Matrix; the push/event JSON parser work is for those streams, not for config.)
- **Parser**: Prefer a **SAX/Expat-style event-based** XML parser to keep the dependency lean. For configuration files we do **not** require full push parsing from beginning to end (e.g. Gonzalez-style `receive(ByteBuffer)` with control returning to the caller on incomplete input). Config files are small and read from the filesystem, so a conventional pull/blocking SAX API (e.g. read file, then parse and receive events) is acceptable here. The important part is event-based parsing (no DOM) to avoid pulling the whole document into memory and to keep a consistent style with the rest of the stack.
- **Schema**: The exact schema for configuration files can be decided later if we run into issues. No need to lock it in upfront.

This document should be updated when we make material architectural decisions or add new backends.
