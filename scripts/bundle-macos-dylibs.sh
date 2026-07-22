#!/usr/bin/env bash
# Bundle libmpv and its whole dependency chain into a built .app, then prove it worked.
#
# Unlike Windows — where libmpv-2.dll is self-contained and bundling is one file copy — the
# Homebrew libmpv pulls in a large graph of dylibs (ffmpeg, libass, harfbuzz, luajit, …), each
# recorded as an ABSOLUTE Homebrew path. A user's Mac has no /opt/homebrew, so an unpatched
# .app dies at launch. `dylibbundler` copies the whole transitive graph into the bundle and
# rewrites every load command to @executable_path-relative form.
#
# This CANNOT be folded into `tauri build`: the DMG bundler regenerates the .app unless it was
# built in the same invocation (crates/tauri-bundler/src/bundle/macos/dmg/mod.rs — it calls
# app::bundle_project() whenever MacOsBundle is not among the bundles of THAT run), so
# `--bundles app` → patch → `--bundles dmg` would silently discard everything done here. The
# release workflow builds `--bundles app`, runs this, then makes the DMG itself.
#
# Usage:  scripts/bundle-macos-dylibs.sh "path/to/Freally Player.app"
set -euo pipefail

APP="${1:?usage: bundle-macos-dylibs.sh <path to .app>}"
LIBS_DIR="$APP/Contents/libs"

[ -d "$APP" ] || { echo "::error::no .app at $APP"; exit 1; }

# Ask the bundle which binary is its entry point rather than hardcoding a name — it follows
# `mainBinaryName`, so a config change here would otherwise fail confusingly at runtime.
EXEC_NAME=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")
BINARY="$APP/Contents/MacOS/$EXEC_NAME"
[ -f "$BINARY" ] || { echo "::error::no executable at $BINARY"; exit 1; }

echo "--- before: libmpv reference in the executable ---"
otool -L "$BINARY" | grep -i mpv || { echo "::error::the binary does not link libmpv at all"; exit 1; }

# -b bundle the dependencies, -x the executable to fix, -d where the copies go, -p what the
# rewritten load commands point at, -of overwrite files already there, -cd create the
# destination directory (without it dylibbundler collects the whole graph and only then
# refuses, because Contents/libs does not exist yet). dylibbundler walks the dependency graph
# recursively, which is the part worth not hand-rolling.
echo "--- bundling dependencies ---"
dylibbundler -b -of -cd \
  -x "$BINARY" \
  -d "$LIBS_DIR" \
  -p "@executable_path/../libs/"

# install_name_tool invalidates code signatures, and on Apple Silicon an invalid signature is
# fatal: the app is killed at launch (SIGKILL) rather than merely warned about. Re-sign the
# whole bundle ad-hoc. This is not a Developer ID signature — the release is unsigned until
# certificates exist — it is the minimum that lets an arm64 binary run at all.
echo "--- re-signing (ad-hoc) ---"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# Verification, not decoration: the failure mode this guards against only shows up on a machine
# without Homebrew, which is exactly the machine we cannot test on. Any surviving absolute
# reference to a Homebrew/local prefix means the app would die at launch for a real user.
echo "--- verifying no absolute local paths survive ---"
leaked=$(
  {
    otool -L "$BINARY"
    find "$LIBS_DIR" -name '*.dylib' -exec otool -L {} \;
  } | grep -E '^\s+(/opt/homebrew|/usr/local|/opt/local)' || true
)
if [ -n "$leaked" ]; then
  echo "::error::these load commands still point outside the bundle:"
  echo "$leaked"
  exit 1
fi

# The point of the whole exercise: libmpv must now resolve from inside the bundle.
if ! otool -L "$BINARY" | grep -q '@executable_path/../libs/libmpv'; then
  echo "::error::the executable does not reference a bundled libmpv"
  otool -L "$BINARY"
  exit 1
fi

echo "--- after ---"
otool -L "$BINARY" | grep -E 'mpv|libs/' || true
echo "bundled $(find "$LIBS_DIR" -name '*.dylib' | wc -l | tr -d ' ') dylibs into $LIBS_DIR"
