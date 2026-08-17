use tauri::AppHandle;
use uuid::Uuid;

use crate::commands::templates::consume_next_seq;
use crate::ytdlp::args::expand_tilde;

/// Replaces a `{id:NNN...}` token (any run of `N`s sets the zero-pad width)
/// with `seq`, zero-padded to that width. Manual scan rather than a regex
/// dependency, since the shape is simple and fixed.
fn replace_seq_token(s: &str, seq: u32) -> String {
    if let Some(start) = s.find("{id:N") {
        let after_prefix = &s[start + 4..]; // skip "{id:"
        if let Some(end_rel) = after_prefix.find('}') {
            let inner = &after_prefix[..end_rel];
            if !inner.is_empty() && inner.chars().all(|c| c == 'N') {
                let width = inner.len();
                let token = format!("{{id:{inner}}}");
                let replacement = format!("{seq:0width$}");
                return s.replacen(&token, &replacement, 1);
            }
        }
    }
    s.to_string()
}

/// Resolves a user-authored `download_to` template into the value passed to
/// yt-dlp's `-o`. Tokens resolve at different times, so this is a hybrid:
///
/// - `{date:YYYY-MM-DD}` — today's date, resolved now (at job-creation time,
///   not the video's upload date, which isn't known until yt-dlp fetches
///   metadata).
/// - `{id:NNN}` (or any run of Ns) — `Template.next_seq`, consumed and
///   incremented now so concurrent/cancelled jobs still get distinct numbers.
/// - `{id:guid}` — a fresh uuid v4 per job.
/// - `{original_filename}` — NOT resolvable app-side (the title is unknown
///   until yt-dlp fetches metadata). Everything from this token onward is
///   replaced by yt-dlp's own `%(title)s`, and a `.%(ext)s` is always
///   appended regardless of whether the user's template had the token at
///   all, since yt-dlp needs the extension slot.
pub fn resolve(app: &AppHandle, template_id: &str, raw: &str) -> String {
    let prefix = match raw.split_once("{original_filename}") {
        Some((before, _after)) => before,
        None => raw,
    };

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut resolved = prefix.replace("{date:YYYY-MM-DD}", &today);

    if resolved.contains("{id:N") {
        let seq = consume_next_seq(app, template_id);
        resolved = replace_seq_token(&resolved, seq);
    }
    if resolved.contains("{id:guid}") {
        resolved = resolved.replace("{id:guid}", &Uuid::new_v4().to_string());
    }

    let expanded = expand_tilde(&resolved);
    // yt-dlp's `-o` template treats a literal `%` as its own escape
    // character — escape any `%` from the user's resolved prefix before
    // appending yt-dlp's own `%(...)s` placeholders.
    let escaped = expanded.replace('%', "%%");

    format!("{escaped}%(title)s.%(ext)s")
}

#[cfg(test)]
mod tests {
    use super::replace_seq_token;

    #[test]
    fn pads_to_token_width() {
        assert_eq!(replace_seq_token("{id:NNN}", 7), "007");
        assert_eq!(replace_seq_token("{id:NN}", 7), "07");
        assert_eq!(replace_seq_token("prefix-{id:NNNN}-suffix", 42), "prefix-0042-suffix");
    }

    #[test]
    fn leaves_string_untouched_without_token() {
        assert_eq!(replace_seq_token("no token here", 7), "no token here");
    }
}
