#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET="$1"
OUT="$2"
APP="src-tauri/target/$TARGET/release/bundle/macos/Wiener DL.app"
ENT_SIDE="src-tauri/entitlements/sidecar.plist"
ENT_APP="src-tauri/entitlements/app.plist"

xattr -cr "$APP" || true
# Sign sidecars first with library-validation-disabled entitlement
# (yt-dlp is a PyInstaller bundle; its embedded Python.framework
#  has a different Team ID than our ad-hoc resign.)
for bin in "$APP/Contents/MacOS/yt-dlp" "$APP/Contents/MacOS/ffmpeg" "$APP/Contents/MacOS/ffprobe" "$APP/Contents/MacOS/deno"; do
  if [ -f "$bin" ]; then
    codesign --force --options runtime --entitlements "$ENT_SIDE" --sign - "$bin"
  fi
done
# Sign the main binary, then the bundle WITHOUT --deep so the
# per-binary entitlements above are preserved.
codesign --force --options runtime --entitlements "$ENT_APP" --sign - "$APP/Contents/MacOS/wiener-dl"
codesign --force --options runtime --entitlements "$ENT_APP" --sign - "$APP"
codesign --verify --strict --verbose=2 "$APP"
codesign --display --entitlements - "$APP/Contents/MacOS/yt-dlp" | grep -A1 "disable-library-validation" || true

STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
for attempt in 1 2 3 4 5; do
  if hdiutil create -volname "Wiener DL" -srcfolder "$STAGE" -ov -format UDZO "$OUT"; then
    break
  fi
  if [ "$attempt" = 5 ]; then
    exit 1
  fi
  sleep $((attempt * 5))
done
rm -rf "$STAGE"
codesign --force --sign - "$OUT"
ls -lh "$OUT"
