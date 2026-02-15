#!/usr/bin/env bash
# Generate Tagliacarte app icons from icons/app-icon.svg.
# Output: icons/app-icon.png (1024x1024), icons/icon.icns (macOS).
# macOS .icns uses app-icon-macos.svg (full-bleed square, no rounded corners)
# so that the OS squircle mask produces edge-to-edge artwork.
# Requires: one of ImageMagick (convert), librsvg (rsvg-convert), or Python cairosvg
# On macOS: sips and iconutil (Xcode command line tools) for .icns.

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ICONS_DIR="$ROOT_DIR/icons"
SVG="$ICONS_DIR/app-icon.svg"
SVG_MACOS="$ICONS_DIR/app-icon-macos.svg"
PNG1024="$ICONS_DIR/app-icon.png"
PNG1024_MACOS="$ICONS_DIR/app-icon-macos.png"
ICONSET="$ICONS_DIR/tagliacarte.iconset"
ICNS="$ICONS_DIR/icon.icns"

cd "$ROOT_DIR"
mkdir -p "$ICONS_DIR"

if [[ ! -f "$SVG" ]]; then
  echo "Missing $SVG" >&2
  exit 1
fi

# ---- SVG → 1024×1024 PNG ----
if command -v convert &>/dev/null; then
  echo "Using ImageMagick..."
  convert -background none -resize 1024x1024 "$SVG" "$PNG1024"
elif command -v rsvg-convert &>/dev/null; then
  echo "Using rsvg-convert (librsvg)..."
  rsvg-convert -w 1024 -h 1024 "$SVG" -o "$PNG1024"
elif python3 -c "import cairosvg" 2>/dev/null; then
  echo "Using Python cairosvg..."
  SVG_PATH="$SVG" PNG_PATH="$PNG1024" python3 << 'PY'
import cairosvg
import os
cairosvg.svg2png(
    url=os.environ["SVG_PATH"],
    write_to=os.environ["PNG_PATH"],
    output_width=1024,
    output_height=1024
)
PY
else
  echo "No SVG→PNG converter found. Install one of:" >&2
  echo "  brew install imagemagick    # for convert" >&2
  echo "  brew install librsvg       # for rsvg-convert" >&2
  echo "  pip install cairosvg       # for Python" >&2
  exit 1
fi

echo "Generated $PNG1024"

# ---- macOS .icns (optional) ----
if [[ "$(uname)" != "Darwin" ]]; then
  echo "Skipping .icns (not macOS). Use $PNG1024 for other platforms."
  exit 0
fi

if ! command -v iconutil &>/dev/null; then
  echo "iconutil not found (install Xcode command line tools). Skipping .icns." >&2
  exit 0
fi

if [[ ! -f "$SVG_MACOS" ]]; then
  echo "Missing $SVG_MACOS (full-bleed square variant for macOS)" >&2
  exit 1
fi

# Generate 1024×1024 PNG from the macOS-specific SVG (no rounded corners)
echo "Generating macOS icon from $SVG_MACOS..."
if command -v convert &>/dev/null; then
  convert -background none -resize 1024x1024 "$SVG_MACOS" "$PNG1024_MACOS"
elif command -v rsvg-convert &>/dev/null; then
  rsvg-convert -w 1024 -h 1024 "$SVG_MACOS" -o "$PNG1024_MACOS"
elif python3 -c "import cairosvg" 2>/dev/null; then
  SVG_PATH="$SVG_MACOS" PNG_PATH="$PNG1024_MACOS" python3 << 'PY'
import cairosvg
import os
cairosvg.svg2png(
    url=os.environ["SVG_PATH"],
    write_to=os.environ["PNG_PATH"],
    output_width=1024,
    output_height=1024
)
PY
fi

# Ensure the PNG is in sRGB colour space. SVGs with only black/white produce
# greyscale PNGs which macOS treats as legacy icons (white-background wrap).
SRGB_PROFILE="/System/Library/ColorSync/Profiles/sRGB Profile.icc"
if [[ -f "$SRGB_PROFILE" ]]; then
  echo "Converting $PNG1024_MACOS to sRGB..."
  sips -m "$SRGB_PROFILE" "$PNG1024_MACOS" --out "$PNG1024_MACOS"
fi

echo "Generated $PNG1024_MACOS"

rm -rf "$ICONSET"
mkdir -p "$ICONSET"

# Required sizes for iconutil (name → pixel size)
# https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Optimizing/Optimizing.html
sips -z 16 16     "$PNG1024_MACOS" --out "$ICONSET/icon_16x16.png"
sips -z 32 32     "$PNG1024_MACOS" --out "$ICONSET/icon_16x16@2x.png"
sips -z 32 32     "$PNG1024_MACOS" --out "$ICONSET/icon_32x32.png"
sips -z 64 64     "$PNG1024_MACOS" --out "$ICONSET/icon_32x32@2x.png"
sips -z 128 128   "$PNG1024_MACOS" --out "$ICONSET/icon_128x128.png"
sips -z 256 256   "$PNG1024_MACOS" --out "$ICONSET/icon_128x128@2x.png"
sips -z 256 256   "$PNG1024_MACOS" --out "$ICONSET/icon_256x256.png"
sips -z 512 512   "$PNG1024_MACOS" --out "$ICONSET/icon_256x256@2x.png"
sips -z 512 512   "$PNG1024_MACOS" --out "$ICONSET/icon_512x512.png"
cp "$PNG1024_MACOS" "$ICONSET/icon_512x512@2x.png"

iconutil -c icns -o "$ICNS" "$ICONSET"
rm -rf "$ICONSET"

echo "Generated $ICNS"
echo "Run 'make build-ui-release' (or reconfigure CMake) so the app bundle uses the icon."
