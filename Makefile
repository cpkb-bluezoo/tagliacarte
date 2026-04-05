# Tagliacarte top-level build (Rust core + Rust app + Flutter UI)

.PHONY: all build-app build-app-debug build-app-release build-tui run-tui flutter-pub flutter-run run-release flutter-build-macos flutter-build-android flutter-build-ios flutter-test test test-integration clean help

CARGO := cargo
# Repo root = directory containing this Makefile (not $(CURDIR), so targets work from any cwd).
TAGLIACARTE_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
FLUTTER_DIR := $(TAGLIACARTE_ROOT)/flutter_ui
APP_CRATE := tagliacarte_app
UNAME_S := $(shell uname -s)
# macOS App Sandbox: do not pass a workspace path; the dylib is embedded by Xcode (see macOS Runner).
ifeq ($(UNAME_S),Darwin)
FLUTTER_RUST_DEFINE :=
else ifeq ($(UNAME_S),Linux)
FLUTTER_RUST_DEFINE := --dart-define=TAGLIACARTE_RUST_LIB=$(TAGLIACARTE_ROOT)/target/release/libtagliacarte_app.so
else
FLUTTER_RUST_DEFINE := --dart-define=TAGLIACARTE_RUST_LIB=$(TAGLIACARTE_ROOT)/target/release/tagliacarte_app.dll
endif

all: build-app flutter-pub

run: flutter-run

run-release: flutter-pub build-app-release
	cd $(FLUTTER_DIR) && flutter run --release $(FLUTTER_RUST_DEFINE)

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

flutter-pub:
	cd $(FLUTTER_DIR) && flutter pub get

flutter-run: flutter-pub build-app-release
	cd $(FLUTTER_DIR) && flutter run $(FLUTTER_RUST_DEFINE)

flutter-build-android: flutter-pub
	cd $(FLUTTER_DIR) && flutter build apk

# Produces Tagliacarte.app under flutter_ui/build/macos/Build/Products/Release/ (or Debug with --debug).
flutter-build-macos: flutter-pub build-app-release
	cd $(FLUTTER_DIR) && flutter build macos $(FLUTTER_RUST_DEFINE)

flutter-build-ios: flutter-pub
	cd $(FLUTTER_DIR) && flutter build ios --no-codesign

flutter-test: flutter-pub
	cd $(FLUTTER_DIR) && flutter test

test:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) test

test-integration:
	cd $(FLUTTER_DIR) && flutter test integration_test

clean:
	cd $(TAGLIACARTE_ROOT) && $(CARGO) clean
	@command -v flutter >/dev/null 2>&1 && cd $(FLUTTER_DIR) && flutter clean || true
	rm -rf $(FLUTTER_DIR)/build $(FLUTTER_DIR)/.dart_tool

help:
	@echo "Tagliacarte build"
	@echo ""
	@echo "Targets:"
	@echo "  all                   - build tagliacarte_app and fetch Flutter deps"
	@echo "  build-app             - build Rust tagliacarte_app crate (release)"
	@echo "  build-app-debug       - build Rust tagliacarte_app crate (debug)"
	@echo "  build-app-release     - build Rust tagliacarte_app crate (release)"
	@echo "  build-tui             - build terminal UI binary (tagliacarte)"
	@echo "  run-tui               - run terminal UI (release)"
	@echo "  flutter-pub           - install Flutter dependencies"
	@echo "  flutter-run           - run Flutter (macOS: Rust lib embedded via Xcode; Linux/Win: dart-define path)"
	@echo "  run-release           - run Flutter in release mode (same rules)"
	@echo "  flutter-build-macos   - build macOS .app bundle (requires Xcode on macOS)"
	@echo "  flutter-build-android - build Android APK"
	@echo "  flutter-build-ios     - build iOS app (requires Xcode)"
	@echo "  flutter-test          - run Flutter tests"
	@echo "  test                  - run Cargo tests"
	@echo "  test-integration      - run Flutter integration tests"
	@echo "  clean                 - cargo clean, flutter clean (if flutter on PATH), rm flutter_ui/build and .dart_tool"
