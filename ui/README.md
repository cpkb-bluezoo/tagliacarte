# Tagliacarte UI

Qt 6 (C++/Widgets) frontend. K-9-style layout:

- Main window: conversation list (placeholder until Phase 2).
- Hamburger (top-left): slide-in pane with main sidebar (stores + Settings) and folder list.
- Desktop/tablet: unfold sidebar and folder list into the main window.

Phase 1h: minimal app skeleton that links to core via FFI.
Phase 2: conversation view (message body in QTextBrowser), compose (To/From/Subject/Body + SMTP send).

## Build (Phase 1h)

1. **Build the Rust FFI library** (from repo root):
   ```bash
   rustup default stable
   cargo build -p tagliacarte_ffi --release
   ```
   The shared library is produced at `target/release/libtagliacarte_ffi.dylib` (macOS) or `target/release/libtagliacarte_ffi.so` (Linux).

2. **Build the Qt app** (requires Qt 6):
   ```bash
   cd ui
   mkdir build && cd build
   cmake .. -DCMAKE_PREFIX_PATH=/path/to/Qt/6.x/gcc_64   # or your Qt install
   cmake --build .
   ```
   If the FFI library is not in `../target/release`, set `TAGLIACARTE_FFI_DIR`:
   ```bash
   cmake .. -DTAGLIACARTE_FFI_DIR=/path/to/dir
   ```

3. **Run**: from `build`, run `./tagliacarte_ui` (or open the .app on macOS). Use "Open Maildir…" to pick a Maildir root; folders and conversation list will populate via the Rust core.
