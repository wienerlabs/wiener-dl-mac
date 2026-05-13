# Wiener DL

Paste URL → MP4. A simple macOS video downloader by Wiener Labs.

- Browser-only? No. You can't reliably download from YouTube/TikTok/etc. inside a browser (CORS + signed URLs + cipher signatures). This is a small native app that bundles the gold-standard `yt-dlp` + `ffmpeg` and runs them locally.
- Server? No. Nothing leaves your Mac.
- Sites? 1800+. Anything `yt-dlp` supports.

## Install

Download the latest `.dmg` from [Releases](https://github.com/wienerlabs/wiener-dl-mac/releases/latest), open it, drag `Wiener DL.app` into `Applications`.

On first launch macOS will warn "from an unidentified developer" because the app is not (yet) Apple-notarized. Right-click → Open → Open. After that the app launches normally.

## Build from source

Requirements: Rust 1.88+, Node 22+, pnpm 11.

```bash
pnpm install
bash scripts/fetch-binaries.sh           # download yt-dlp + ffmpeg sidecars
NPM_CONFIG_VERIFY_DEPS_BEFORE_RUN=false pnpm exec tauri build
# Output: src-tauri/target/release/bundle/macos/Wiener DL.app
```

For Intel + Apple Silicon both, set `FETCH_X86=1` before the fetch script.

## How it works

Tauri 2 wraps a single-page vanilla TS + Vite frontend in a system webview. A Rust backend spawns `yt-dlp` as a sidecar child process with these arguments:

- format selector tuned to your quality/format pick (e.g. `bv*[height<=1080]+ba/b[height<=1080]` for 1080p)
- `--newline --no-colors --no-warnings` so we can stream progress lines
- output template: `~/Downloads/%(title).200B [%(id)s].%(ext)s`
- `--ffmpeg-location` pointing to the bundled ffmpeg

Stdout lines are parsed for `[download] XX.X%` progress and `Destination:` paths, then streamed to the frontend via Tauri events. The frontend shows a progress bar and reveals the file in Finder when done.

## License

MIT for app code. The bundled binaries keep their own licenses:

- `yt-dlp` — Unlicense
- `ffmpeg` / `ffprobe` — LGPL/GPL (the static builds we ship are LGPL-only by default)
