# Wiener DL

Paste URL → MP4. A simple macOS video downloader by Wiener Labs.

- Browser-only? No. You can't reliably download from YouTube/TikTok/etc. inside a browser (CORS + signed URLs + cipher signatures). This is a small native app that bundles the gold-standard `yt-dlp` + `ffmpeg` and runs them locally.
- Server? No. Nothing leaves your Mac. The only outside call the app makes on its own is the yt-dlp update check on GitHub at launch.
- Sites? 1800+. Anything `yt-dlp` supports.
- Up to date? Sites change their players often, so the app keeps its own copy of `yt-dlp` and updates it at launch. The update is checked against the SHA-256 sums yt-dlp publishes with every release.

## Install

Download the `.dmg` for your Mac from [Releases](https://github.com/wienerlabs/wiener-dl-mac/releases/latest):

- Apple Silicon (M1 and newer): [Wiener-DL-mac-apple-silicon.dmg](https://github.com/wienerlabs/wiener-dl-mac/releases/latest/download/Wiener-DL-mac-apple-silicon.dmg)
- Intel: [Wiener-DL-mac-intel.dmg](https://github.com/wienerlabs/wiener-dl-mac/releases/latest/download/Wiener-DL-mac-intel.dmg)

Open it and drag `Wiener DL.app` into `Applications`.

The app is not notarized by Apple yet, so macOS blocks the first launch:

- macOS 15 and newer: open the app once, then go to System Settings → Privacy & Security, click "Open Anyway" next to Wiener DL and confirm.
- macOS 14 and older: right-click the app, choose Open, then Open again.

After that the app launches normally.

## Browser login

Instagram, private and age-restricted videos need a signed-in session. Pick the browser you are signed in with under "Browser login" and `yt-dlp` reads its cookies on your Mac (`--cookies-from-browser`). Cookies are only sent to the site you download from. Chrome, Brave and Edge ask for Keychain access the first time; Safari needs Full Disk Access for Wiener DL in System Settings.

## Build from source

Requirements: Rust 1.88+, Node 22+, pnpm 11.

`scripts/fetch-binaries.sh` downloads the sidecars (`yt-dlp`, `ffmpeg`, `ffprobe` and `deno`) and checks `yt-dlp` and `deno` against their published SHA-256 sums. `scripts/package-mac.sh` signs the app ad hoc with the entitlements in `src-tauri/entitlements` and builds the `.dmg`.

```bash
pnpm install
bash scripts/fetch-binaries.sh
NPM_CONFIG_VERIFY_DEPS_BEFORE_RUN=false pnpm exec tauri build --target aarch64-apple-darwin --bundles app
bash scripts/package-mac.sh aarch64-apple-darwin release/Wiener-DL-mac-apple-silicon.dmg
```

For the Intel build, run the fetch script with `FETCH_X86=1` and use `x86_64-apple-darwin` as the target. Pushing a `v*` tag builds both on GitHub Actions and publishes the release.

## How it works

Tauri 2 wraps a single-page vanilla TS + Vite frontend in a system webview. At launch the Rust backend copies the bundled `yt-dlp` to `~/Library/Application Support/com.wienerlabs.wienerdl/bin` and runs `yt-dlp -U` on that copy. Downloads use the updated copy, or the bundled one when the update cannot run. Each download spawns `yt-dlp` with:

- format selector tuned to your quality/format pick (e.g. `bv*[height<=1080]+ba/b[height<=1080]` for 1080p)
- `--newline --no-colors --no-warnings` so we can stream progress lines
- output template: `~/Downloads/%(title).200B [%(id)s].%(ext)s`
- `--ffmpeg-location` pointing to the bundled ffmpeg
- `--js-runtimes deno:<bundled deno>` for YouTube's JavaScript challenges
- `--cookies-from-browser <browser>` when "Browser login" is set

Stdout lines are parsed for `[download] XX.X%` progress and `Destination:` paths, then streamed to the frontend via Tauri events. The frontend shows a progress bar and reveals the file in Finder when done.

## License

MIT for app code. The bundled binaries keep their own licenses:

- `yt-dlp`: Unlicense
- `deno`: MIT
- `ffmpeg` / `ffprobe`: LGPL/GPL (the static builds we ship are LGPL-only by default)
