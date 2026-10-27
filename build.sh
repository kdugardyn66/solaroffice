#!/usr/bin/env bash
# Builds the macOS .dmg for SolarOffice.
#
# Works from either place:
#   - inside the source folder (solaroffice-main/build.sh), or
#   - next to it, in the folder that holds it (the source folder may be named solaroffice or
#     solaroffice-main, as a GitHub zip download unpacks):
#
#       build.sh
#       solaroffice-main/
#
# Usage:
#   ./build.sh              build the .dmg (universal: Apple silicon + Intel)
#   ./build.sh --native     build the .dmg for this Mac only (faster)
#   ./build.sh --app        build SolarOffice.app only (no .dmg); combine with --native
#   ./build.sh --run        build and run the app directly (no bundle)
#
# The finished .app and .dmg end up in <source folder>/dist/.
# Environment: SIGN_IDENTITY="Developer ID Application: …" signs with a real identity (default
# ad-hoc); DMG_LAYOUT=0 skips the Finder window layout.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP="solaroffice"

usage() {
  echo "Usage: $0 [--native] [--app] | --run" >&2
  exit 2
}

RUN=0
export ARCH="${ARCH:-universal}"
export DMG="${DMG:-1}"
for arg in "$@"; do
  case "$arg" in
    --run) RUN=1 ;;
    --native) ARCH=native ;;
    --app) DMG=0 ;;
    -h | --help) usage ;;
    *) echo "error: unknown option '$arg'" >&2; usage ;;
  esac
done

if [ "$(uname -s)" != "Darwin" ] && [ "$RUN" = 0 ]; then
  echo "error: packaging needs macOS (use --run elsewhere)" >&2
  exit 1
fi
command -v cargo >/dev/null || { echo "error: cargo not found; install Rust from https://rustup.rs" >&2; exit 1; }

# Reads the version from Cargo.toml: [workspace.package], or else [package].
cargo_version() {
  local section v
  for section in "[workspace.package]" "[package]"; do
    v="$(awk -v want="$section" '
      /^\[/ { in_sec = ($0 == want) }
      in_sec && /^version[[:space:]]*=/ { gsub(/.*=[[:space:]]*"|".*/, ""); print; exit }
    ' "$1/Cargo.toml" 2>/dev/null)"
    if [ -n "$v" ]; then echo "$v"; return; fi
  done
}

# The source folder: this one (build.sh inside it), or a plain / -main named folder beside it.
DIR=""
for candidate in "$ROOT" "$ROOT/$APP" "$ROOT/$APP-main"; do
  if [ -f "$candidate/Cargo.toml" ] && [ -d "$candidate/apps/$APP" ]; then DIR="$candidate"; break; fi
done
if [ -z "$DIR" ]; then
  echo "error: no SolarOffice source here, or in a folder '$APP' or '$APP-main' in $ROOT" >&2
  echo "       folders there now:" >&2
  ls -1 "$ROOT" | sed 's/^/         /' >&2
  exit 1
fi
echo "==> using $DIR"

# Rust 1.88+ (Dioxus 0.7).
rust_ver="$(rustc --version 2>/dev/null | awk '{print $2}')"
if [ -n "$rust_ver" ] && [ "$(printf '%s\n' 1.88.0 "$rust_ver" | sort -V | head -1)" != 1.88.0 ]; then
  echo "error: Rust $rust_ver is too old; SolarOffice needs 1.88 or newer (run: rustup update)" >&2
  exit 1
fi

# Keep the multi-gigabyte target folder out of iCloud Drive (iCloud skips *.nosync).
if [[ "$DIR" == *"/Mobile Documents/"* ]] && [ ! -e "$DIR/target" ]; then
  mkdir -p "$DIR/target.nosync" && ln -s target.nosync "$DIR/target"
  echo "==> iCloud Drive folder: build output goes to target.nosync (not synced)"
fi

if [ "$RUN" = 1 ]; then
  cd "$DIR" && exec cargo run --release -p "$APP"
fi

chmod +x "$DIR"/packaging/macos/*.sh 2>/dev/null || true
version="$(cargo_version "$DIR")"
(
  cd "$DIR" || exit 1
  if [ -z "${VERSION:-}" ] && [ -n "$version" ]; then export VERSION="$version"; fi
  ./packaging/macos/package.sh
) || { echo "FAILED: $APP (scroll up for the error)" >&2; exit 1; }

echo "==> $APP $version done:"
find "$DIR/dist" -maxdepth 1 \( -name '*.dmg' -o -name '*.app' \) -exec ls -ldh {} + 2>/dev/null || true
