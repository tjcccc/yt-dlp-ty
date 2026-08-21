use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

// `Manager` is needed for `try_state`: the history write happens on the
// reader thread after the process exits, where the Db handle has to be
// fetched from app state rather than passed down.
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::commands::binaries::resolve_configured;
use crate::commands::config::load_config;
use crate::commands::history;
use crate::db::{Db, DbState};
use crate::registry::{JobRegistry, PendingJob};
use crate::ytdlp::args::{
    apply_chosen_format, apply_mode, build_download_args, ChosenFormat, DownloadRequest as ArgsRequest,
    PRINT_EXTRACTOR_PREFIX, PRINT_PATH_PREFIX,
};
use crate::ytdlp::path_template;
use crate::ytdlp::progress::{
    expects_merge, is_already_downloaded_line, is_merge_line, parse_progress_line, PassTracker,
};

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
    pub phase: String, // "queued" | "downloading" | "merging" | "completed" | "skipped" | "error" | "cancelled"
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
    /// The shell-quoted command this job runs. Sent once, on the first event
    /// after spawn, rather than on every progress tick — it never changes and
    /// progress is emitted several times a second.
    pub command: Option<String>,
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
            command: None,
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
    db: DbState<'_>,
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

    let template_name = {
        let conn = db.0.lock().unwrap();
        crate::commands::templates::template_name(&conn, &request.template_id)
    };

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
            template_name: template_name.clone(),
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
    let url = pending.url.clone();
    let args = build_download_args(&ArgsRequest {
        url: pending.url,
        download_to: pending.download_to,
        parameters: pending.parameters,
        ffmpeg_path: pending.ffmpeg_path,
        proxy: Some(proxy),
    });
    let merge_expected = expects_merge(&args);
    // Shell-quoted so it can be pasted into a terminal unchanged, which is
    // the fastest way to tell an app bug apart from a yt-dlp/site problem.
    let command = {
        let mut parts = vec![ytdlp_path.to_string_lossy().to_string()];
        parts.extend(args.iter().cloned());
        shell_words::join(&parts)
    };

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
    let _ = app.emit(
        "job://progress",
        JobProgressEvent {
            overall_percent: Some(0.0),
            command: Some(command.clone()),
            ..JobProgressEvent::terminal(&job_id, "downloading")
        },
    );

    spawn_progress_reader(
        app,
        registry,
        job_id,
        stdout,
        stderr_tail,
        merge_expected,
        HistoryContext {
            url,
            template_name: pending.template_name,
            started_at: chrono::Utc::now(),
            command,
        },
    );
}

/// Everything a history row needs that the reader thread can't derive from
/// yt-dlp's output alone.
struct HistoryContext {
    url: String,
    template_name: Option<String>,
    started_at: chrono::DateTime<chrono::Utc>,
    /// Cloned rather than borrowed from the progress event: that event moves
    /// the command out, and history is written much later on the reader
    /// thread once the process has exited.
    command: String,
}

/// Falls back to the URL's host when yt-dlp doesn't report an extractor —
/// "pornhub.com" is still more use in the history list than an empty cell.
fn host_of(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next()?;
    (!host.is_empty()).then(|| host.trim_start_matches("www.").to_string())
}

fn record_history(
    app: &AppHandle,
    ctx: &HistoryContext,
    status: &str,
    filepath: Option<String>,
    extractor: Option<String>,
) {
    let Some(db) = app.try_state::<Db>() else { return };
    let finished_at = chrono::Utc::now();
    // The filesystem is authoritative for size: yt-dlp's reported total is an
    // estimate for adaptive streams, and after a merge it describes neither
    // of the two source streams.
    let size_bytes = filepath
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len() as i64);
    let filename = filepath.as_ref().and_then(|p| {
        std::path::Path::new(p)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
    });

    let conn = db.0.lock().unwrap();
    history::record(
        &conn,
        &history::NewEntry {
            filename,
            filepath,
            platform: extractor.or_else(|| host_of(&ctx.url)),
            template_name: ctx.template_name.clone(),
            url: ctx.url.clone(),
            started_at: ctx.started_at.to_rfc3339(),
            finished_at: finished_at.to_rfc3339(),
            duration_ms: (finished_at - ctx.started_at).num_milliseconds(),
            size_bytes,
            status: status.to_string(),
            command: Some(ctx.command.clone()),
        },
    );
}

fn spawn_progress_reader(
    app: AppHandle,
    registry: JobRegistry,
    job_id: String,
    stdout: std::process::ChildStdout,
    stderr_tail: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    merge_expected: bool,
    history_ctx: HistoryContext,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut tracker = PassTracker::new(merge_expected);
        // Captured from the `--print after_move:` lines. Absent when the
        // download never got as far as producing a file.
        let mut final_path: Option<String> = None;
        let mut extractor: Option<String> = None;
        // yt-dlp skipped the fetch because the target file was already on
        // disk (see `is_already_downloaded_line`).
        let mut already_downloaded = false;
        // Whether any real transfer happened. Needed because both can be
        // true in one job: a playlist where some entries were already on
        // disk and others were fetched is a completed download, not a
        // skipped one.
        let mut saw_transfer = false;
        let tail_text = |tail: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>| {
            let buf = tail.lock().unwrap();
            let text = buf.iter().cloned().collect::<Vec<_>>().join("\n");
            (!text.is_empty()).then_some(text)
        };

        for line in reader.lines() {
            let Ok(line) = line else { break };

            if let Some(rest) = line.strip_prefix(PRINT_PATH_PREFIX) {
                final_path = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = line.strip_prefix(PRINT_EXTRACTOR_PREFIX) {
                extractor = Some(rest.trim().to_string());
                continue;
            }
            if is_already_downloaded_line(&line) {
                already_downloaded = true;
                continue;
            }

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
                if fields.status == "downloading" {
                    saw_transfer = true;
                }
                let overall_percent = tracker.observe(raw_percent, &fields.status);
                // A skipped file still produces one `finished` progress line.
                // Emitting it would flash "Merging…" on a job that never
                // transferred a byte, immediately before its "Skipped" label.
                if already_downloaded && !saw_transfer {
                    continue;
                }
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
                        command: None,
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
            let succeeded = matches!(&status, Ok(s) if s.success());
            // Nothing was fetched because the file was already there — a
            // distinct outcome from both "completed" (we downloaded it) and
            // "error", and the one the user needs in order to know why a job
            // finished instantly.
            let skipped = succeeded && already_downloaded && !saw_transfer;
            let event = if skipped {
                JobProgressEvent {
                    overall_percent: Some(100.0),
                    ..JobProgressEvent::terminal(&job_id, "skipped")
                }
            } else if succeeded {
                JobProgressEvent::terminal(&job_id, "completed")
            } else {
                JobProgressEvent::error(
                    &job_id,
                    tail_text(&stderr_tail).unwrap_or_else(|| "yt-dlp exited with an error".to_string()),
                )
            };
            let _ = app.emit("job://progress", event);
            record_history(
                &app,
                &history_ctx,
                // Recorded as its own status rather than folded into
                // "completed": the visible history lists completed rows
                // only, so a skip doesn't add a second row for a file that
                // is already listed from the run that actually fetched it.
                match (skipped, succeeded) {
                    (true, _) => "skipped",
                    (_, true) => "completed",
                    _ => "error",
                },
                final_path,
                extractor,
            );
            // A slot just freed up — backfill from the queue, if any.
            fill_slots(&app, registry);
        } else {
            // The job is gone from the registry, so cancellation removed it
            // and already emitted the terminal event. It still ran, so it
            // still belongs in history — recorded here rather than in
            // `cancel_job`, which has only a job id and none of the context
            // this thread holds. (A job cancelled while still queued never
            // spawned a reader and is deliberately not recorded: nothing
            // happened to remember.)
            record_history(&app, &history_ctx, "cancelled", final_path, extractor);
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
