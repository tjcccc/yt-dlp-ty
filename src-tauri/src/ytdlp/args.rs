/// Single source of truth for assembling yt-dlp CLI arguments.
/// Any frontend preview of args must treat this as authoritative — if they
/// disagree, this wins.
pub struct DownloadRequest {
    pub url: String,
    pub download_to: String,
    /// Already tokenized (see `apply_mode`). Kept as tokens rather than a
    /// raw string so a parse failure can never silently reach this point
    /// and drop the user's flags — bad input is rejected up front, where it
    /// can still be reported.
    pub parameters: Vec<String>,
    /// Resolved absolute path to ffmpeg, if found. yt-dlp does its own PATH
    /// search for ffmpeg when merging separate video+audio streams, and a
    /// bundled/GUI-launched process may not inherit a PATH that includes
    /// Homebrew's `/opt/homebrew/bin` — passing this explicitly via
    /// `--ffmpeg-location` closes that gap the same way `resolve_binary_path`
    /// already does for our own direct `yt-dlp` spawn.
    pub ffmpeg_path: Option<String>,
    /// From `AppConfig.proxy` (Config page) — empty means "no proxy set".
    pub proxy: Option<String>,
}

const PROGRESS_TEMPLATE: &str = "dl:%(info.id)s|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s";

/// Expands a leading `~` to the user's home directory. yt-dlp is invoked
/// directly (no shell), so `~` is never expanded by the OS the way it would
/// be on a command line — this must happen here.
pub fn expand_tilde(path: &str) -> String {
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Tokenizes the raw Parameters text the user typed. Returns a human-readable
/// error rather than an empty list on failure: an unbalanced quote used to be
/// swallowed by `unwrap_or_default()`, which silently discarded *every* flag
/// (cookies, proxy, `--no-playlist`, …) and ran a download that looked normal
/// but behaved nothing like what was asked for. Failing loudly is the point.
pub fn parse_parameters(parameters: &str) -> Result<Vec<String>, String> {
    shell_words::split(parameters).map_err(|_| {
        "Parameters could not be parsed — check for an unbalanced quote.".to_string()
    })
}

/// Drops any `-f`/`--format` flag and its value. Shared by every
/// format-selection mode, all of which replace the user's raw selector
/// rather than fighting with it.
fn strip_format_flag(tokens: &[String]) -> Vec<String> {
    let mut stripped: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "-f" || tokens[i] == "--format" {
            i += 2; // skip the flag and its value
            continue;
        }
        stripped.push(tokens[i].clone());
        i += 1;
    }
    stripped
}

/// A format the user picked in the "Choose format first" flow. `video_only`
/// comes from the probed entry's codecs (video stream present, audio absent)
/// and decides whether an audio stream has to be paired in — picking a
/// video-only format without pairing would silently download a silent video.
#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChosenFormat {
    pub format_id: String,
    pub video_only: bool,
}

/// Applies one URL's picked format. The `/<id>` fallback matters: if pairing
/// fails (no separate audio available), yt-dlp still fetches the requested
/// stream rather than erroring out.
pub fn apply_chosen_format(tokens: &[String], chosen: &ChosenFormat) -> Vec<String> {
    let mut args = strip_format_flag(tokens);
    let selector = if chosen.video_only {
        format!("{id}+bestaudio/{id}", id = chosen.format_id)
    } else {
        chosen.format_id.clone()
    };
    args.push("-f".to_string());
    args.push(selector);
    args
}

/// Arguments for a metadata-only probe (`yt-dlp -j`), used by the
/// "Choose format first" picker. Deliberately built from the *same* user
/// parameters and proxy as a real download so the format list shown is the
/// one that will actually be available at download time — a picker fed by
/// different flags (different cookies, no proxy) can list formats the real
/// run then can't fetch. Only the terminal action differs: `-j` instead of
/// `-o`/progress flags.
pub fn build_probe_args(parameters: &[String], proxy: Option<&str>, url: &str) -> Vec<String> {
    let mut args = strip_format_flag(parameters);
    args.push("-j".to_string());
    args.push("--no-color".to_string());
    // `-j` doesn't just dump metadata — it resolves the format selector too,
    // and aborts with "Requested format is not available" when the default
    // selector matches nothing. That happens on real sites (PornHub lists
    // formats whose codecs it doesn't report, which the default `bv*+ba/b`
    // won't match), so a probe would fail on exactly the videos the picker
    // exists to help with. This makes the probe report what's available
    // instead of pre-judging it — selection is the user's job here, and an
    // actually-unavailable video still comes back with an empty format list.
    args.push("--ignore-no-formats-error".to_string());
    if let Some(proxy) = proxy.filter(|p| !p.trim().is_empty()) {
        args.push("--proxy".to_string());
        args.push(proxy.to_string());
    }
    args.push(url.to_string());
    args
}

/// Applies the Options-toggle format-selection mode to raw Parameters text,
/// returning the final token list. The mockup's three toggles ("Choose format
/// first", "Best video", "Best audio") are one mutually exclusive strategy,
/// not independent booleans — when a format-selection mode is active it
/// replaces any `-f`/`--format` flag already present in the raw text.
/// Tokenized via `shell_words`, not a regex, since values may be quoted or
/// contain spaces. `mode` is one of "raw" | "bestVideo" | "bestAudio" (and
/// "chooseFormat", accepted but treated as a no-op here — its real behavior
/// is Milestone 3's job).
///
/// Every mode parses, including the pass-through ones: validation must not
/// depend on which toggle happens to be active, or malformed input would slip
/// through in exactly the default ("raw") case most users are in.
pub fn apply_mode(parameters: &str, mode: &str) -> Result<Vec<String>, String> {
    let tokens = parse_parameters(parameters)?;
    // "chooseFormat" keeps the raw tokens here because its selector is
    // per-URL (each video gets its own picked format), applied later by
    // `apply_chosen_format` once the user has actually chosen.
    if mode == "raw" || mode == "chooseFormat" {
        return Ok(tokens);
    }

    let mut stripped = strip_format_flag(&tokens);

    match mode {
        "bestVideo" => {
            stripped.push("-f".to_string());
            stripped.push("bestvideo+bestaudio/best".to_string());
        }
        "bestAudio" => {
            stripped.push("-f".to_string());
            stripped.push("bestaudio/best".to_string());
            stripped.push("-x".to_string());
        }
        _ => {}
    }

    Ok(stripped)
}

/// Builds the full argument list for a real download. `--newline` is
/// mandatory: yt-dlp normally rewrites its progress line in place with `\r`,
/// and stdout is read line-by-line, so without it no progress line is ever
/// seen until the whole download finishes.
pub fn build_download_args(req: &DownloadRequest) -> Vec<String> {
    let mut args: Vec<String> = req.parameters.clone();

    args.push("--newline".to_string());
    args.push("--no-color".to_string());
    args.push("--progress-template".to_string());
    args.push(PROGRESS_TEMPLATE.to_string());
    if let Some(ffmpeg_path) = &req.ffmpeg_path {
        args.push("--ffmpeg-location".to_string());
        args.push(ffmpeg_path.clone());
    }
    if let Some(proxy) = req.proxy.as_deref().filter(|p| !p.trim().is_empty()) {
        args.push("--proxy".to_string());
        args.push(proxy.to_string());
    }
    args.push("-o".to_string());
    args.push(expand_tilde(&req.download_to));
    args.push(req.url.clone());

    args
}

#[cfg(test)]
mod tests {
    use super::{apply_chosen_format, apply_mode, build_probe_args, ChosenFormat};

    fn applied(parameters: &str, mode: &str) -> Vec<String> {
        apply_mode(parameters, mode).expect("expected parameters to parse")
    }

    #[test]
    fn raw_mode_is_unmodified() {
        assert_eq!(applied("-v --no-playlist", "raw"), ["-v", "--no-playlist"]);
    }

    #[test]
    fn best_video_strips_existing_format_flag_and_appends() {
        assert_eq!(
            applied("-v -f 22 --no-playlist", "bestVideo"),
            ["-v", "--no-playlist", "-f", "bestvideo+bestaudio/best"]
        );
    }

    #[test]
    fn best_audio_strips_long_format_flag_and_extracts() {
        assert_eq!(
            applied("--format 140 --no-playlist", "bestAudio"),
            ["--no-playlist", "-f", "bestaudio/best", "-x"]
        );
    }

    #[test]
    fn choose_format_defers_the_selector_to_per_url_application() {
        // `apply_mode` intentionally leaves the raw tokens alone here: each
        // URL gets its own picked format, applied later by
        // `apply_chosen_format`, which is what strips the stale `-f`.
        assert_eq!(applied("-f 22", "chooseFormat"), ["-f", "22"]);
    }

    #[test]
    fn quoted_value_containing_spaces_survives_as_one_token() {
        assert_eq!(
            applied("--user-agent \"Mozilla 5.0\" --no-playlist", "raw"),
            ["--user-agent", "Mozilla 5.0", "--no-playlist"]
        );
    }

    #[test]
    fn chosen_video_only_format_pairs_in_audio() {
        // A video-only stream downloaded alone would be silent, so it must
        // be paired — with a fallback to the bare id if no separate audio
        // is available, rather than failing the download outright.
        let chosen = ChosenFormat { format_id: "399".into(), video_only: true };
        assert_eq!(
            apply_chosen_format(&["--no-playlist".to_string()], &chosen),
            ["--no-playlist", "-f", "399+bestaudio/399"]
        );
    }

    #[test]
    fn chosen_progressive_format_is_used_as_is() {
        let chosen = ChosenFormat { format_id: "18".into(), video_only: false };
        assert_eq!(apply_chosen_format(&[], &chosen), ["-f", "18"]);
    }

    #[test]
    fn chosen_format_replaces_any_existing_selector() {
        let chosen = ChosenFormat { format_id: "18".into(), video_only: false };
        let existing = vec!["-f".to_string(), "22".to_string(), "-v".to_string()];
        assert_eq!(apply_chosen_format(&existing, &chosen), ["-v", "-f", "18"]);
    }

    #[test]
    fn probe_args_carry_user_flags_and_proxy_but_no_download_flags() {
        // The picker must be fed the same cookies/proxy the real download
        // uses, or it can list formats the actual run then can't fetch.
        let params = vec!["--cookies-from-browser".to_string(), "chrome".to_string()];
        let args = build_probe_args(&params, Some("http://127.0.0.1:7890"), "URL");
        assert_eq!(
            args,
            [
                "--cookies-from-browser",
                "chrome",
                "-j",
                "--no-color",
                "--ignore-no-formats-error",
                "--proxy",
                "http://127.0.0.1:7890",
                "URL"
            ]
        );
        assert!(!args.iter().any(|a| a == "-o" || a == "--progress-template"));
    }

    #[test]
    fn probe_never_lets_format_selection_fail_the_listing() {
        // `-j` resolves the format selector as well as dumping metadata, so
        // without this flag a probe aborts with "Requested format is not
        // available" on any site whose formats the default selector can't
        // match — i.e. it fails on exactly the videos the picker is for.
        let args = build_probe_args(&[], None, "URL");
        assert!(args.iter().any(|a| a == "--ignore-no-formats-error"));
    }

    #[test]
    fn probe_args_omit_proxy_when_unset() {
        let args = build_probe_args(&[], Some("   "), "URL");
        assert!(!args.iter().any(|a| a == "--proxy"), "blank proxy should not be passed");
    }

    #[test]
    fn parameters_may_be_written_one_flag_per_line() {
        // The Parameters field is a textarea and users write flags on
        // separate lines. `shell_words` already treats a newline as a
        // separator; this pins that down so nobody "simplifies" the parser
        // into a space-split later, which would fuse `chrome--write-sub`
        // style tokens together and silently corrupt the command.
        assert_eq!(
            applied("--no-playlist\n--cookies-from-browser chrome\n\n  --write-sub  \n", "raw"),
            ["--no-playlist", "--cookies-from-browser", "chrome", "--write-sub"]
        );
    }

    #[test]
    fn a_quoted_value_survives_a_following_line_break() {
        assert_eq!(
            applied("--user-agent \"Mozilla 5.0\"\n-v", "raw"),
            ["--user-agent", "Mozilla 5.0", "-v"]
        );
    }

    #[test]
    fn unbalanced_quote_is_an_error_not_a_silent_drop() {
        // Regression: this used to `unwrap_or_default()` into an empty list,
        // so the download ran with none of the user's flags and no warning.
        for mode in ["raw", "bestVideo", "bestAudio", "chooseFormat"] {
            assert!(
                apply_mode("--user-agent \"Mozilla", mode).is_err(),
                "mode {mode} silently accepted malformed parameters"
            );
        }
    }
}
