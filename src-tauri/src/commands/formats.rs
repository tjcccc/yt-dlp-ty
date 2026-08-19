use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

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
    /// The exact command that was run, shell-quoted so it can be pasted into
    /// a terminal unchanged. Without this the app is a black box when a probe
    /// misbehaves — you can't tell which binary ran, whether the proxy or
    /// cookie flags were applied, or reproduce it outside the app.
    pub command: String,
}

/// Keeps a useful amount of failure output without letting a pathological run
/// stream unbounded text into the UI.
const STDERR_TAIL_LINES: usize = 25;

impl VideoFormats {
    fn failed(url: &str, command: String, error: String) -> Self {
        Self {
            url: url.to_string(),
            title: url.to_string(),
            video_id: String::new(),
            formats: Vec::new(),
            error: Some(error),
            command,
        }
    }
}

/// Renders an invocation the way a shell would accept it back.
fn quote_command(program: &Path, args: &[String]) -> String {
    let mut parts = vec![program.to_string_lossy().to_string()];
    parts.extend(args.iter().cloned());
    shell_words::join(&parts)
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
    // are grids of preview images, so picking one downloads no video at all.
    //
    // Identify them by their mhtml container, NOT by "both codecs are none".
    // That earlier test looked equivalent but silently dropped real formats:
    // plenty of extractors report no codec metadata for direct progressive
    // downloads (PornHub's `240p`/`1080p`/`2160p` entries have null vcodec
    // *and* null acodec), and those are often the best thing to pick. The
    // picker was hiding half the list on such sites.
    let is_storyboard = as_string(format, "ext", "") == "mhtml"
        || as_string(format, "protocol", "") == "mhtml";
    if is_storyboard {
        return None;
    }

    let vcodec = as_string(format, "vcodec", "none");
    let acodec = as_string(format, "acodec", "none");

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
    let command = quote_command(ytdlp, &args);

    let output = match Command::new(ytdlp).args(&args).output() {
        Ok(output) => output,
        Err(e) => return VideoFormats::failed(url, command, format!("failed to run yt-dlp: {e}")),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Keep a tail rather than only the final line: yt-dlp's actual cause
        // is routinely a WARNING several lines above the terminating ERROR
        // (an unavailable extractor, a cookie problem, a failed JS runtime),
        // and reporting one line hid exactly the part that explains why.
        let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
        let start = lines.len().saturating_sub(STDERR_TAIL_LINES);
        let message = if lines.is_empty() {
            "yt-dlp exited with an error".to_string()
        } else {
            lines[start..].join("\n")
        };
        return VideoFormats::failed(url, command, message);
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
        return VideoFormats::failed(url, command, "yt-dlp returned no video metadata".to_string());
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
        return VideoFormats::failed(url, command, "no downloadable formats found".to_string());
    }

    VideoFormats {
        url: url.to_string(),
        title,
        video_id: as_string(first, "id", ""),
        formats,
        error: None,
        command,
    }
}

/// Counter identifying the probe batch that currently owns the picker.
///
/// Cancelling bumps it; a running batch compares it against the value it read
/// at start and stops between chunks once they differ. A counter rather than a
/// flag so that cancelling batch N can't also abort a batch N+1 the user
/// started immediately afterwards.
#[derive(Default)]
pub struct ProbeEpoch(AtomicU64);

/// Stops the running probe batch at its next chunk boundary.
///
/// Deliberately partial: it does not kill the yt-dlp children already running,
/// so with a single URL — one chunk, one process — nothing is actually stopped
/// and this is a no-op beyond bookkeeping. Killing the children means spawning
/// them with retained handles instead of `Command::output()`, which is written
/// up in DEVLOG under v0.2.2 and deferred until the wasted work is worth the
/// restructuring.
#[tauri::command]
pub fn cancel_probe(epoch: State<'_, ProbeEpoch>) {
    epoch.0.fetch_add(1, Ordering::SeqCst);
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

        // Read once at the start: everything spawned by this call belongs to
        // this epoch, and a later cancel is what makes the two disagree.
        let epoch = app.state::<ProbeEpoch>();
        let mine = epoch.0.load(Ordering::SeqCst);

        let mut results: Vec<VideoFormats> = Vec::with_capacity(request.urls.len());
        for chunk in request.urls.chunks(chunk_size) {
            // Checked between chunks only — the chunk already in flight runs
            // to completion. Whatever has been collected so far is returned
            // rather than discarded here; the caller drops the result of a
            // probe it no longer owns (see MainPage's probe run token).
            if epoch.0.load(Ordering::SeqCst) != mine {
                break;
            }
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
                            .unwrap_or_else(|_| VideoFormats::failed(url, String::new(), "probe thread panicked".to_string()))
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

#[cfg(test)]
mod tests {
    use super::parse_format;
    use serde_json::json;

    #[test]
    fn keeps_formats_that_report_no_codec_metadata() {
        // Shape taken from a real PornHub probe: a direct progressive mp4
        // whose extractor reports neither codec. These were being dropped as
        // if they were storyboards, hiding half the picker's list — and they
        // are frequently the format worth choosing.
        let entry = parse_format(&json!({
            "format_id": "1080p", "ext": "mp4", "resolution": "1080p",
            "protocol": "https", "vcodec": null, "acodec": null
        }))
        .expect("a real format with unreported codecs must stay selectable");
        assert_eq!(entry.format_id, "1080p");
        assert_eq!(entry.vcodec, "none");
    }

    #[test]
    fn drops_storyboard_image_sheets() {
        // Shape taken from a real YouTube probe. These are grids of preview
        // images, so picking one downloads no video at all.
        assert!(parse_format(&json!({
            "format_id": "sb0", "ext": "mhtml", "resolution": "48x27",
            "protocol": "mhtml", "vcodec": "none", "acodec": "none"
        }))
        .is_none());
    }

    #[test]
    fn keeps_ordinary_adaptive_streams() {
        let entry = parse_format(&json!({
            "format_id": "hls-9562", "ext": "mp4", "resolution": "3840x2160",
            "protocol": "m3u8_native", "vcodec": "avc1.640034", "acodec": "mp4a.40.2"
        }))
        .expect("adaptive stream must be selectable");
        assert_eq!(entry.acodec, "mp4a.40.2");
    }
}
