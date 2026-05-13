import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

type ProgressPayload =
  | { kind: "stage"; message: string }
  | { kind: "progress"; percent: number; speed?: string; eta?: string; downloaded?: string; total?: string }
  | { kind: "log"; line: string }
  | { kind: "done"; path: string }
  | { kind: "error"; message: string };

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const urlInput = $<HTMLInputElement>("url");
const qualitySel = $<HTMLSelectElement>("quality");
const formatSel = $<HTMLSelectElement>("format");
const downloadBtn = $<HTMLButtonElement>("download");
const statusBox = $<HTMLElement>("status");
const statusLine = $<HTMLElement>("status-line");
const progressFill = $<HTMLElement>("progress-fill");
const logBox = $<HTMLPreElement>("log");
const actionsBox = $<HTMLElement>("status-actions");
const revealBtn = $<HTMLButtonElement>("reveal");
const openFileBtn = $<HTMLButtonElement>("open-file");
const resetBtn = $<HTMLButtonElement>("reset");
const openDownloadsBtn = $<HTMLButtonElement>("open-downloads");
const siteLink = $<HTMLAnchorElement>("site-link");

let savedPath: string | null = null;
let unlisten: UnlistenFn | null = null;

siteLink.href = "https://wieners-tools.vercel.app/tr/tools/video-downloader/";

function setStatus(kind: "info" | "error" | "success" | "indeterminate", message: string, percent?: number) {
  statusBox.hidden = false;
  statusBox.classList.remove("is-error", "is-success", "is-indeterminate");
  if (kind === "error") statusBox.classList.add("is-error");
  else if (kind === "success") statusBox.classList.add("is-success");
  else if (kind === "indeterminate") statusBox.classList.add("is-indeterminate");
  statusLine.textContent = message;
  if (typeof percent === "number") progressFill.style.width = `${Math.max(0, Math.min(100, percent))}%`;
}

function appendLog(line: string) {
  logBox.hidden = false;
  // Keep tail only — last 500 chars
  const next = (logBox.textContent ?? "") + line + "\n";
  logBox.textContent = next.length > 8000 ? next.slice(-8000) : next;
  logBox.scrollTop = logBox.scrollHeight;
}

function lockForm(locked: boolean) {
  urlInput.disabled = locked;
  qualitySel.disabled = locked;
  formatSel.disabled = locked;
  downloadBtn.disabled = locked;
}

function resetAll() {
  savedPath = null;
  statusBox.hidden = true;
  statusBox.classList.remove("is-error", "is-success", "is-indeterminate");
  actionsBox.hidden = true;
  logBox.hidden = true;
  logBox.textContent = "";
  progressFill.style.width = "0";
  urlInput.value = "";
  urlInput.focus();
}

async function startDownload() {
  const url = urlInput.value.trim();
  if (!url || !/^https?:\/\//.test(url)) {
    setStatus("error", "Enter a valid URL (https://...)");
    return;
  }
  const quality = qualitySel.value;
  const format = formatSel.value;

  // Auto-pick audio format if quality = audio
  let effectiveFormat = format;
  if (quality === "audio" && format !== "mp3" && format !== "m4a") {
    effectiveFormat = "m4a";
  }
  if ((format === "mp3" || format === "m4a") && quality !== "audio") {
    qualitySel.value = "audio";
  }

  lockForm(true);
  actionsBox.hidden = true;
  logBox.hidden = true;
  logBox.textContent = "";
  setStatus("indeterminate", "Connecting…", 0);

  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  unlisten = await listen<ProgressPayload>("download-progress", (event) => {
    const p = event.payload;
    if (p.kind === "stage") setStatus("indeterminate", p.message);
    else if (p.kind === "progress") {
      const parts = [`${p.percent.toFixed(1)}%`];
      if (p.downloaded && p.total) parts.push(`${p.downloaded} / ${p.total}`);
      if (p.speed) parts.push(p.speed);
      if (p.eta) parts.push(`ETA ${p.eta}`);
      setStatus("info", parts.join("  ·  "), p.percent);
    } else if (p.kind === "log") appendLog(p.line);
    else if (p.kind === "done") {
      savedPath = p.path;
      setStatus("success", `Saved · ${p.path.split("/").pop()}`, 100);
      actionsBox.hidden = false;
      lockForm(false);
    } else if (p.kind === "error") {
      setStatus("error", p.message);
      actionsBox.hidden = false;
      revealBtn.hidden = true;
      openFileBtn.hidden = true;
      lockForm(false);
    }
  });

  try {
    await invoke("download_video", { url, quality, format: effectiveFormat });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    setStatus("error", msg);
    actionsBox.hidden = false;
    revealBtn.hidden = true;
    openFileBtn.hidden = true;
    lockForm(false);
  }
}

downloadBtn.addEventListener("click", () => {
  void startDownload();
});

urlInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    void startDownload();
  }
});

revealBtn.addEventListener("click", () => {
  if (savedPath) void revealItemInDir(savedPath);
});

openFileBtn.addEventListener("click", () => {
  if (savedPath) void openPath(savedPath);
});

resetBtn.addEventListener("click", () => resetAll());

openDownloadsBtn.addEventListener("click", async () => {
  const home = await invoke<string>("downloads_dir");
  void openPath(home);
});

// Auto-paste from clipboard on focus if it looks like a URL and the field is empty
window.addEventListener("focus", async () => {
  try {
    if (urlInput.value.trim()) return;
    const clip = await navigator.clipboard.readText();
    if (clip && /^https?:\/\//.test(clip)) {
      urlInput.value = clip.trim();
    }
  } catch {
    // Permission denied is fine; user can paste manually
  }
});

// Quality ↔ format coherence
qualitySel.addEventListener("change", () => {
  if (qualitySel.value === "audio") {
    formatSel.value = "m4a";
  } else if (formatSel.value === "mp3" || formatSel.value === "m4a") {
    formatSel.value = "mp4";
  }
});
formatSel.addEventListener("change", () => {
  if ((formatSel.value === "mp3" || formatSel.value === "m4a") && qualitySel.value !== "audio") {
    qualitySel.value = "audio";
  }
});
