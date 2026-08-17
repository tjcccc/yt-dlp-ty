use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::commands::binaries::resolve_configured;
use crate::commands::config::load_config;
use crate::ytdlp::args::{build_probe_args, parse_parameters};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRequest {
    pub urls: Vec<String>,
    pub parameters: String,
}

/// One selectable row of the format table, mirroring the columns yt-dlp's
/// own `-F` output shows.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatEntry {
    pub format_id: String,
    pub ext: String,
    pub resolution: String,
    pub fps: Option<f64>,
    pub filesize: Option<u64>,
    pub tbr: Option<f64>,
    pub proto: String,
    pub vcodec: String,
    pub acodec: String,
}

/// Probe result for one input URL. `error` is per-URL rather than failing
/// the whole batch: one dead link among five shouldn't block choosing
/// formats for the other four.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFormats {
    pub url: String,
    pub title: String,
    pub video_id: String,
    pub formats: Vec<FormatEntry>,
    pub error: Option<String>,
}

impl VideoFormats {
    fn failed(url: &str, error: String) -> Self {
        Self {
            url: url.to_string(),
            title: url.to_string(),
            video_id: String::new(),
            formats: Vec::new(),
            error: Some(error),
        }
    }
}

fn as_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn as_string(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

/// yt-dlp usually provides `resolution` directly, but older/odd extractors
/// only set width/height, and audio-only entries set neither.
fn resolution_of(format: &Value) -> String {
    if let Some(res) = format.get("resolution").and_then(Value::as_str) {
        return res.to_string();
    }
    match (as_f64(format, "width"), as_f64(format, "height")) {
        (Some(w), Some(h)) => format!("{w:.0}x{h:.0}"),
        _ => "unknown".to_string(),
    }
}

fn parse_format(format: &Value) -> Option<FormatEntry> {
    let format_id = format.get("format_id").and_then(Value::as_str)?.to_string();

    // Skip storyboards/thumbnail sheets: yt-dlp lists them in `-F`, but they
    // carry neither a video nor an audio stream, so picking one downloads a
    // grid of preview images instead of the video. Note this tests for the
    // literal "none" — an extractor that reports "unknown" codecs is a real
    // stream with missing metadata and must stay selectable.
    let vcodec = as_string(format, "vcodec", "none");
    let acodec = as_string(format, "acodec", "none");
    if vcodec == "none" && acodec == "none" {
        return None;
    }

    Some(FormatEntry {
        format_id,
        ext: as_string(format, "ext", "?"),
        resolution: resolution_of(format),
        fps: as_f64(format, "fps"),
        // `filesize` is exact but frequently null for adaptive streams;
        // `filesize_approx` is yt-dlp's own estimate and is what its `-F`
        // table falls back to (shown there with a `~`).
        filesize: as_f64(format, "filesize")
            .or_else(|| as_f64(format, "filesize_approx"))
            .map(|f| f as u64),
        tbr: as_f64(format, "tbr"),
        proto: as_string(format, "protocol", "?"),
        vcodec,
        acodec,
    })
}

fn probe_one(ytdlp: &Path, parameters: &[String], proxy: &str, url: &str) -> VideoFormats {
    let args = build_probe_args(parameters, Some(proxy), url);

    let output = match Command::new(ytdlp).args(&args).output() {
        Ok(output) => output,
        Err(e) => return VideoFormats::failed(url, format!("failed to run yt-dlp: {e}")),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Only the tail is useful — yt-dlp's stderr leads with extractor
        // chatter and warnings before the actual failure.
        let message = stderr
            .lines()
            .filter(|l| !l.trim().is_empty())
            .last()
            .unwrap_or("yt-dlp exited with an error")
            .to_string();
        return VideoFormats::failed(url, message);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // `-j` emits one JSON document per line, one per video. A playlist URL
    // therefore yields several; the picker is one row per *URL* (matching
    // how a job is spawned), so the first document supplies the format list
    // and the extra count is surfaced in the title instead of being hidden.
    let docs: Vec<Value> = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let Some(first) = docs.first() else {
        return VideoFormats::failed(url, "yt-dlp returned no video metadata".to_string());
    };

    let mut title = as_string(first, "title", url);
    if docs.len() > 1 {
        title = format!("{title} (+{} more in playlist)", docs.len() - 1);
    }

    let formats: Vec<FormatEntry> = first
        .get("formats")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(parse_format).collect())
        .unwrap_or_default();

    if formats.is_empty() {
        return VideoFormats::failed(url, "no downloadable formats found".to_string());
    }

    VideoFormats {
        url: url.to_string(),
        title,
        video_id: as_string(first, "id", ""),
        formats,
        error: None,
    }
}

/// Fetches available formats for each URL without downloading anything.
///
/// Runs on a blocking thread: each probe spawns a real yt-dlp process that
/// takes seconds, and a sync command would hold the UI thread for the whole
/// batch. URLs are probed in parallel but chunked by the same concurrency
/// setting downloads use, so a large paste doesn't fork dozens of processes
/// at once.
#[tauri::command]
pub async fn probe_formats(app: AppHandle, request: ProbeRequest) -> Result<Vec<VideoFormats>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let parameters = parse_parameters(&request.parameters)?;
        let config = load_config(&app);
        let ytdlp = resolve_configured(&app, "yt-dlp");
        let chunk_size = (config.concurrency.max(1) as usize).min(8);

        let mut results: Vec<VideoFormats> = Vec::with_capacity(request.urls.len());
        for chunk in request.urls.chunks(chunk_size) {
            let probed: Vec<VideoFormats> = std::thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|url| scope.spawn(|| probe_one(&ytdlp, &parameters, &config.proxy, url)))
                    .collect();
                handles
                    .into_iter()
                    .zip(chunk)
                    .map(|(handle, url)| {
                        handle
                            .join()
                            .unwrap_or_else(|_| VideoFormats::failed(url, "probe thread panicked".to_string()))
                    })
                    .collect()
            });
            results.extend(probed);
        }
        Ok(results)
    })
    .await
    .map_err(|e| format!("probe task failed: {e}"))?
}
