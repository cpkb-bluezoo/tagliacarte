# Tagliacarte top-level build (Rust core + Rust app + wry GUI + terminal UI)

.PHONY: all build-app build-app-debug build-app-release build-tui run-tui build-gui bundle-gui run-gui run test-gui test clean help

CARGO := cargo
# Repo root = directory containing this Makefile (not $(CURDIR), so targets work from any cwd).
TAGLIACARTE_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
APP_CRATE := tagliacarte_app
UNAME_S := $(shell uname -s)

all: build-app build-gui

run: run-gui

build-app:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) build -p $(APP_CRATE) --release

build-app-debug:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) build -p $(APP_CRATE)

build-app-release:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) build -p $(APP_CRATE) --release

# Terminal UI (ratatui); binary: target/release/tagliacarte
build-tui:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) build -p tagliacarte --release

run-tui:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) run -p tagliacarte --release

# wry GUI (0.2.0); UI files live in gui/ui. TAGLIACARTE_UI_DIR=gui/ui serves them live from disk.
build-gui:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) build -p tagliacarte-gui --release

# macOS only: target/bundle/Tagliacarte.app, with the application's name, icon and identity.
bundle-gui:
	$(TAGLIACARTE_ROOT)/scripts/bundle-gui-macos.sh

# On macOS the app runs from a (debug) bundle, because that is what gives it its name
# in the menu bar and Dock and its icon; run unbundled it is called tagliacarte-gui.
# The executable is run directly, so the terminal, environment and Ctrl-C work as usual.
ifeq ($(UNAME_S),Darwin)
run-gui:
	cd $(TAGLIACARTE_ROOT) && PROFILE=debug $(TAGLIACARTE_ROOT)/scripts/bundle-gui-macos.sh >/dev/null
	TAGLIACARTE_UI_DIR=$(TAGLIACARTE_ROOT)/gui/ui $(TAGLIACARTE_ROOT)/target/bundle-debug/Tagliacarte.app/Contents/MacOS/tagliacarte-gui
else
run-gui:
	cd $(TAGLIACARTE_ROOT) && TAGLIACARTE_UI_DIR=$(TAGLIACARTE_ROOT)/gui/ui $(CARGO) run -p tagliacarte-gui
endif

test-gui:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) test -p tagliacarte-gui

test:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) test

clean:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) clean

help:
	@echo "Tagliacarte build"
	@echo ""
	@echo "Targets:"
	@echo "  all                   - build tagliacarte_app and the wry GUI (release)"
	@echo "  run                   - same as run-gui"
	@echo "  build-app             - build Rust tagliacarte_app crate (release)"
	@echo "  build-app-debug       - build Rust tagliacarte_app crate (debug)"
	@echo "  build-app-release     - build Rust tagliacarte_app crate (release)"
	@echo "  build-tui             - build terminal UI binary (tagliacarte)"
	@echo "  run-tui               - run terminal UI (release)"
	@echo "  build-gui             - build the wry GUI (release)"
	@echo "  run-gui               - run the wry GUI with the UI served live from gui/ui"
	@echo "  bundle-gui            - macOS: build target/bundle/Tagliacarte.app (name, icon, identity)"
	@echo "  test-gui              - run the wry GUI's unit tests"
	@echo "  test                  - run Cargo tests"
	@echo "  clean                 - cargo clean"
