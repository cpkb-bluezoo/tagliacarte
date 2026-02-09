# Test Maildir

Sample Maildir for manual testing of Tagliacarte. Contains four messages in INBOX (the root `new/` and `cur/`).

**Use in the app:** Settings → Accounts → Maildir → choose this directory (e.g. `test_data/maildir` from the repo root). Then open INBOX from the folder list.

- **new/** — unread messages (four sample RFC 5322 mails)
- **cur/** — read messages (empty)
- **tmp/** — delivery lock (empty)

Message filenames follow Maildir convention: `<timestamp>.<unique>.local`. Content is minimal valid email (From, To, Subject, Date, Message-ID, body).
