#!/usr/bin/env bash
# Builds SolarOffice.app and a .dmg with a background window.   ./packaging/macos/package.sh
#   ARCH=native             build for this Mac only (faster; default: universal arm64 + x86_64)
#   DMG=0                   stop after SolarOffice.app (no .dmg)
#   DMG_BACKGROUND=my.png   another background (640x400; my@2x.png next to it is used for Retina)
#   DMG_LAYOUT=0            skip the Finder window layout (no background; for headless builds)
#   SIGN_IDENTITY="Developer ID Application: …"   sign with a real identity (default: ad-hoc)
# The layout step drives Finder: allow Terminal to control Finder when macOS asks
# (System Settings → Privacy & Security → Automation).
#
# The app is assembled and signed in $TMPDIR and then copied to dist/: this source folder may
# live in iCloud Drive, whose extended attributes make codesign fail.
set -euo pipefail
cd "$(dirname "$0")/../.."
SRC="$PWD"
VERSION="${VERSION:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
ARCH="${ARCH:-universal}"
export MACOSX_DEPLOYMENT_TARGET=11.0
trap 'echo "error: packaging/macos/package.sh failed at line $LINENO: $BASH_COMMAND" >&2' ERR

# 1. Build.
HOST_TARGET="$(rustc -vV | awk '/^host:/ {print $2}')"
case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin); SUFFIX=universal ;;
  native) TARGETS=("$HOST_TARGET"); SUFFIX="${HOST_TARGET%%-*}" ;;
  *) echo "error: ARCH must be universal or native, not '$ARCH'" >&2; exit 2 ;;
esac
rustup target add "${TARGETS[@]}" >/dev/null
for t in "${TARGETS[@]}"; do
  echo "==> cargo build --release --target $t"
  cargo build --release --target "$t" -p solaroffice
done

# 2. Assemble the bundle in a scratch folder.
WORK=$(mktemp -d "${TMPDIR:-/tmp}/solaroffice-pkg.XXXXXX")
DEVICE=""
eject_device() { # eject, retrying while Spotlight or a scanner still holds the volume
  local dev="$1"
  for _ in 1 2 3 4 5; do
    diskutil eject "$dev" >/dev/null 2>&1 && return 0
    echo "  the disk image is still busy, retrying in 2 s…" >&2; sleep 2
  done
  diskutil unmountDisk force "$dev" >/dev/null 2>&1 || true
  diskutil eject "$dev" >/dev/null 2>&1
}
cleanup() {
  if [ -n "$DEVICE" ]; then eject_device "$DEVICE" || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

APP_NAME="SolarOffice.app"
APP="$WORK/$APP_NAME"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
if [ "${#TARGETS[@]}" -gt 1 ]; then
  BINS=()
  for t in "${TARGETS[@]}"; do BINS+=("target/$t/release/solaroffice"); done
  lipo -create "${BINS[@]}" -output "$APP/Contents/MacOS/solaroffice"
else
  cp "target/${TARGETS[0]}/release/solaroffice" "$APP/Contents/MacOS/solaroffice"
fi
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist.in > "$APP/Contents/Info.plist"
cp assets/app-icon/solaroffice.icns "$APP/Contents/Resources/solaroffice.icns"
ditto examples "$APP/Contents/Resources"   # the example documents sit directly in Resources; the app copies them into the save folder on first launch
printf 'APPL????' > "$APP/Contents/PkgInfo"
xattr -cr "$APP" 2>/dev/null || true

# 3. Sign: ad-hoc unless SIGN_IDENTITY is set. Ad-hoc failing is not fatal (the app still runs
#    locally); a real identity failing is.
if [ -n "${SIGN_IDENTITY:-}" ]; then
  codesign --force --deep --options runtime --timestamp -s "$SIGN_IDENTITY" "$APP"
else
  codesign --force --deep -s - "$APP" 2>/dev/null || echo "warning: ad-hoc signing failed; the app is unsigned" >&2
fi

mkdir -p dist
rm -rf "dist/$APP_NAME"
ditto "$APP" "dist/$APP_NAME"
echo "Built: $SRC/dist/$APP_NAME"
if [ "${DMG:-1}" = 0 ]; then exit 0; fi

# 4. The .dmg.
DMG_FILE="$SRC/dist/SolarOffice-$VERSION-macos-$SUFFIX.dmg"
VOL="SolarOffice"                                          # volume name shown in Finder
BG="${DMG_BACKGROUND:-packaging/macos/dmg-background.png}" # 640x400
BG2X="${BG%.png}@2x.png"                                   # optional 1280x800 for Retina
WIN_W=640; WIN_H=400                                       # window size; match the background

# `diskutil image` (macOS 26+) replaces hdiutil, which is deprecated; older systems use hdiutil.
MACOS_MAJOR=$(sw_vers -productVersion | cut -d. -f1)
if [ "$MACOS_MAJOR" -ge 26 ]; then IMG=diskutil; else IMG=hdiutil; fi
echo "==> making the .dmg with $IMG"
RW="$WORK/SolarOffice-rw.dmg"

# An open SolarOffice volume (an earlier .dmg) would make this one "SolarOffice 1", and Finder
# saves the window layout per volume name: eject it first.
for vol in /Volumes/"$VOL" /Volumes/"$VOL "[0-9]*; do
  if [ -d "$vol" ]; then echo "Ejecting $vol"; diskutil eject "$vol" >/dev/null 2>&1 || diskutil unmount force "$vol" >/dev/null 2>&1 || true; fi
done

# A blank writable image: the app plus headroom (there is no writable `create from`).
APP_MB=$(du -sm "$APP" | awk '{print $1}')
SIZE_MB=$(( APP_MB + APP_MB / 5 + 20 ))
if [ "$IMG" = diskutil ]; then
  diskutil image create blank --size "${SIZE_MB}m" --volumeName "$VOL" "$RW" >/dev/null
else
  hdiutil create -quiet -size "${SIZE_MB}m" -fs HFS+ -volname "$VOL" "$RW" 2>&1 | grep -v "deprecated" || true
fi
[ -f "$RW" ] || { echo "error: could not create the disk image" >&2; exit 1; }

# Attach it browsable (not -nobrowse: Finder has to see it to save the window layout).
if [ "$IMG" = diskutil ]; then ATTACH_OUT=$(diskutil image attach "$RW"); else ATTACH_OUT=$(hdiutil attach "$RW" -noautoopen -noverify); fi
DEVICE=$(echo "$ATTACH_OUT" | grep -E '^/dev/disk' | head -1 | awk '{print $1}' || true)
[ -n "$DEVICE" ] || { echo "error: could not attach the disk image:" >&2; echo "$ATTACH_OUT" >&2; exit 1; }
# The mount point is in the attach output; an APFS image mounts on a separate container disk, so
# don't look it up by $DEVICE. Otherwise wait for /Volumes/$VOL to appear.
MNT=$(echo "$ATTACH_OUT" | grep -o '/Volumes/.*$' | sed -E 's/[[:space:]]+$//' | head -1 || true)
if [ -z "$MNT" ]; then
  for _ in $(seq 1 20); do [ -d "/Volumes/$VOL" ] && break; sleep 0.5; done
  MNT="/Volumes/$VOL"
fi
[ -d "$MNT" ] || { echo "error: the disk image did not mount (expected $MNT). It said:" >&2; echo "$ATTACH_OUT" >&2; exit 1; }
[ "$(basename "$MNT")" = "$VOL" ] || { echo "error: the image mounted as $MNT, not /Volumes/$VOL (eject the other $VOL volume)" >&2; exit 1; }
mdutil -i off "$MNT" >/dev/null 2>&1 || true

# Contents: the app, an Applications shortcut, and the background in a hidden folder.
ditto "$APP" "$MNT/$APP_NAME"
ln -s /Applications "$MNT/Applications"
BG_FILE=""
if [ "${DMG_LAYOUT:-1}" != 0 ]; then
  [ -f "$BG" ] || { echo "error: no background image at $BG (set DMG_BACKGROUND, or DMG_LAYOUT=0)" >&2; exit 1; }
  mkdir "$MNT/.background"
  if [ -f "$BG2X" ] && command -v tiffutil >/dev/null; then
    # one file with both sizes: Finder picks the sharp one on Retina screens
    tiffutil -cathidpicheck "$BG" "$BG2X" -out "$MNT/.background/background.tiff" >/dev/null 2>&1
    BG_FILE="background.tiff"
  else
    cp "$BG" "$MNT/.background/background.png"
    BG_FILE="background.png"
  fi
fi

# Lay out the window with Finder; it saves the layout in .DS_Store on the volume.
if [ -n "$BG_FILE" ]; then
  echo "==> laying out the window in Finder…"
  if ! osascript <<APPLESCRIPT
tell application "Finder"
  tell disk "$VOL"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {100, 100, $((100 + WIN_W)), $((100 + WIN_H))}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 13
    set background picture of opts to file ".background:$BG_FILE"
    set position of item "$APP_NAME" of container window to {160, 200}
    set position of item "Applications" of container window to {480, 200}
    update without registering applications
    delay 2
    close
  end tell
end tell
APPLESCRIPT
  then
    echo "error: Finder could not lay out the window. Allow Terminal to control Finder in System Settings" >&2
    echo "       → Privacy & Security → Automation, then run this again (or DMG_LAYOUT=0 for no background)." >&2
    exit 1
  fi
  for _ in $(seq 1 20); do [ -f "$MNT/.DS_Store" ] && break; sleep 0.5; done
  [ -f "$MNT/.DS_Store" ] || echo "warning: Finder did not save the window layout (.DS_Store); the background may not show" >&2
fi
chmod -Rf go-w "$MNT" 2>/dev/null || true
sync

# Eject (there is no `diskutil image detach`), then compress to a read-only .dmg.
eject_device "$DEVICE" || { echo "error: could not eject $DEVICE" >&2; exit 1; }
DEVICE=""
rm -f "$DMG_FILE"
if [ "$IMG" = diskutil ]; then
  diskutil image create from "$RW" "$DMG_FILE" --format UDZO >/dev/null
else
  hdiutil convert "$RW" -quiet -format UDZO -imagekey zlib-level=9 -o "$DMG_FILE"
fi
[ -f "$DMG_FILE" ] || { echo "error: could not compress $DMG_FILE" >&2; exit 1; }
echo "Built: $DMG_FILE"
