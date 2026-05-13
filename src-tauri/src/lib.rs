use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;
use tokio::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "snake_case")]
enum Progress {
    Stage {
        message: String,
    },
    Progress {
        percent: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        speed: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        eta: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        downloaded: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<String>,
    },
    Log {
        line: String,
    },
    Done {
        path: String,
    },
    Error {
        message: String,
    },
}

#[derive(Default)]
struct DownloadState {
    running: Mutex<bool>,
}

fn downloads_dir_path() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .map(|h| h.join("Downloads"))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

#[tauri::command]
fn downloads_dir() -> String {
    downloads_dir_path().to_string_lossy().into_owned()
}

fn quality_format_string(quality: &str, format: &str) -> String {
    // yt-dlp -f format selector
    // For audio-only, we pick best audio and let postprocessor convert
    if format == "mp3" || format == "m4a" {
        return "bestaudio/best".to_string();
    }
    match quality {
        "audio" => "bestaudio/best".to_string(),
        "best" => "bv*+ba/b".to_string(),
        "1080" => "bv*[height<=1080]+ba/b[height<=1080]".to_string(),
        "720" => "bv*[height<=720]+ba/b[height<=720]".to_string(),
        "480" => "bv*[height<=480]+ba/b[height<=480]".to_string(),
        _ => "bv*+ba/b".to_string(),
    }
}

fn parse_progress_line(line: &str) -> Option<Progress> {
    // yt-dlp --newline emits lines like:
    // [download]  42.3% of   12.34MiB at 1.23MiB/s ETA 00:07
    // [download] 100% of   12.34MiB in 00:09
    let trimmed = line.trim_start();
    if !trimmed.starts_with("[download]") {
        return None;
    }
    let rest = trimmed.trim_start_matches("[download]").trim_start();

    // Try percent
    let percent_end = rest.find('%')?;
    let percent_str = rest[..percent_end].trim();
    let percent: f64 = percent_str.parse().ok()?;

    let after_pct = &rest[percent_end + 1..];

    // Try size/speed/eta
    let mut total: Option<String> = None;
    let mut speed: Option<String> = None;
    let mut eta: Option<String> = None;

    // "of   SIZE at SPEED ETA TIME"
    if let Some(of_idx) = after_pct.find("of ") {
        let after_of = &after_pct[of_idx + 3..];
        let parts: Vec<&str> = after_of.split_whitespace().collect();
        if let Some(first) = parts.first() {
            total = Some((*first).to_string());
        }
        for i in 0..parts.len() {
            if parts[i] == "at" && i + 1 < parts.len() {
                speed = Some(parts[i + 1].to_string());
            }
            if parts[i] == "ETA" && i + 1 < parts.len() {
                eta = Some(parts[i + 1].to_string());
            }
        }
    }

    Some(Progress::Progress {
        percent,
        speed,
        eta,
        downloaded: None,
        total,
    })
}

fn parse_destination_line(line: &str) -> Option<String> {
    // yt-dlp prints lines like:
    //   [download] Destination: /Users/foo/Downloads/Title [id].mp4
    //   [Merger] Merging formats into "/Users/foo/Downloads/Title [id].mp4"
    //   [ExtractAudio] Destination: /Users/foo/Downloads/Title [id].mp3
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("[download] Destination: ") {
        return Some(rest.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("[ExtractAudio] Destination: ") {
        return Some(rest.to_string());
    }
    if let Some(idx) = trimmed.find("Merging formats into ") {
        let after = &trimmed[idx + "Merging formats into ".len()..];
        let cleaned = after.trim_matches('"').trim();
        return Some(cleaned.to_string());
    }
    // "[download] /path/to/file has already been downloaded"
    if trimmed.starts_with("[download] ") && trimmed.ends_with(" has already been downloaded") {
        let inner = trimmed
            .trim_start_matches("[download] ")
            .trim_end_matches(" has already been downloaded");
        return Some(inner.to_string());
    }
    None
}

#[tauri::command]
async fn download_video(
    app: AppHandle,
    state: tauri::State<'_, DownloadState>,
    url: String,
    quality: String,
    format: String,
) -> Result<(), String> {
    {
        let mut running = state.running.lock().await;
        if *running {
            return Err("A download is already running".into());
        }
        *running = true;
    }

    let result = run_download(&app, &url, &quality, &format).await;

    {
        let mut running = state.running.lock().await;
        *running = false;
    }

    if let Err(ref e) = result {
        let _ = app.emit(
            "download-progress",
            Progress::Error {
                message: e.clone(),
            },
        );
    }
    result
}

async fn run_download(
    app: &AppHandle,
    url: &str,
    quality: &str,
    format: &str,
) -> Result<(), String> {
    let dl_dir = downloads_dir_path();
    if let Err(e) = std::fs::create_dir_all(&dl_dir) {
        return Err(format!("Cannot access Downloads folder: {e}"));
    }

    let output_template = format!("{}/%(title).200B [%(id)s].%(ext)s", dl_dir.to_string_lossy());

    let format_selector = quality_format_string(quality, format);

    let mut args: Vec<String> = vec![
        "--no-playlist".into(),
        "--newline".into(),
        "--no-colors".into(),
        "--no-warnings".into(),
        "-f".into(),
        format_selector,
        "-o".into(),
        output_template,
        "--restrict-filenames".into(),
    ];

    // Container / postprocessing
    match format {
        "mp3" => {
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push("mp3".into());
            args.push("--audio-quality".into());
            args.push("0".into());
        }
        "m4a" => {
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push("m4a".into());
        }
        "mkv" => {
            args.push("--merge-output-format".into());
            args.push("mkv".into());
        }
        "webm" => {
            args.push("--merge-output-format".into());
            args.push("webm".into());
        }
        _ => {
            // mp4 default
            args.push("--merge-output-format".into());
            args.push("mp4".into());
            // Prefer H.264 when going to mp4 to maximize compatibility
            args.push("-S".into());
            args.push("res:1080,fps,codec:h264:m4a".into());
        }
    }

    // Use bundled ffmpeg location (Tauri sidecar resolves to same dir as yt-dlp)
    // yt-dlp searches PATH if not set, but we want a hermetic build.
    if let Some(ff_path) = resolve_sidecar_path(app, "ffmpeg") {
        if let Some(dir) = ff_path.parent() {
            args.push("--ffmpeg-location".into());
            args.push(dir.to_string_lossy().into_owned());
        }
    }

    args.push(url.to_string());

    let _ = app.emit(
        "download-progress",
        Progress::Stage {
            message: "Starting yt-dlp…".into(),
        },
    );

    let sidecar = app
        .shell()
        .sidecar("yt-dlp")
        .map_err(|e| format!("yt-dlp sidecar not found: {e}"))?;

    let (mut rx, _child) = sidecar
        .args(&args)
        .spawn()
        .map_err(|e| format!("Failed to spawn yt-dlp: {e}"))?;

    let mut last_destination: Option<String> = None;
    let mut stderr_buf = String::new();

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes).into_owned();
                let line = line.trim_end_matches('\n').to_string();
                if line.is_empty() {
                    continue;
                }
                if let Some(p) = parse_progress_line(&line) {
                    let _ = app.emit("download-progress", p);
                }
                if let Some(dest) = parse_destination_line(&line) {
                    last_destination = Some(dest);
                }
                let _ = app.emit(
                    "download-progress",
                    Progress::Log { line: line.clone() },
                );
                // Also bubble up stage hints
                if line.starts_with("[ExtractAudio]") {
                    let _ = app.emit(
                        "download-progress",
                        Progress::Stage {
                            message: "Extracting audio…".into(),
                        },
                    );
                } else if line.contains("[Merger]") {
                    let _ = app.emit(
                        "download-progress",
                        Progress::Stage {
                            message: "Merging streams…".into(),
                        },
                    );
                }
            }
            CommandEvent::Stderr(bytes) => {
                let line = String::from_utf8_lossy(&bytes).into_owned();
                stderr_buf.push_str(&line);
                let _ = app.emit(
                    "download-progress",
                    Progress::Log {
                        line: line.trim_end_matches('\n').to_string(),
                    },
                );
            }
            CommandEvent::Terminated(payload) => {
                let code = payload.code.unwrap_or(-1);
                if code == 0 {
                    let path = last_destination
                        .clone()
                        .unwrap_or_else(|| dl_dir.to_string_lossy().into_owned());
                    let _ = app.emit("download-progress", Progress::Done { path });
                    return Ok(());
                } else {
                    let msg = if stderr_buf.is_empty() {
                        format!("yt-dlp exited with code {code}")
                    } else {
                        // Take the last meaningful line of stderr
                        let last = stderr_buf
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .last()
                            .unwrap_or("yt-dlp failed")
                            .to_string();
                        last
                    };
                    return Err(msg);
                }
            }
            _ => {}
        }
    }

    Err("yt-dlp ended without a termination event".into())
}

fn resolve_sidecar_path(app: &AppHandle, name: &str) -> Option<PathBuf> {
    // In bundled builds, sidecars are placed next to the main executable in the .app bundle.
    let resource_dir = app.path().resource_dir().ok()?;
    let candidate = resource_dir.join(name);
    if candidate.exists() {
        return Some(candidate);
    }
    // Dev: sidecar lives in src-tauri/binaries/<name>-<triple>
    let exe = app.path().resolve(name, tauri::path::BaseDirectory::Resource).ok()?;
    if exe.exists() {
        return Some(exe);
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DownloadState::default())
        .invoke_handler(tauri::generate_handler![download_video, downloads_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
