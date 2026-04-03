#!/usr/bin/env bash
# macOS Dock/Launchpad composite app icons incorrectly when assets are
# grayscale+alpha PNGs; use sRGB RGBA (e.g. PNG32) instead.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SVG="$ROOT/assets/icons/app-icon-macos.svg"
OUT="$ROOT/macos/Runner/Assets.xcassets/AppIcon.appiconset"
if ! command -v magick >/dev/null 2>&1; then
  echo "ImageMagick 'magick' is required (e.g. brew install imagemagick)" >&2
  exit 1
fi
for s in 16 32 64 128 256 512 1024; do
  magick -background none "$SVG" -resize "${s}x${s}" -colorspace sRGB "PNG32:$OUT/app_icon_${s}.png"
done
echo "Updated $OUT (sRGB RGBA)."
