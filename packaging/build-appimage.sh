#!/usr/bin/env bash
# Build a portable AppImage of the Phanes desktop app (phanes-ui).
#
# Requires: a Rust toolchain, `linuxdeploy`, and `appimagetool` on PATH, plus
# FUSE for running the result. Produces dist/Phanes-<version>-x86_64.AppImage.
#
# The GUI is built with `--features ui,enrich`, so the AI features (Scan + AI,
# Ask, bridges) work when a local OpenAI-compatible model server is running;
# without one, the deterministic features still work (the model layer is opt-in).
set -euo pipefail

cd "$(dirname "$0")/.."   # repo root
ROOT="$(pwd)"
PKG="$ROOT/packaging/appimage"
APPDIR="$ROOT/target/appimage/Phanes.AppDir"
OUT="$ROOT/dist"

VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
ARCH="$(uname -m)"

echo "==> Building phanes-ui (release, features ui,enrich)"
cargo build --release --features ui,enrich --bin phanes-ui

echo "==> Assembling AppDir at $APPDIR"
rm -rf "$APPDIR"
mkdir -p "$APPDIR"

# linuxdeploy populates the AppDir from the binary + desktop + icon, bundling the
# binary's shared-library dependencies (it correctly leaves GL/driver libs to the
# host). It also writes AppRun and the top-level icon/desktop links.
export VERSION ARCH
linuxdeploy \
  --appdir "$APPDIR" \
  --executable "$ROOT/target/release/phanes-ui" \
  --desktop-file "$PKG/phanes.desktop" \
  --icon-file "$PKG/phanes.png" \
  --output appimage

mkdir -p "$OUT"
# linuxdeploy emits Phanes-<ARCH>.AppImage in the CWD; normalise the name.
SRC="$(ls -1t Phanes*-"$ARCH".AppImage Phanes*."$ARCH".AppImage 2>/dev/null | head -1 || true)"
[ -z "${SRC:-}" ] && SRC="$(ls -1t Phanes*.AppImage 2>/dev/null | head -1)"
DEST="$OUT/Phanes-$VERSION-$ARCH.AppImage"
mv -f "$SRC" "$DEST"
chmod +x "$DEST"

echo "==> Done: $DEST"
ls -lh "$DEST"
