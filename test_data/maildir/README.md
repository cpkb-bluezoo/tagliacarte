# Test Maildir

Sample Maildir for manual testing of Tagliacarte. Contains seven messages in INBOX (the root `new/` and `cur/`); one has a 3000-byte attachment, `sample.bin`, and one is HTML with remote images (an unresolvable host and a loopback address, so nothing is ever fetched), and one has a calendar invitation attached (`invite.ics`, Quarterly review on 2 March 2026).

**Use in the app:** Settings → Accounts → Maildir → choose this directory (e.g. `test_data/maildir` from the repo root). Then open INBOX from the folder list.

- **new/** — unread messages (four sample RFC 5322 mails; the fifth, with the attachment, is in `cur/`)
- **cur/** — read messages (empty)
- **tmp/** — delivery lock (empty)

Message filenames follow Maildir convention: `<timestamp>.<unique>.local`. Content is minimal valid email (From, To, Subject, Date, Message-ID, body).
