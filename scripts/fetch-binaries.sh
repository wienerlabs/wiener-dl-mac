#!/usr/bin/env bash
# Download yt-dlp + ffmpeg binaries and place them with Tauri sidecar naming.
# Tauri expects: src-tauri/binaries/<name>-<target-triple>
# We provide both Apple Silicon (aarch64-apple-darwin) and Intel (x86_64-apple-darwin).
#
# yt-dlp ships a universal2 binary — same file works on both architectures.
# ffmpeg/ffprobe come from evermeet.cx as separate arm64 and x86_64 static builds.

set -euo pipefail

cd "$(dirname "$0")/.."

BIN_DIR="src-tauri/binaries"
mkdir -p "$BIN_DIR"

YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
FFMPEG_ARM_URL="https://www.osxexperts.net/ffmpeg711arm.zip"
FFPROBE_ARM_URL="https://www.osxexperts.net/ffprobe711arm.zip"
FFMPEG_X86_URL="${FFMPEG_X86_URL:-https://evermeet.cx/ffmpeg/getrelease/zip}"
FFPROBE_X86_URL="${FFPROBE_X86_URL:-https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip}"

FETCH_X86="${FETCH_X86:-0}"

echo "→ Downloading yt-dlp (universal)…"
curl -fL --retry 3 -o "$BIN_DIR/yt-dlp_macos" "$YTDLP_URL"
chmod +x "$BIN_DIR/yt-dlp_macos"

# Sidecar naming: same binary for both architectures (yt-dlp_macos is universal2)
cp "$BIN_DIR/yt-dlp_macos" "$BIN_DIR/yt-dlp-aarch64-apple-darwin"
cp "$BIN_DIR/yt-dlp_macos" "$BIN_DIR/yt-dlp-x86_64-apple-darwin"
chmod +x "$BIN_DIR/yt-dlp-aarch64-apple-darwin" "$BIN_DIR/yt-dlp-x86_64-apple-darwin"

fetch_ffmpeg() {
  local url="$1"
  local out="$2"
  local tmp
  tmp=$(mktemp -d)
  echo "→ Downloading $(basename "$out") from $url…"
  curl -fL --retry 3 -o "$tmp/x.zip" "$url"
  unzip -o -q "$tmp/x.zip" -d "$tmp"
  # The zip contains a single binary — skip __MACOSX metadata and zip itself
  local extracted
  extracted=$(find "$tmp" -type f \
    -not -path "*/__MACOSX/*" \
    -not -name "._*" \
    -not -name "*.zip" \
    -not -name ".DS_Store" \
    | head -n 1)
  if [ -z "$extracted" ] || [ ! -s "$extracted" ]; then
    echo "ERROR: could not find binary inside zip from $url" >&2
    ls -la "$tmp" >&2
    rm -rf "$tmp"
    return 1
  fi
  cp "$extracted" "$out"
  chmod +x "$out"
  # Strip macOS quarantine xattr so the binary runs without prompts
  xattr -c "$out" 2>/dev/null || true
  rm -rf "$tmp"
}

fetch_ffmpeg "$FFMPEG_ARM_URL"  "$BIN_DIR/ffmpeg-aarch64-apple-darwin"
fetch_ffmpeg "$FFPROBE_ARM_URL" "$BIN_DIR/ffprobe-aarch64-apple-darwin"

if [ "$FETCH_X86" = "1" ]; then
  fetch_ffmpeg "$FFMPEG_X86_URL"  "$BIN_DIR/ffmpeg-x86_64-apple-darwin"
  fetch_ffmpeg "$FFPROBE_X86_URL" "$BIN_DIR/ffprobe-x86_64-apple-darwin"
else
  echo "→ Skipping x86_64 ffmpeg (set FETCH_X86=1 to include). yt-dlp already shared via universal binary."
fi

echo ""
echo "✓ Binaries ready:"
ls -lh "$BIN_DIR" | grep -v '\.gitkeep$' | grep -v 'yt-dlp_macos$' || true
