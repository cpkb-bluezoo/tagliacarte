# Tagliacarte top-level build.
# Prerequisites: Rust (rustup default stable), Qt 6 (set QT_PREFIX if needed).

.PHONY: all debug release clean build-ffi build-ui run test test-integration help icons

# Build type: release (default) or debug
BUILD ?= release

# Qt 6 install path for CMake (e.g. /opt/Qt/6.6.0/gcc_64 or /Users/.../Qt/6.x/macos)
# Set via: make QT_PREFIX=/path or export QT_PREFIX
QT_PREFIX ?=

# ---------- Rust ----------
CARGO := cargo
CARGO_BUILD = $(CARGO) build
CARGO_RELEASE = $(CARGO) build --release
TARGET_DIR := $(CURDIR)/target
FFI_RELEASE := $(TARGET_DIR)/release/libtagliacarte_ffi.dylib
FFI_RELEASE_LINUX := $(TARGET_DIR)/release/libtagliacarte_ffi.so
FFI_DEBUG := $(TARGET_DIR)/debug/libtagliacarte_ffi.dylib
FFI_DEBUG_LINUX := $(TARGET_DIR)/debug/libtagliacarte_ffi.so

# ---------- Icons ----------
ICONS_SCRIPT := $(CURDIR)/scripts/gen-icons.sh

# ---------- UI ----------
UI_DIR := $(CURDIR)/ui
UI_BUILD := $(UI_DIR)/build
# FFI dir passed to CMake (so UI links and finds the dylib/so)
FFI_DIR_RELEASE := $(TARGET_DIR)/release
FFI_DIR_DEBUG := $(TARGET_DIR)/debug

# ---------- Targets ----------
# Default: release build of everything
all: build-ffi-release build-ui-release

debug: build-ffi-debug build-ui-debug

release: build-ffi-release build-ui-release

# Rust: core + ffi
build-ffi: build-ffi-release
build-ffi-release:
	$(CARGO_RELEASE) -p tagliacarte_ffi
build-ffi-debug:
	$(CARGO_BUILD) -p tagliacarte_ffi

# App icon (SVG → PNG + macOS .icns). Run once or when icons change.
icons:
	@$(ICONS_SCRIPT)

# Qt UI (depends on FFI being built so lib exists for linking)
build-ui: build-ui-release
build-ui-release: build-ffi-release
	@mkdir -p $(UI_BUILD)
	cd $(UI_BUILD) && cmake $(UI_DIR) \
		-DCMAKE_BUILD_TYPE=Release \
		-DTAGLIACARTE_FFI_DIR=$(FFI_DIR_RELEASE) \
		$(if $(QT_PREFIX),-DCMAKE_PREFIX_PATH=$(QT_PREFIX),)
	$(MAKE) -C $(UI_BUILD)
build-ui-debug: build-ffi-debug
	@mkdir -p $(UI_BUILD)
	cd $(UI_BUILD) && cmake $(UI_DIR) \
		-DCMAKE_BUILD_TYPE=Debug \
		-DTAGLIACARTE_FFI_DIR=$(FFI_DIR_DEBUG) \
		$(if $(QT_PREFIX),-DCMAKE_PREFIX_PATH=$(QT_PREFIX),)
	$(MAKE) -C $(UI_BUILD)

run: build-ui-release
	@if [ -d "$(UI_BUILD)/tagliacarte_ui.app" ]; then \
		open "$(UI_BUILD)/tagliacarte_ui.app"; \
	else \
		$(UI_BUILD)/tagliacarte_ui; \
	fi

test:
	$(CARGO) test

test-integration:
	$(CARGO) test -p tagliacarte_core --test http_integration -- --ignored --nocapture

clean:
	$(CARGO) clean
	rm -rf $(UI_BUILD)

help:
	@echo "Tagliacarte top-level build"
	@echo ""
	@echo "Targets:"
	@echo "  all (default)  - build FFI (release) and Qt UI"
	@echo "  release        - same as all"
	@echo "  debug          - build FFI and UI in debug mode"
	@echo "  build-ffi      - build Rust core + ffi (release)"
	@echo "  build-ui       - build Qt app (release); runs cmake if needed"
	@echo "  icons          - generate app icon (icons/app-icon.png, icons/icon.icns on macOS)"
	@echo "  run            - build then run the Qt app"
	@echo "  test           - cargo test (unit tests only)"
	@echo "  test-integration - run HTTP integration tests (requires network)"
	@echo "  clean          - cargo clean + remove ui/build"
	@echo ""
	@echo "Variables:"
	@echo "  QT_PREFIX      - Qt 6 install path for CMake (e.g. /opt/Qt/6.6.0/gcc_64)"
	@echo "                    use: make QT_PREFIX=/path/to/Qt/6.x/..."
	@echo "  BUILD=debug    - use debug build for FFI/UI"
