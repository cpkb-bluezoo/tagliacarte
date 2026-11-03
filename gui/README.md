# Tagliacarte GUI (wry)

The 0.2.0 front end: a wry webview over `tagliacarte_app`, replacing Flutter.
The UI is plain HTML, CSS and ES modules under `ui/` (no bundler, no Node).

```bash
make run-gui     # debug build, UI served live from gui/ui (macOS: run from a debug bundle, for the name and icon)
make test-gui    # unit tests
make bundle-gui  # macOS: target/bundle/Tagliacarte.app
```

## How it works

- Everything the page shows is served by the `app` custom protocol from this
  process. The CSP and the navigation handler stop the page fetching anything
  else, so remote content (images later) is proxied by the back end.
- The page calls the back end with `POST /api/<name>` and a flat JSON object;
  the reply is `{"ok":true}`, `{"ok":true,"result":...}` or
  `{"ok":false,"error":"..."}`. Calls return once work is scheduled.
- Results and session activity arrive as a sequence of small JSON events from
  a held `POST /poll` (wry cannot stream a response, so this is a long-poll).
  The configuration is delivered the same way: `configStarted`, one
  `configAccount` / `configTransport` each, then `configLoaded`.
- The command channel requires a per-launch token (`X-Token`), because
  WKWebView sends no `Origin` on same-origin fetches to a custom scheme.
- Static files carry an ETag and `Cache-Control: no-cache`, and honour
  `If-None-Match` with a 304.
- JSON is read and written with rjsonparser through `tagliacarte_core::json`.
- A sequence of events that replaces something on the page (configuration,
  contacts) is pushed inside `Hub::sequence`, so two overlapping loads never
  interleave in the queue.

## Account pictures

An account is a round picture in the rail and in Settings (`ui/modules/avatar.js`):
the account's image when it has one, else a coloured disc with initials (one
word gives its first letter, several give the first and last word's; the colour
follows the account id). The picture is a setting of the account, a web address
or a file path, edited in the account's form and saved in `config.xml` as the
`<store>` attribute `avatar-url` (a Nostr profile sync sets it too). The page
asks for `/avatar/<account>` and never names where the picture is; the back end
(`app/src/avatar.rs`) fetches a web address with the remote-image rules or
reads a file of at most 2 MB, and serves only what the first bytes say is an
image. If it does not load, the disc stays.

A Matrix account's picture is looked up when asked for, not saved: the page
always asks for a Matrix account's picture (it has no way to know whether the
profile has one). The back end uses the picture chosen in the account's form
if it is an `mxc://` address, and otherwise the one in the account's Matrix
profile (`/profile/{userId}`). The `mxc://` address is fetched from the
account's own homeserver with its access token: a 96-pixel thumbnail and then
the original, on the authenticated `/_matrix/client/v1/media/...` paths that
current homeservers require and then on the earlier `/_matrix/media/v3/...`
ones. The address is checked before anything is sent (its server name and
media id become path segments). Opening the account's store signs in once and
is shared with the session, so this runs off the UI thread. A picture chosen
as a web address or file still wins over the profile, and a changed profile
picture shows up when the browser's hour-long cache lets it.

## Where you left off

The account, the folder and the message the user is in are saved to
`config.xml` as they move (`location.save`, after a 250 ms pause, so
keyboard stepping through a list is one write; the file is rewritten only if
the place changed, and the session is not touched). Choosing an account saves
only the account, so each account keeps its own last folder and message. At
start the page returns the first selected account to its saved folder and
message (`ui/modules/restore.js`): it waits for the folder to be listed and for
the list to load, and a folder that is gone, a message that was deleted, or the
user moving first are skipped without a word. Only the account that is selected
at start is restored, not one chosen later.

## Pane widths

The folder pane and the message list have a handle on their right border (the
reader takes what is left; the rail is a fixed width and has none). Drag it, or
focus it and press the left and right arrows (Shift for bigger steps), or
double-click it (or press Home) to put the pane back at its default of 260 and
380 pixels. The panes cannot go below 140 and 240 pixels, nor leave the reader
less than 320. The widths are the viewer's own, in local storage (`localprefs.js`,
`panes.js`), and a window too small for them shows them reduced without
forgetting them. While a handle is being dragged the reader's frame ignores the
pointer, so the drag is not lost over the message. The handles only show for the
mail panes, not for Settings, Compose or the calendar.

## Icon, title and menu

- **Title:** the page's own `document.title` (`ui/modules/title.js`): what is
  open, then the application, for example `Lunch? - Inbox - Tagliacarte` or
  `Inbox (3) - Work - Tagliacarte`. The shell mirrors it to the window.
- **Menu:** `src/menu.rs` owns the structure and the shortcuts (App, File, Edit,
  View, Message, Window, Help). The page sends the words (`menu.labels`, from
  the ARB strings, once at start) and which message actions can be used now
  (`menu.enable`); a choice of one of its own items comes back as a
  `menuAction` event handled in `ui/modules/menu.js`. The system's items (Hide,
  Quit, Copy, ...) never reach the page. Message actions share one rule with
  the buttons in the reader (`messageActions` in `reader.js`).
- **Icon:** in a bundle it is the one compiled from `icons/AppIcon.icon`.
  `make bundle-gui` (macOS) builds `target/bundle/Tagliacarte.app` with the
  name, identifier `org.bluezoo.tagliacarte`, that icon and an ad hoc
  signature. Run unbundled (`make run-gui`) macOS would show the executable's
  name and a generic icon; the Dock icon is set from `assets/app-icon.png`
  instead (the menu bar still says `tagliacarte-gui` there; only a bundle fixes
  that). Windows and Linux get the same PNG as the window icon.
- Only macOS has the menu attached so far, and only macOS has been looked at.

## Calendar

The calendar button at the foot of the rail (above Settings) opens a view that
replaces the mail panes. It has the same four scales as the Flutter calendar: a
day (the events), a week (seven collapsible days), a month (a six-week grid, three
events a day and "+n more", clicking a day lists its events or starts a new one)
and a year (twelve months, clicking one opens it). Previous, Today and Next move by
the scale's own unit, the calendars are toggles (hiding one needs no new request),
and the scale and the hidden calendars are kept (`ui/modules/localprefs.js`).
Weeks start on Monday. The sync button syncs the calendars of Gmail, Graph and
Exchange accounts (or anything signed in with Google or Microsoft) for two weeks
back and four months ahead, as before.

Events are made and changed in an editor with the fields the store has (calendar,
title, all day, start, end, location, description, invitees) and written straight to
the store by `app/src/calendar_view.rs`; the Flutter interface built an iCalendar
text for every change and had no date fields. An event that is changed keeps who
organised it, how it repeats and the server's tag, and goes up a step in sequence.
Delete marks an event cancelled (as Flutter did) and the list now leaves cancelled
events out, which Flutter did not. There is always a calendar called Local (the
store has none until something is synced). An invitation attached to a message has
a calendar button beside it that adds its events to a calendar called Invitations.

Times are UTC milliseconds in the store. An all-day event is its first day at UTC
midnight with the end at the midnight after its last, so the page shows it by its
UTC date; read through local time it falls on the wrong day in zones west of UTC.

Not covered, because the store or importer does not do it: a repeating event
(`RRULE`) is stored but shown once, not on each repeat; a time with a `TZID`
(Google and Outlook invitations) is read as UTC by the importer, so it can be at
the wrong hour; a change is made in the local store only, and a later sync of the
service can replace it; there is no CalDAV user interface (the Flutter one had none).

## The reader

Top to bottom: the headers (left-aligned, from the top edge of the panel, with the
subject), the offer to load remote images when there are some, the message body,
and the attachments last, only when there are any. The actions (Reply, Reply all,
Forward, or Edit draft in Drafts) are round Lucide icon buttons floating in front
of the headers' pane at the top-right corner, whose right side is left clear for
them. They are the same pictures the
Flutter interface used (`ui/icons/`, drawn through CSS masks so they take the
text colour) and follow the same rule as the Message menu (`messageActions`).

**Signature mark.** A signed message shows a mark at the right of the panel,
under the actions' row: a green `badge-check` when the signature checks out, a
red `badge-x` when it does not, a grey `circle-help` when it could not be checked
(a missing or unusable key). Nothing is shown for a message that is not signed or
whose sender has no key in the address book (Contacts: PGP key file, S/MIME
certificate). The back end (`mail_crypto::incoming_signature_status`) sends a
`messageSignature` event as part of opening a message. From IMAP the message is
fetched whole for this only when its structure says `multipart/signed` and its
attachments are under 25 MB. PGP/MIME is checked as RFC 3156 says, over the raw
bytes of the first part including its headers (the older layout this app itself
used to write, the part's body alone, is accepted too, and a message kept with bare
LF is treated as CRLF); S/MIME goes through OpenSSL and also requires the signer to
be the certificate held for that address. Fixtures for these are in
`app/tests/fixtures/` (public keys and certificates and finished messages only).

**Offer to load images.** The body, when the frame loads it, is counted by the back
end (`ViewResponse::remote_images`, a `messageRemote` event), whether or not images
are allowed, so the offer appears only when there is something to load. Pictures
are all it covers: remote stylesheets, fonts and the like are blocked by the body's
content security policy and are not fetched. A body is therefore sent `no-store`, so
that every load reaches the back end; the page also remembers each count for the
session.

## Remote images

Blocked unless the person opts in, and the page never fetches them. The body
document's content security policy allows images from the app and nothing
else, so a blocked message needs no rewriting. Opting in is the Viewing
setting (`loadRemoteImages`, off by default) or the "Load images" button, which
offers itself only for a message with an HTML body (`messageBody` event) and
applies to that message alone. The page then asks for `.../body?images=1`.

With that, the back end rewrites remote image references (`img`, `srcset`,
`poster`, `background`, CSS `url()` in `<style>` and `style=`; never links) to
`/remote/<key>/<base64url address>` and serves them itself. The key is a secret
chosen at launch that only a rewritten body contains, so a message cannot make
the app fetch by writing a proxy URL. The fetcher (`app/src/remote_image.rs`)
takes only http and https, only addresses on the public internet, judged on the
answer the dial then uses (it is pinned in a dedicated resolver's cache, so a
hostile DNS server cannot answer differently the second time) and again at each
redirect (four at most), only `image/*` up to 10 MB, and sends no cookie or
referrer. The window's protocol answers with one body, which is why there is a
size cap and not a stream.

When an image does not show, the terminal says why: one line per body
(`remote images allowed: N address(es) pointed at the proxy`, so 0 means the
message had none to rewrite) and one per refused fetch
(`remote image refused (status): reason [address]`). A server that calls an
image `application/octet-stream`, or sends no type, is judged by the first
bytes for the usual formats; anything that says it is `text/html` is refused.

`cargo test -p tagliacarte_app --test remote_image_internet -- --ignored`
checks HTTPS and certificate names against the real internet.

## Saving an attachment

An attachment in the message view is a button. `attachment.save` takes the
message, the attachment's position and a suggested name, and no destination:
the back end shows the native save dialog itself and writes only to the path
that comes back. The bytes go from the store to a `.part` file beside the
destination, decoded as they arrive (IMAP fetches just that part, so memory is
one chunk), and the file replaces the destination only when complete; a failed
save leaves any existing file alone. The page hears one `attachmentSaved`
event (`path` and `bytes`, `cancelled`, or `error`). A local store has to read
the whole message to find the part. In `--selftest` there is no dialog and the
file is written beside the scratch config as `saved-<name>`.

## Compose

Reply, Reply all, Forward, Edit draft and a plain Compose button, as a view
that replaces the mail panes. The Compose button is second in the rail, under
the accounts and above Calendar and Settings, and is always there: it starts a
message from the account in view, or goes back to the one being written, and it
is off while the compose view is showing or the account in view cannot send. The
compose bar's actions (delivery notifications, signing and encryption, attach,
save draft, discard, send) are round Lucide icon buttons named by their tooltips. The reply rules (subjects, recipients, threading,
quoting, the HTML sanitiser) are pure JavaScript in `ui/modules/replyformat.js`
and checked by the self-test.

- Attachments never sit whole in memory. Files chosen or dropped are added by
  path (`compose.attachPath`); bytes from the page go to `POST /upload` as a raw
  body and are spooled to a temp file (64 MB cap, the page cannot hand wry a
  larger body in one piece, so big files go by path or drop).
- Sending writes the message to a spool file in bounded chunks
  (`write_rfc822` in core) and hopf-smtp sends that file dot-stuffed and
  chunked. Bcc is never in the headers. Crypto reads the message into memory
  only when signing or encrypting is requested.
- Still buffered: the IMAP APPEND of the Sent copy and of drafts (hopf-imap
  sends one literal slice), Gmail REST and NNTP.
- Not offered: Exchange, Matrix and Nostr compose.

## Settings

Seven tabs: Accounts, Outgoing, Security, Viewing, Composing, Contacts, About.

- Preferences (Security, Viewing, Composing) save one setting at a time with
  `settings.set`. A config that does not parse is never overwritten.
- Accounts and transports are edited as drafts and saved with `account.save`
  / `transport.save`. What each account type writes to `config.xml` is in
  `ui/modules/settings/accountschema.js`, a pure module the self-test checks.
- Anything that waits on a person or a network (OAuth, SMTP verification,
  CardDAV, Nostr relays) returns at once and reports with an event.
- Strings are the ARB files, served at `/locales/<lang>.arb`; keys that only
  this UI needs are prefixed `web` and live in `app_en.arb` (other languages
  fall back to English until translated).
- Not carried over because they did nothing in the Flutter UI: the OAuth and
  Matrix E2EE buttons on Security, and Test Send / Signature Editor on
  Composing.

## Self-test

`tagliacarte-gui --selftest` drives the real UI against the real session and
prints `[page] PASS|FAIL ...` lines, then exits. It changes settings,
accounts, transports and contacts and puts them back, so use a scratch config
and data directory, never real ones:

```bash
mkdir -p /tmp/tc && cp -R test_data/maildir /tmp/tc/maildir
cat > /tmp/tc/config.xml <<XML
<tagliacarte>
  <stores>
    <store id="s1" type="maildir" display-name="Test" path="/tmp/tc/maildir"/>
  </stores>
</tagliacarte>
XML
printf 'BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Selftest\r\nEMAIL:alice.selftest@example.org\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bob Selftest\r\nEMAIL:bob.selftest@example.org\r\nEND:VCARD\r\n' > /tmp/tc/test.vcf
cp gui/assets/app-icon.png /tmp/tc/avatar.png
TAGLIACARTE_DATA_DIR=/tmp/tc TAGLIACARTE_CONFIG=/tmp/tc/config.xml \
  cargo run -p tagliacarte-gui -- --selftest
```

`TAGLIACARTE_TRACE=1` logs each request.
