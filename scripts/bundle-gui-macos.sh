#!/bin/sh
# Builds target/bundle/Tagliacarte.app (target/bundle-debug/ for PROFILE=debug) from the wry GUI.
#
# The bundle is what gives macOS the application's name (menu bar, Dock, About),
# its icon and its identity. The icon is compiled from the Icon Composer file in
# icons/ with actool, so that it gets the system's own styling, not the grey box
# macOS puts around a flat .icns.
#
# The result is signed ad hoc, which is enough to run on this machine. Signing
# for other people's (a Developer ID and notarisation) is a separate step.
#
#   scripts/bundle-gui-macos.sh            release build
#   PROFILE=debug scripts/bundle-gui-macos.sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
profile=${PROFILE:-release}
name=Tagliacarte
identifier=org.bluezoo.tagliacarte
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/gui/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "no version in gui/Cargo.toml" >&2; exit 1; }

if [ "$profile" = release ]; then
    cargo build --manifest-path "$root/Cargo.toml" -p tagliacarte-gui --release
else
    cargo build --manifest-path "$root/Cargo.toml" -p tagliacarte-gui
fi

if [ "$profile" = release ]; then out=bundle; else out=bundle-$profile; fi
app="$root/target/$out/$name.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/$profile/tagliacarte-gui" "$app/Contents/MacOS/tagliacarte-gui"

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
xcrun actool "$root/icons/AppIcon.icon" --compile "$app/Contents/Resources" \
    --platform macosx --minimum-deployment-target 12.0 --app-icon AppIcon \
    --output-partial-info-plist "$scratch/icon.plist" >/dev/null

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>tagliacarte-gui</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleIconName</key>
	<string>AppIcon</string>
	<key>CFBundleIdentifier</key>
	<string>$identifier</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>$name</string>
	<key>CFBundleDisplayName</key>
	<string>$name</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.productivity</string>
	<key>LSMinimumSystemVersion</key>
	<string>12.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>Copyright © 2026 Chris Burdess</string>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist" >/dev/null

codesign --force --deep --sign - "$app" >/dev/null 2>&1
codesign --verify --deep --strict "$app"

echo "$app"
