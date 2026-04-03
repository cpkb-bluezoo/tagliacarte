# Tagliacarte Flutter UI — design decisions

## Account strip (left rail)

Circular account buttons are the primary way to switch between configured accounts. This section records rules for their appearance so behaviour stays consistent as backends (Nostr, Matrix, mail, etc.) evolve.

### Colour (auto-generated avatars only)

- Each account gets a **distinct** background colour, stable for that account (derived from account id so it does not flicker between sessions).
- Colours are **web-safe** (216-colour style `#RRGGBB` values) and **muted**.
- **Light** system theme: use **lighter** muted fills with **darker** label text for contrast.
- **Dark** system theme: use **darker** muted fills with **lighter** label text for contrast.
- The active (selected) account is still indicated separately (e.g. ring using the theme primary colour); the per-account colour remains visible or only slightly subdued when selected, as implemented in code.

### Avatar image vs auto-generated glyph

- If an **avatar image** is configured for the account (HTTP/HTTPS URL, or on desktop a local file path that exists), it **replaces** both the **background colour** and the **initials text** for that strip entry.
- Protocol-specific avatars (e.g. Nostr/Matrix profile images) are supplied through the same **avatar URL (or path) field** on the account when the backend provides one; there is no separate visual path in the strip.
- If loading the image fails (network error, missing file), fall back to the auto-generated colour + initials rules below.

### Initials text (auto-generated only)

The strip label is **not** the account id or backend type; it is derived from **display name** and **email** as follows.

- **Display name** in configuration is the **Account name** field (`AppAccount.label`). Capitalisation in that string is **preserved** in the initials (we do not force upper/lower case).
- **Whitespace** between words is normalised (any run of spaces/tabs splits words); empty tokens are ignored. Hyphenated or punctuated tokens without spaces count as a **single** word (e.g. `mary-jane` → one word → first grapheme only).
- If there is **exactly one** word in the display name: show the **first character** (first Unicode extended grapheme cluster) of that word.
- If there is **more than one** word: show the first character of the **first** word and the first character of the **last** word, concatenated (e.g. `Chris Burdess` → `CB`, `mary-jane Watson` → `mW` — first token `mary-jane`, last token `Watson`).
- If the display name is **missing or empty** after trim: fall back to **email** (`AppAccount.email`, from username/email in settings). Use the **first character** (first grapheme) of the email string.
- If both display name and email are unusable: show `?`.

### Tooltips

- The tooltip for an entry should identify the account for the user (typically the same **Account name** / label, optionally with backend type in settings lists only; strip tooltips use the label).

## Failed authentication — credential prompt (all stores and transports)

This applies to **any** operation that requires authentication to a **store** (receive/sync) or, once modeled, an **outbound transport** (e.g. SMTP): not only “password missing in the vault”, but also **wrong or stale credentials** (password changed on the server, revoked token, etc.) whenever the backend can classify the failure as fixable by re-entering secrets.

### Product rules

- **Modal dialog** — The user must be prompted in a **modal** (not only a SnackBar) so the problem cannot be missed and the flow is explicit.
- **Always two explicit outcomes:**
  - **Proceed** (e.g. **Save** / **Sign in**) — validate input, persist credentials for the correct **credential key** (today often a store URI; later also **per-transport** keys such as an SMTP URL), invalidate caches / retry the failed operation.
  - **Cancel** — Close the dialog **without** saving; the **store (or transport) remains unusable** for that session until the user tries again or fixes configuration: no fake “open” state, no silent retry with old secrets.
- **`barrierDismissible: false`** — Dismissing by tapping outside must **not** count as success; the user must choose **Cancel** or **Proceed** (same pattern as the current IMAP dialog).
- **Context in the dialog** — Show **what** is being authenticated using **human-readable labels** (account display name, transport display name, short host summary such as `mail.example.com`). **Avoid** leading with full store/transport URIs in the main dialog body — they add clutter for typical users. Canonical URIs / ids belong in **settings detail**, a **“Technical details”** expander, or debug/support tooling only.

### Implementation status

- **Today:** IMAP is partially implemented (string match on “no saved password” and a store-URI–keyed save). **Generalization is required:** map other backends’ errors to the same modal pattern, use **transport-scoped** credential keys when sending mail, and treat **auth failure** (not only “missing”) as prompt-worthy where the protocol allows distinguishing it from network/server errors.

---

## IMAP — current credential dialog (reference implementation)

The following describes the **current** IMAP-specific UI; it should evolve into the generic **failed authentication** dialog (copy, title, and credential key differ per backend).

### When the dialog appears (IMAP today)

1. **Selecting an IMAP account** — if listing folders fails for that reason, show the dialog; on successful **Save**, retry the folder list (loop until success, cancel, or a non–missing-password error).
2. **Message list** — `ref.listen` on the folder’s message provider: if the error matches the known “no saved password” text, show the dialog; on Save, **invalidate** the folder messages provider (and the current message detail provider when applicable) so data reloads without an app restart.
3. **Message body / detail** — same pattern on the message-detail provider, but **only when the detail pane is inline on the home layout** (desktop split view). When the user opens a message on a **separate route** (e.g. mobile `StoreMessageDetailScreen`), that screen registers its **own** listener. This split avoids **two** credential dialogs firing for the same failure.

### Dialog layout and behaviour (`imap_credential_dialog.dart`)

- **Presentation:** `AlertDialog`, title **“IMAP sign-in”**, **`barrierDismissible: false`** (tap-outside does not close; user must **Cancel** or **Save**).
- **Context line (current):** Full **store URI** under the title — **interim** for IMAP only; the generic credential modal should prefer **account label + short summary** per the rules above, not the raw URI, unless the user opens technical details.
- **Fields:** **Username** then **Password**, outline `TextField`s in a **`SingleChildScrollView`** + `Column` (`mainAxisSize: min`, stretch width) so small viewports still scroll.
- **Username prefill:** If the URI is `imap://` or `imaps://` and has a **userinfo** fragment, prefill from the user part (strip a `:password` segment if present); apply **`Uri.decodeComponent`** so percent-encoded `@` in the authority becomes a real **`@`** in the field.
- **Password:** Obscured by default; **suffix** visibility toggle (eye icons) with tooltips **“Show password”** / **“Hide password”**. Password text is **not** trimmed for save (only the username is trimmed for the non-empty validation check).
- **Actions:** **Cancel** (`TextButton`) pops `false`; **Save** (`FilledButton`) runs the Rust save call, shows a small **progress indicator** on the button while busy, pops `true` on success. Validation errors (**empty user/password**) and save failures use a **SnackBar** on the dialog’s context.
- **Persistence:** Save uses **`frbSaveStoreCredential`** with the same **`useKeychain`** flag as the rest of the session (from loaded app settings).

### Copy / strings

- Dialog title and field labels are currently **English literals** in code (`'IMAP sign-in'`, `'Username'`, `'Password'`, `'Save'`, `'Cancel'`, validation SnackBar). When l10n is extended to this flow, these should move into ARB like other user-facing strings.

### Related implementation notes (not layout)

- The native side reuses **one `Store` per `(storeUri, useKeychain)`** for the Flutter bridge so a single screen load does not open multiple IMAP connections (and repeat **AUTHENTICATE PLAIN**) for folders, list, and detail. The cache is **invalidated** when credentials are saved so the next open picks up the new password. **URL username** for IMAP auth is **percent-decoded** in Rust so it matches what the server expects when the store URI was built with encoded userinfo (e.g. `user%40host`).

---

## Stores vs outbound transports (configuration direction)

Not all “accounts” are alike: some protocols bundle **identity + send + sync** in one connection (e.g. Nostr, Matrix, Graph, NNTP); **mail reception** often uses a **store** (IMAP, POP3, Maildir, mbox) while **sending** uses a separate **SMTP** (or similar) transport. Several stores may legitimately share **one** SMTP endpoint and **one** saved password; duplicating SMTP credentials per store is wrong.

### Target model (conceptual)

- **Transports** — A reusable list of outbound definitions (at least **SMTP** initially), each with a stable **XML id** (`t1`, `t2`, …), connection parameters (host, port, security), **display name**, and **credentials keyed on that id** (vault / keychain row per transport id).
- **Stores** — Reception/sync definitions (IMAP, Maildir, …), each with stable **XML id** (`s1`, `s2`, …); **store credentials** (where applicable) are keyed on **store id**, not on a derived URL string alone.
- **Linkage** — Each store that needs SMTP references **one or more** transports (by ref). Compose-time rules may later allow choosing among allowed transports (user choice, mailbox, message rules).

Two config shapes were discussed:

1. **URL-heavy** — `<transport url="smtps://…"/>` and `<store>…<transport url="…"/></store>`. Simple but **changing port/TLS** rewrites URLs and risks treating the same logical server as a different credential bucket unless normalized carefully.
2. **ID + structured fields** — `<transport id="t1" type="smtp" host="…" port="…" security="starttls" …/>` and `<transport ref="t1"/>`. Same logical transport can be edited (e.g. 465 → 587) **without** a new identity if **credential key** is defined as “human server + auth identity”, not the full URL string.

**Recommendation:** Prefer **stable ids** for file references plus **structured fields** (or a **normalized canonical URL** computed at load time) for credential storage, so refactors of port/security do not orphan keychain entries.

### OAuth, passwords, and “username on the transport”

- Separate **transport endpoint** (host, port, TLS mode, which SASL family applies) from **credentials** stored in the vault.
- **Password / app-password SMTP** — The server still needs an **auth identity** (login or email) plus a **secret**. That pair is naturally a **credential record** (type: password) attached to the transport (or referenced by id). The transport definition does not have to duplicate “username” as a long-lived property if the credential type always carries **identity + secret**; in the **settings UI** it is fine to show one form (host + port + username + password) for ergonomics, while the data model keeps secrets out of the transport row.
- **OAuth2 (e.g. SMTP XOAUTH2)** — Same idea: credential type is **OAuth** (tokens, refresh, provider metadata). There is still almost always an **account identifier** the provider expects (typically the mailbox **email** as `authcid`), plus token material — not “no username”, but **username is not a transport property**; it lives with the OAuth credential bundle. The transport row might only record `mechanism=oauth2`, host, and optional provider id.

### Sender identities (From / Reply-To) vs transport

- **Associate sender identities with the mail account (store)** — the mailbox personality the user selected in the strip: a set of **From** (display name + address) and optional **Reply-To**, with one **default** for compose. Users think in terms of “this inbox / this account”, not “this SMTP socket”.
- **Transport enforces policy** — Some servers require **From** to match the authenticated user; others allow the whole domain or aliases. That is **server rules**, not something to duplicate as fake “transport usernames”. At send time: **validate** the chosen identity against the chosen transport (and surface a clear error if the server rejects spoofing).
- **Advanced** — Later: per-identity default transport, or multiple allowed transports per identity, if real users need different submission paths for different From addresses.

### Problems and edge cases to plan for

- **Credential identity** — **Vault keys follow XML ids** (`t1`, `s1`, …). Editing host/port on a transport does **not** create a new secret bucket as long as the id is unchanged. Changing auth identity (e.g. login email) updates the credential payload for that same id (pre-alpha: no migration story required).
- **Deleting a transport** — Stores keep an **ordered list** of transport ids. When a transport is **deleted**, that id is **removed from every store’s list** (app layer updates config in one transaction). The **first remaining** transport becomes the new default; if the list is **empty**, the store has **no** outbound transport. This is **not an error** and needs no “broken ref” banner: for store types that **require** an external transport to send, an **empty list** simply means **sending is unavailable** (compose, reply, forward, etc. **disabled** for that store). The store detail UI for those types should show **explanatory copy** (e.g. that at least one transport must be selected in the list to send mail). Optional: confirm dialog when deleting a transport (“Will be removed from N account(s)”) before delete.
- **Multiple transports per store** — Compose UI needs a **default** + optional overrides; rules engine is a later phase.
- **Store types with built-in send** — Matrix/Nostr/etc. have **no** SMTP row; settings UI must not imply every store needs a transport link.
- **Duplicate display names** — “Mailhost” twice; ids (`t1`, `t2`) are authoritative in config, labels for humans.
- **DTD/validation** — XML won’t enforce IDREF without a schema; app code should validate refs on load and in the settings editor.

### Settings editor — two tabs (Accounts vs Outgoing)

Settings uses **two tabs**:

1. **Accounts** — edits **stores** internally (`s1`, `s2`, … in XML).
2. **Outgoing** — edits **transports** internally (`t1`, `t2`, … in XML).

**Shared chrome (both tabs)**

- **Left:** A **vertical strip** of existing providers (same *idea* as the main window / drawer account strip: primary navigation within the tab).
- **Bottom of the strip** (where the main UI puts **Settings**): an **Add** button only — so **Add** is visually separated from selecting existing providers (no accidental conflation with “pick another row”).
- **Right:** A **detail pane** for the selected or newly added provider.
- **Dirty state:** The **Save** button is **muted / visually disabled** when the detail pane matches **last saved** state (**clean**), and **enabled / emphasised** when anything has changed (**dirty**). Same rule on **both** tabs. A **new** provider not yet saved is **dirty** (Save enabled). (Optionally disable the control while save is in flight.)

**Accounts tab (stores)**

- Selecting a strip entry shows that store’s **detail pane**.
- **Top field:** **Store type** — `DropdownButton` (or equivalent), **order:** IMAP, POP3, Maildir, mbox, Nostr, Matrix, Graph API. Changing type **replaces or resets** most fields below to match that backend (user may lose inapplicable data; consider confirm-on-change if non-empty).
- **Add:** Opens an **empty-new** detail pane with type **default IMAP**; user can change type before first save.
- **Detail footer:**  
  - **Bottom left:** **Delete** — removes the current store from config; **omit** for a not-yet-saved new store. **Confirm** in a dialog before delete.  
  - **Bottom right:** **Save** — **create** new store (assign new `sN` id) or **update** existing.
- **Stores that need external SMTP:** A dedicated **area** with a **multiselect list** of transports (by display name / summary; values are **transport ids**). Only **existing** transports appear in the list, so there is no “save invalid ref” ordering problem. **Workflow** is free: e.g. **save** a store with **no** transports, **create** a transport on the Outgoing tab, **return** to the store and **add** it to the list. **Default send transport** = **first** selected item in list order. **Move up / down** buttons beside the list to **reorder** (order defines default + fallback sequence for compose). If the list is **empty**, show short **help text** that sending requires at least one transport (no error state; sending actions stay disabled in the main app until configured).
- **Sender identities** (From / Reply-To) remain on the store detail as previously decided; not repeated here field-by-field.

**Outgoing tab (transports)**

- Same pattern: **strip** + **Add** at bottom of strip + **detail pane**.
- **Add** → new empty transport detail (sensible default type, e.g. SMTP).
- **Delete** (existing only, with confirmation) **bottom left** — on confirm, remove the transport from config **and** strip its id from every store’s transport list (see **Deleting a transport** above). **Save** **bottom right**.
- Transport **type** dropdown at top if multiple outbound kinds exist later; fields depend on type.

**Main window account strip (unchanged in role)**

- Still reflects **stores** (same ids as Accounts tab); editing happens in Settings → Accounts.

### Compose and runtime

- **From** = identities on the **store**; **Send using…** = ordered transport list when length > 1.
- **No transports** (empty list) on a store type that needs SMTP: **do not** treat as config error — **disable** compose / reply / forward (and related) for that store until the user adds a transport in Settings.
- Auth errors: **display name + short host**, not raw URIs in the primary message.

### Design risks and conflicts (foreseeable)

| Issue | Mitigation |
|--------|------------|
| **Changing store type** after save | Fields reset; may orphan **store-id** credentials in vault for old type. Pre-alpha: acceptable; later optionally delete stale secrets or warn. |
| **Delete store** while main window has it selected | Invalidate selection; switch to another store or empty state; clear FRB cache for that store id. |
| **Delete transport** | **Remove that id from all store lists** on delete (no dangling refs). Optional confirm: “Removed from N account(s).” Next list item becomes default; empty list ⇒ send disabled for affected stores (by design). |
| **Reorder transports** vs unsaved multiselect | Save commits order; **Save** enabled only while dirty (see **Dirty state** above). |
| **Narrow / mobile** | Strip + detail may need **master–detail** navigation (strip full-screen → push detail); Add stays discoverable (FAB or app bar). |
| **Duplicate strip metaphors** | Three strips (main, Accounts settings, Outgoing settings) — acceptable if labels/context differ; avoid identical icons with no title. |
| **Graph API / OAuth** | Detail pane for those types is heavier (browser sign-in); same layout still works, but **Save** may mean “persist config” while tokens arrive asynchronously. |

This section is **directional** until `config.xml` / JSON and Rust loaders agree on the same model.

---

## Message move / copy (menu tag, folder target, drag-drop)

This records product rules for **moving** and **copying** messages between folders (and stores), plus how they interact with **selection**, **context menus**, and **desktop drag-and-drop**. Implementation will span Flutter (menus, tree, list) and Rust (per-store append / delete / expunge).

### Two ways to choose the source set

1. **Menu / command path** — The user chooses **Move** or **Copy** from the compact overflow menu, the **Message** application menu (desktop with native menu bar), or an equivalent **tablet** overflow when there is no app menu. **Actuating Move or Copy** records a **tagged set**: message ids plus **source store + source folder** (ids alone are not enough across accounts).  
   - **Subsequent selection changes in the list do not change the tagged set.** The tag is replaced only when the user actuates **Move** or **Copy** again (new tag replaces the previous pending move or copy).

2. **Drag-and-drop path (desktop, same store only)** — There is **no separate tagged buffer** for this path. The **list selection** is the sole source of truth: whatever messages are selected when the drag is **initiated** are the payload. Conceptually, the selected set **is** the “tagged” set for this operation, but it is **not** the same state machine as the menu tag (see **Interaction between tag and selection** below).

### Multi-selection and platform conventions

- **Desktop:** Multiple rows are selected using **platform modifiers** while clicking (e.g. **Shift** for range, **Ctrl** / **Cmd** for disjoint selection), consistent with the OS and other mail clients.
- **Touch / compact:** Use the existing long-press → multi-select pattern (or platform equivalents).
- **Message list rows** are plain list items (no separate drag handle). **Mouse down** on **any row that belongs to the current selection** may begin a drag; the whole **selected set** participates, not only the row under the cursor (same idea as Finder and most mail UIs).

### When the source set is fixed (drag-drop vs menu)

- **Menu tag:** Fixed at the instant **Move** or **Copy** is invoked; later clicks do not add or remove messages from that pending operation.
- **Drag-drop:** In typical desktop mail and file managers, **which messages** move is fixed when the **drag session starts** (mouse movement past the drag threshold while button is down on a selected item). The user cannot realistically change the selection **during** the drag; in practice **snapshot the selected ids at drag start**.  
  **Drop** finalizes **destination folder** and, with **modifier keys**, **move vs copy** (match **platform expectations**, e.g. macOS / Windows conventions for “copy here” vs “move here”).
- **Correction:** Saying the source set is fixed “only at drop” is unusual; **destination and copy/move mode** are decided at drop; **source messages** are those selected at **drag start**.

### How this compares to familiar UIs

- **Apple Mail, Outlook, Thunderbird (broadly):** Select one or more messages; drag to a folder → **move** by default in many same-mailbox cases; **modifier at drop** (e.g. **Option/Alt**) often means **copy**. Exact modifiers differ by OS and version — **follow the active platform’s conventions**, documented in shortcuts help if needed.
- **Menu-driven “Move to…”** in some apps opens a picker; our design uses **tag + “Move here” on folder** instead, but the **drag** half behaves like classic **drag-selected-messages-to-folder**.

### Folder targets and context menu

- **Right click / long press / context menu** on a folder **other than the source folder** shows **Move here** and **Copy here** **only when** there is a **pending menu tag** (Move or Copy) with at least one message.
- **Targets** must be **writable** (not read-only, e.g. some NNTP or subscribed-only folders). **Exclude** the **source folder** (same store + same folder identity as the tag’s source).
- **Cross-store:** Allowed; **append to target first**, then **remove from source** on **move**, using existing **delete semantics** (mark deleted, trash, etc.) per store/folder rules.

### Move vs copy capability

- **Move** requires the ability to **delete** (or equivalent) messages on the **source** after a successful append. If the source cannot delete, **offer copy only** (or disable move).
- **Copy from “smart” / virtual folders:** **Allow copy** when the backend can supply message content. We **do not** depend on classifying folders as smart vs not; if the store can fetch bytes for append, copy is allowed. **Move** still requires a real deletable source.

### Partial success (especially cross-store)

- Track **per-message** append results. **Delete from source only for messages whose append succeeded.** If an append fails for a given message, **leave it on the source** and include it in a **warning** summary.
- Messages **already gone** from the source (another client, expunge, etc.) when the operation runs: **report a warning**; nothing else to do for those ids.

### Duplicates on target

- **Out of scope** for the transfer flow: we do not special-case duplicate detection. Any duplicate appears when the user opens that folder like any other message.

### Expunge (IMAP)

- As part of this work, add an explicit **Expunge** (or equivalent) **context menu item** on **folders that support expunge** (e.g. IMAP), separate from move/copy. Exact eligibility follows backend capability flags.

### Interaction between menu tag and drag (recommendation)

- **Drag-drop does not read the menu tag buffer**; it uses **list selection only**.
- Optionally **clear the pending menu tag** when a **drag-drop move/copy completes successfully** so the user is not left with a stale “pending move” after acting via drag. If the tag remains, folder context menus could still show **Move here** until the user clears or replaces the tag — product choice; **document whichever behaviour we implement.**

### Accessibility

- **Keyboard-only and assistive tech:** Context-menu-only targeting is insufficient. **Defer** a dedicated solution (e.g. **Move to folder…** / **Copy to folder…** modal searchable picker, or platform-standard accessibility APIs) to a **later iteration**; note as **known gap** until then.

### Tablet without application menu

- When the layout is **desktop-like** but **no native app menu** exists, expose **Move** / **Copy** (and related items) via a **compact-style overflow (⋮)** so behaviour matches mobile, not macOS-only chrome.
