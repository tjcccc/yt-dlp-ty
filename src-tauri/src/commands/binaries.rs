use std::path::PathBuf;
use std::process::Command;

use tauri::AppHandle;

use crate::commands::config::load_config;

/// A bundled `.app` launched from Finder does not inherit the user's shell
/// PATH, so a Homebrew-installed `yt-dlp`/`ffmpeg` at `/opt/homebrew/bin` is
/// invisible to `Command::new("yt-dlp")` even though `pnpm tauri dev` finds
/// it fine from a terminal. Probe common install locations before falling
/// back to a bare name (which still works in dev, and in any environment
/// where PATH happens to be inherited).
pub fn resolve_binary_path(name: &str, override_path: Option<&str>) -> PathBuf {
    if let Some(p) = override_path {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }

    let home = dirs::home_dir();
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(format!("/opt/homebrew/bin/{name}")),
        PathBuf::from(format!("/usr/local/bin/{name}")),
    ];
    if let Some(home) = &home {
        candidates.push(home.join(".local/bin").join(name));
    }
    candidates.push(PathBuf::from(format!("/usr/bin/{name}")));

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    // Fall back to a bare name and let Command::new search the process's PATH.
    PathBuf::from(name)
}

/// `resolve_binary_path`, but reading the user's saved override (if any)
/// from persisted `AppConfig` first — the layer nothing wired up until this
/// milestone.
pub fn resolve_configured(app: &AppHandle, name: &str) -> PathBuf {
    let config = load_config(app);
    let override_path = match name {
        "yt-dlp" => config.ytdlp_path,
        "ffmpeg" => config.ffmpeg_path,
        _ => None,
    };
    resolve_binary_path(name, override_path.as_deref())
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryCheck {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// ffmpeg's version flag is `-version` (single dash) — `--version` is not
/// recognized and exits non-zero. yt-dlp and most other CLIs use the GNU
/// double-dash form.
fn version_flag(name: &str) -> &'static str {
    if name == "ffmpeg" {
        "-version"
    } else {
        "--version"
    }
}

/// yt-dlp prints a bare version (`2026.07.04`), but ffmpeg prints a whole
/// banner line (`ffmpeg version 8.1 Copyright (c) 2000-2026 the FFmpeg
/// developers`) that swamps the UI's status text. Reduce the banner to just
/// the version token; anything unrecognized is passed through as-is.
fn short_version(raw: &str) -> String {
    raw.strip_prefix("ffmpeg version ")
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or(raw)
        .to_string()
}

#[tauri::command]
pub fn check_binary(app: AppHandle, name: String, custom_path: Option<String>) -> BinaryCheck {
    // An explicit `custom_path` (the Config page testing a candidate path
    // before saving it) wins; otherwise fall back to whatever the user
    // already saved, so the automatic check on app start reflects it too.
    let override_path = custom_path.or_else(|| {
        let config = load_config(&app);
        match name.as_str() {
            "yt-dlp" => config.ytdlp_path,
            "ffmpeg" => config.ffmpeg_path,
            _ => None,
        }
    });
    let resolved = resolve_binary_path(&name, override_path.as_deref());

    // ffmpeg prints its version banner to stderr even on success; yt-dlp
    // prints to stdout. Check both so `found`/`version` work for either.
    match Command::new(&resolved).arg(version_flag(&name)).output() {
        Ok(output) if output.status.success() => {
            let combined = if !output.stdout.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            };
            BinaryCheck {
                found: true,
                path: Some(resolved.to_string_lossy().to_string()),
                version: Some(short_version(
                    String::from_utf8_lossy(combined).lines().next().unwrap_or("").trim(),
                )),
                error: None,
            }
        }
        Ok(output) => BinaryCheck {
            found: false,
            path: Some(resolved.to_string_lossy().to_string()),
            version: None,
            error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(e) => BinaryCheck {
            found: false,
            path: Some(resolved.to_string_lossy().to_string()),
            version: None,
            error: Some(e.to_string()),
        },
    }
}

/// Runs `yt-dlp -U` (self-update) as a manual, user-triggered action — never
/// implicitly per download job (see DEVLOG for why: it fails outright on a
/// Homebrew-managed install, and shouldn't re-run on every single job even
/// where it works).
#[tauri::command]
pub fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let resolved = resolve_configured(&app, "yt-dlp");
    let output = Command::new(&resolved)
        .arg("-U")
        .output()
        .map_err(|e| format!("failed to run {}: {e}", resolved.display()))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(combined.trim().to_string())
    } else {
        Err(combined.trim().to_string())
    }
}
