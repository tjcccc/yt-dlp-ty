/// Fields parsed from one `dl:`-prefixed progress line, matching the
/// `--progress-template` set in `args.rs`.
pub struct ProgressFields {
    pub status: String,
    pub downloaded_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
    pub speed_bps: Option<f64>,
    pub eta_seconds: Option<f64>,
}

fn parse_f64(s: &str) -> Option<f64> {
    if s.is_empty() || s == "None" || s == "NA" {
        None
    } else {
        s.parse::<f64>().ok()
    }
}

/// Parses one stdout line. Returns `None` for any line that isn't a
/// progress line emitted by our `--progress-template` (e.g. yt-dlp's own
/// `[youtube]`/`[Merger]` log lines).
pub fn parse_progress_line(line: &str) -> Option<ProgressFields> {
    let rest = line.strip_prefix("dl:")?;
    let parts: Vec<&str> = rest.split('|').collect();
    if parts.len() < 7 {
        return None;
    }

    let total_bytes = parse_f64(parts[3]).or_else(|| parse_f64(parts[4]));

    Some(ProgressFields {
        status: parts[1].to_string(),
        downloaded_bytes: parse_f64(parts[2]),
        total_bytes,
        speed_bps: parse_f64(parts[5]),
        eta_seconds: parse_f64(parts[6]),
    })
}

/// True for yt-dlp log lines indicating the post-download merge/remux step
/// (ffmpeg combining separate video+audio streams), which isn't covered by
/// `--progress-template`.
pub fn is_merge_line(line: &str) -> bool {
    line.contains("[Merger]") || line.contains("[ffmpeg]") || line.contains("Merging formats")
}

/// True for the line yt-dlp prints instead of downloading when the target
/// file is already on disk: `[download] <path> has already been downloaded`
/// (older builds append `and merged`). Matched on the substring rather than
/// the whole line because the path in the middle is arbitrary.
///
/// This is yt-dlp's own existing-file check, which compares the *final
/// output path* — a partially fetched file is still a `.part` and never
/// matches, so a hit means a complete earlier download of the same name.
/// Only reachable because `build_download_args` passes `--no-quiet`; with
/// `--print`'s implied `--quiet` this line is never printed at all.
pub fn is_already_downloaded_line(line: &str) -> bool {
    line.contains("has already been downloaded")
}

/// Best-effort prediction of whether the given yt-dlp args imply a
/// video+audio merge: an explicit `-f`/`--format` value containing `+`
/// (two streams), or no explicit format selector at all — yt-dlp's modern
/// default (`bv*+ba/b`) merges on essentially any source with adaptive
/// formats. Not a guarantee (an unusual fallback could still behave
/// differently); `PassTracker`'s `last_percent` floor is the safety net for
/// when this prediction turns out wrong.
pub fn expects_merge(args: &[String]) -> bool {
    let format_value = args.iter().enumerate().find_map(|(i, a)| {
        if (a == "-f" || a == "--format") && i + 1 < args.len() {
            Some(args[i + 1].as_str())
        } else {
            None
        }
    });
    match format_value {
        Some(v) => v.contains('+'),
        None => true,
    }
}

/// Remaps yt-dlp's own per-stream 0-100 percent into one monotonic 0-100
/// value for the frontend. Without this, a video+audio merge (two full
/// download passes plus an ffmpeg merge) would show percent going
/// 0→100→0→100 rather than one smooth climb.
///
/// Which split to use is decided once, from `expects_merge` computed at
/// spawn time — not reactively, because reacting only after a second pass
/// is already underway would mean the first pass was already shown at raw
/// (uncompressed) scale, and compressing the second pass afterward would
/// require the displayed percent to jump backward. Predicting up front
/// avoids that; `last_percent` is a floor so the reported value can never
/// decrease even if the prediction turns out wrong.
pub struct PassTracker {
    expects_merge: bool,
    pass_index: u8, // 0 = first pass, 1 = second pass, 2+ = merging/done
    in_finished_run: bool,
    last_percent: f64,
}

impl PassTracker {
    pub fn new(expects_merge: bool) -> Self {
        Self {
            expects_merge,
            pass_index: 0,
            in_finished_run: false,
            last_percent: 0.0,
        }
    }

    fn bounds(&self) -> (f64, f64) {
        if !self.expects_merge {
            return (0.0, 100.0);
        }
        match self.pass_index {
            0 => (0.0, 50.0),
            1 => (50.0, 95.0),
            _ => (95.0, 100.0),
        }
    }

    /// Feed one progress line's raw 0-100 percent and status; returns the
    /// remapped, monotonically non-decreasing overall percent.
    ///
    /// Bounds for *this* line are read before advancing `pass_index` — a
    /// "finished" line closes out the pass that just ended (capping it at
    /// its band's own `hi`), and only the *next* line sees the advanced
    /// pass's bounds. Advancing first would make the closing line of pass 1
    /// jump straight into pass 2's band instead of completing pass 1's.
    pub fn observe(&mut self, raw_percent: f64, status: &str) -> f64 {
        let (lo, hi) = self.bounds();
        let mapped = lo + (raw_percent.clamp(0.0, 100.0) / 100.0) * (hi - lo);

        if status == "finished" {
            if !self.in_finished_run {
                self.in_finished_run = true;
                self.pass_index += 1;
            }
        } else {
            self.in_finished_run = false;
        }

        self.last_percent = self.last_percent.max(mapped);
        self.last_percent
    }

    /// Called when an `is_merge_line` is observed (yt-dlp emits `[Merger]`
    /// lines outside `--progress-template` entirely) — jumps into the merge
    /// band immediately rather than waiting for the next progress line.
    pub fn observe_merge(&mut self) -> f64 {
        self.pass_index = self.pass_index.max(2);
        self.last_percent = self.last_percent.max(95.0);
        self.last_percent
    }
}

#[cfg(test)]
mod line_tests {
    use super::{is_already_downloaded_line, is_merge_line, parse_progress_line};

    #[test]
    fn recognizes_the_skipped_file_line() {
        assert!(is_already_downloaded_line(
            "[download] /Users/me/Downloads/yt-dlp/pornhub/Clip.mp4 has already been downloaded"
        ));
        // Older yt-dlp builds report a merged target this way.
        assert!(is_already_downloaded_line(
            "[download] /Users/me/Clip.mkv has already been downloaded and merged"
        ));
    }

    #[test]
    fn ordinary_lines_are_not_mistaken_for_a_skip() {
        assert!(!is_already_downloaded_line("[download] Destination: /Users/me/Clip.mp4"));
        assert!(!is_already_downloaded_line("dl:abc|downloading|1024|2048|NA|NA|NA"));
        // A title could mention downloading; only the exact phrase counts.
        assert!(!is_already_downloaded_line("[download] How I downloaded a file.mp4"));
    }

    #[test]
    fn skip_line_is_neither_progress_nor_merge() {
        // The reader checks these in sequence, so a skip line must not be
        // claimed by either of the other two matchers first.
        let line = "[download] /Users/me/Clip.mp4 has already been downloaded";
        assert!(parse_progress_line(line).is_none());
        assert!(!is_merge_line(line));
    }
}

#[cfg(test)]
mod pass_tracker_tests {
    use super::PassTracker;

    fn is_monotonic(values: &[f64]) -> bool {
        values.windows(2).all(|w| w[1] >= w[0])
    }

    #[test]
    fn single_pass_download_is_smooth_zero_to_hundred() {
        // e.g. `-f 18` — a progressive format, no merge expected. Must not
        // be artificially compressed into 0-50, or a real single-pass
        // download (the common, already-verified case) would regress to
        // looking stuck at 50% until it snaps to 100%.
        let mut tracker = PassTracker::new(false);
        let mut seen = Vec::new();
        for raw in [0.0, 25.0, 50.0, 75.0, 100.0] {
            seen.push(tracker.observe(raw, "downloading"));
        }
        seen.push(tracker.observe(100.0, "finished"));
        assert_eq!(seen, vec![0.0, 25.0, 50.0, 75.0, 100.0, 100.0]);
        assert!(is_monotonic(&seen));
    }

    #[test]
    fn two_pass_merge_is_monotonic_with_no_snap_back() {
        // Simulates: video pass 0->100, audio pass 0->100, a `[Merger]`
        // line, then completion — the exact scenario that would otherwise
        // show percent going 0->100->0->100.
        let mut tracker = PassTracker::new(true);
        let mut seen = Vec::new();

        for raw in [0.0, 50.0, 100.0] {
            seen.push(tracker.observe(raw, "downloading"));
        }
        seen.push(tracker.observe(100.0, "finished")); // end of pass 1

        for raw in [0.0, 50.0, 100.0] {
            seen.push(tracker.observe(raw, "downloading"));
        }
        seen.push(tracker.observe(100.0, "finished")); // end of pass 2

        seen.push(tracker.observe_merge());

        assert!(is_monotonic(&seen), "percent sequence went backward: {seen:?}");
        assert_eq!(*seen.first().unwrap(), 0.0);
        assert_eq!(*seen.last().unwrap(), 95.0);
        // Pass 1 must stay within its 0-50 band, pass 2 within 50-95 —
        // i.e. the merge case never re-crosses ground pass 1 already
        // covered (the "snap back" this whole mechanism exists to avoid).
        assert!(seen[..4].iter().all(|p| *p <= 50.0));
        assert!(seen[4..8].iter().all(|p| *p <= 95.0));
    }

    #[test]
    fn percent_never_decreases_even_if_prediction_was_wrong() {
        // Predicted single-pass (no merge expected), but a second pass
        // shows up anyway — an edge case the up-front prediction can't
        // rule out. The floor must still hold: never report a lower
        // percent than already shown.
        let mut tracker = PassTracker::new(false);
        tracker.observe(90.0, "downloading");
        let after_finish = tracker.observe(100.0, "finished");
        let unexpected_second_pass = tracker.observe(0.0, "downloading");
        assert!(unexpected_second_pass >= after_finish);
    }
}
