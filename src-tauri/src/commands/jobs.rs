use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::commands::binaries::resolve_configured;
use crate::commands::config::load_config;
use crate::registry::{JobRegistry, PendingJob};
use crate::ytdlp::args::{
    apply_chosen_format, apply_mode, build_download_args, ChosenFormat, DownloadRequest as ArgsRequest,
};
use crate::ytdlp::path_template;
use crate::ytdlp::progress::{expects_merge, is_merge_line, parse_progress_line, PassTracker};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiDownloadRequest {
    pub urls: Vec<String>,
    /// Raw download-to template (may still contain `{date:...}`/`{id:...}`/
    /// `{original_filename}` tokens) — resolved per-URL in `path_template`.
    pub download_to: String,
    pub parameters: String,
    /// "raw" | "bestVideo" | "bestAudio" | "chooseFormat" — see
    /// `ytdlp::args::apply_mode`.
    pub mode: String,
    pub template_id: String,
    /// Only populated when `mode == "chooseFormat"`: url -> the format the
    /// user picked for that URL in the picker. Keyed by URL because each
    /// video gets its own selection.
    #[serde(default)]
    pub chosen_formats: HashMap<String, ChosenFormat>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    pub job_id: String,
    pub phase: String, // "queued" | "downloading" | "merging" | "completed" | "error" | "cancelled"
    pub downloaded_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
    pub speed_bps: Option<f64>,
    pub eta_seconds: Option<f64>,
    /// Remapped, monotonically non-decreasing 0-100 — see `PassTracker`.
    /// The frontend should use this for the progress bar width rather than
    /// computing its own ratio from downloaded/total bytes, which resets
    /// per-pass for a video+audio merge.
    pub overall_percent: Option<f64>,
    /// Tail of yt-dlp's stderr, populated only on phase == "error", so a
    /// failure (e.g. a merge step that can't find ffmpeg, a real extractor
    /// error) is diagnosable from the UI instead of a bare "Error" label.
    pub error_message: Option<String>,
}

impl JobProgressEvent {
    fn terminal(job_id: &str, phase: &str) -> Self {
        Self {
            job_id: job_id.to_string(),
            phase: phase.to_string(),
            downloaded_bytes: None,
            total_bytes: None,
            speed_bps: None,
            eta_seconds: None,
            overall_percent: if phase == "completed" { Some(100.0) } else { None },
            error_message: None,
        }
    }

    fn error(job_id: &str, message: String) -> Self {
        Self {
            error_message: Some(message),
            ..Self::terminal(job_id, "error")
        }
    }
}

/// Caps how much stderr we retain in memory per job — enough for a useful
/// tail, not an unbounded buffer for a long/verbose run.
const STDERR_TAIL_LINES: usize = 40;

fn spawn_stderr_reader(stderr: std::process::ChildStderr) -> std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>> {
    let tail = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::with_capacity(STDERR_TAIL_LINES)));
    let tail_writer = tail.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let mut buf = tail_writer.lock().unwrap();
            if buf.len() == STDERR_TAIL_LINES {
                buf.pop_front();
            }
            buf.push_back(line);
        }
    });
    tail
}

#[tauri::command]
pub fn start_downloads(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    request: MultiDownloadRequest,
) -> Result<Vec<String>, String> {
    let resolved_ffmpeg = resolve_configured(&app, "ffmpeg");
    let ffmpeg_path = resolved_ffmpeg
        .is_absolute()
        .then(|| resolved_ffmpeg.to_string_lossy().to_string());
    // Validated before anything is queued, so malformed Parameters surface
    // as an error in the UI instead of quietly running a download with none
    // of the user's flags applied.
    let effective_parameters = apply_mode(&request.parameters, &request.mode)?;

    if request.mode == "chooseFormat" {
        // Guarded here as well as in the picker UI: reaching a spawn with no
        // selection would silently fall back to yt-dlp's default format,
        // which is the opposite of what "choose format first" promises.
        if let Some(missing) = request.urls.iter().find(|u| !request.chosen_formats.contains_key(*u)) {
            return Err(format!("No format chosen for {missing}"));
        }
    }

    let mut job_ids = Vec::with_capacity(request.urls.len());
    for url in request.urls {
        let job_id = Uuid::new_v4().to_string();
        let resolved_download_to = path_template::resolve(&app, &request.template_id, &request.download_to);
        // The format selector is per-URL in "choose format first" mode, so
        // parameters are finalized per job rather than once for the batch.
        let parameters = match request.chosen_formats.get(&url) {
            Some(chosen) if request.mode == "chooseFormat" => {
                apply_chosen_format(&effective_parameters, chosen)
            }
            _ => effective_parameters.clone(),
        };
        registry.push_pending(PendingJob {
            job_id: job_id.clone(),
            url,
            download_to: resolved_download_to,
            parameters,
            ffmpeg_path: ffmpeg_path.clone(),
        });
        job_ids.push(job_id);
    }

    fill_slots(&app, registry.inner().clone());

    Ok(job_ids)
}

/// Pops queued jobs into running slots until the configured concurrency cap
/// is reached or the queue is empty. Called after `start_downloads` queues
/// new work, and again every time a running job finishes/errors/is
/// cancelled, so a freed slot gets backfilled.
fn fill_slots(app: &AppHandle, registry: JobRegistry) {
    // Held across the entire loop — see `JobRegistry::lock_fill`. Without
    // it, two reader threads reporting completion at the same instant can
    // each see the same free slot and both spawn, exceeding the cap.
    let _fill_guard = registry.lock_fill();
    let concurrency = load_config(app).concurrency.max(1) as usize;
    while registry.running_count() < concurrency {
        let Some(pending) = registry.pop_pending() else { break };
        spawn_pending(app.clone(), registry.clone(), pending);
    }
}

fn spawn_pending(app: AppHandle, registry: JobRegistry, pending: PendingJob) {
    let job_id = pending.job_id.clone();
    let ytdlp_path = resolve_configured(&app, "yt-dlp");

    // Read fresh at spawn time (not queue time) so a proxy change on the
    // Config page takes effect for jobs still waiting on a concurrency slot.
    let proxy = load_config(&app).proxy;
    let args = build_download_args(&ArgsRequest {
        url: pending.url,
        download_to: pending.download_to,
        parameters: pending.parameters,
        ffmpeg_path: pending.ffmpeg_path,
        proxy: Some(proxy),
    });
    let merge_expected = expects_merge(&args);

    let child = Command::new(&ytdlp_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = app.emit(
                "job://progress",
                JobProgressEvent::error(&job_id, format!("failed to spawn yt-dlp at {}: {e}", ytdlp_path.display())),
            );
            return;
        }
    };

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = app.emit(
            "job://progress",
            JobProgressEvent::error(&job_id, "failed to capture yt-dlp stdout/stderr".to_string()),
        );
        return;
    };
    let stderr_tail = spawn_stderr_reader(stderr);

    registry.insert(job_id.clone(), child);
    let _ = app.emit("job://progress", JobProgressEvent { downloaded_bytes: None, total_bytes: None, speed_bps: None, eta_seconds: None, overall_percent: Some(0.0), error_message: None, ..JobProgressEvent::terminal(&job_id, "downloading") });

    spawn_progress_reader(app, registry, job_id, stdout, stderr_tail, merge_expected);
}

fn spawn_progress_reader(
    app: AppHandle,
    registry: JobRegistry,
    job_id: String,
    stdout: std::process::ChildStdout,
    stderr_tail: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    merge_expected: bool,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut tracker = PassTracker::new(merge_expected);
        let tail_text = |tail: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>| {
            let buf = tail.lock().unwrap();
            let text = buf.iter().cloned().collect::<Vec<_>>().join("\n");
            (!text.is_empty()).then_some(text)
        };

        for line in reader.lines() {
            let Ok(line) = line else { break };

            if let Some(fields) = parse_progress_line(&line) {
                if fields.status == "error" {
                    let _ = app.emit(
                        "job://progress",
                        JobProgressEvent::error(&job_id, tail_text(&stderr_tail).unwrap_or_else(|| "yt-dlp reported an error".to_string())),
                    );
                    continue;
                }
                let raw_percent = match (fields.downloaded_bytes, fields.total_bytes) {
                    (Some(d), Some(t)) if t > 0.0 => (d / t) * 100.0,
                    _ if fields.status == "finished" => 100.0,
                    _ => 0.0,
                };
                let overall_percent = tracker.observe(raw_percent, &fields.status);
                let phase = if fields.status == "finished" { "merging" } else { "downloading" };
                let _ = app.emit(
                    "job://progress",
                    JobProgressEvent {
                        job_id: job_id.clone(),
                        phase: phase.to_string(),
                        downloaded_bytes: fields.downloaded_bytes,
                        total_bytes: fields.total_bytes,
                        speed_bps: fields.speed_bps,
                        eta_seconds: fields.eta_seconds,
                        overall_percent: Some(overall_percent),
                        error_message: None,
                    },
                );
            } else if is_merge_line(&line) {
                let overall_percent = tracker.observe_merge();
                let _ = app.emit(
                    "job://progress",
                    JobProgressEvent {
                        overall_percent: Some(overall_percent),
                        ..JobProgressEvent::terminal(&job_id, "merging")
                    },
                );
            }
        }

        // The pipe closed because the process exited (normally or via
        // kill()). If it's still in the registry, cancel_job/cancel_all
        // hasn't already removed and reported it — so this reader thread
        // owns reporting the final state. If it's gone, cancellation got
        // there first and already emitted the terminal event — avoid
        // double-emitting.
        if let Some(mut child) = registry.remove(&job_id) {
            let status = child.wait();
            let event = match status {
                Ok(s) if s.success() => JobProgressEvent::terminal(&job_id, "completed"),
                _ => JobProgressEvent::error(
                    &job_id,
                    tail_text(&stderr_tail).unwrap_or_else(|| "yt-dlp exited with an error".to_string()),
                ),
            };
            let _ = app.emit("job://progress", event);
            // A slot just freed up — backfill from the queue, if any.
            fill_slots(&app, registry);
        }
    });
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, registry: State<'_, JobRegistry>, job_id: String) -> Result<(), String> {
    if registry.remove_pending(&job_id) {
        let _ = app.emit("job://progress", JobProgressEvent::terminal(&job_id, "cancelled"));
        return Ok(());
    }
    let removed = registry.remove(&job_id);
    let was_running = removed.is_some();
    if let Some(mut child) = removed {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = app.emit("job://progress", JobProgressEvent::terminal(&job_id, "cancelled"));
    if was_running {
        // A slot just freed up — backfill from the queue, if any.
        fill_slots(&app, registry.inner().clone());
    }
    Ok(())
}

#[tauri::command]
pub fn cancel_all(app: AppHandle, registry: State<'_, JobRegistry>) -> Result<(), String> {
    for pending in registry.drain_pending() {
        let _ = app.emit("job://progress", JobProgressEvent::terminal(&pending.job_id, "cancelled"));
    }
    for job_id in registry.kill_all_running() {
        let _ = app.emit("job://progress", JobProgressEvent::terminal(&job_id, "cancelled"));
    }
    Ok(())
}
