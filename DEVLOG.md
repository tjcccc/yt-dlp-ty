# DEVLOG

## 2026-08-16

Repo initialized with docs-first milestone: `CLAUDE.md`, `spec/ui.md`, `README.md`,
`DEVLOG.md`. No application code yet.

Decided architecture for the full build (Tauri v2 + React/TypeScript/Vite + Tailwind,
pnpm, system-detected `yt-dlp`/`ffmpeg` binaries, macOS 26 "Liquid Glass"-inspired UI
approximated in CSS). Full plan recorded at
`~/.claude/plans/synthetic-finding-forest.md`. Next up: Milestone 1 — scaffold the
Tauri project and get one real download working end-to-end with live progress.

## 2026-08-17

Milestone 1 done: scaffolded Tauri v2 + React/TypeScript/Vite + Tailwind v4 via
`pnpm create tauri-app`, added `zustand`. Rust side: `commands/binaries.rs`
(`resolve_binary_path`/`check_binary`, probing Homebrew/`~/.local/bin`/`/usr/bin`
before falling back to PATH), `commands/jobs.rs` (`start_download`/`cancel_job`
backed by a `JobRegistry`), `ytdlp/args.rs` (arg assembly + `~` expansion, since
yt-dlp is spawned directly with no shell to do it), `ytdlp/progress.rs` (parses
`--progress-template` output; `--newline` is mandatory or no progress line is
seen until the process exits). Frontend: `MainPage` (single URL + Parameters
input, live binary-check status), `DownloadingPage` (progress bar, Cancel).

Simplified vs. the full plan: single URL only (no multi-line/multi-job), no
templates/config persistence, no two-pass 0–50/50–95/95–100% progress remapping
(single-pass percentage, sufficient since M1 has no video+audio-merge case yet).

Verified end-to-end by driving the actual built app (not a mock) via macOS
Accessibility/AppleScript UI automation: typed a URL and Parameters into the
real fields, clicked the real Download button, watched the progress bar
advance from a real yt-dlp child process, watched it reach "Completed" with the
file landing at the resolved `-o` path, then re-ran and clicked the real Cancel
button and confirmed the child process actually died. Used a local test file
(YouTube extraction is currently broken in this environment — reproduced
identically via bare `yt-dlp -F` outside the app too, so it's an
upstream/environment issue, not an M1 bug) served over localhost so the test
was deterministic.

Two real bugs found and fixed during this pass, not just from writing code:
- `DownloadingPage` selected `useStore(s => Object.values(s.jobs))` — a
  selector returning a fresh array every call breaks React's
  `useSyncExternalStore` snapshot-caching check and blanked the whole page on
  first job. Fixed by selecting the raw `jobs` record and deriving the array
  in the component body instead.
- `check_binary` ran every binary with `--version`, but ffmpeg only accepts
  `-version` (single dash) and prints its banner to stderr, not stdout — so it
  always reported "not found" even when installed. Fixed by using the right
  flag per binary and checking both streams.

Next up: Milestone 2 — templates/config persistence, multi-URL + concurrency,
download-to path templating, Options-toggle precedence.

### Follow-up

The YouTube-extraction failure noted above was re-investigated and was specific
to that one run (rate limiting or a transient network blip), not a real break.
Re-verified by driving the actual running app via AppleScript/Accessibility
automation a second time: typed `https://www.youtube.com/watch?v=aqz-KE-bpKQ`
into the real URL field and `-f 18` into Parameters (a single progressive
format — no video+audio merge, since that path is still M2 scope), clicked the
real Download button, and watched the real progress bar advance
(0%→69%→74%→Completed) driven by real `job://progress` events from a real
yt-dlp child process. The actual video file (28.5MB) landed at the resolved
`~/Downloads/yt-dlp-ty-test/` path. Milestone 1 is now confirmed working
end-to-end against real YouTube, not just a local test file.

### Bugfix — merge step reporting "Error"

User-reported: a real YouTube download completed but flipped to "Error" once
the video+audio merge step ran (i.e. any format requiring `bestvideo+bestaudio`,
which is the common case beyond legacy progressive formats like `-f 18`).

Root cause, confirmed directly (not guessed): yt-dlp does its own PATH search
for `ffmpeg` when it needs to merge separate streams. Under a minimal PATH —
`yt-dlp -v --simulate` with `PATH=/usr/bin:/bin:/usr/sbin:/sbin` reports
`exe versions: none` — yt-dlp can't find Homebrew's ffmpeg at all and the merge
step fails outright. This is exactly the "GUI-PATH problem" already called out
in `CLAUDE.md` for our own `yt-dlp` spawn, just one layer deeper: it also
applies to what yt-dlp itself needs to find internally. Confirmed the fix
directly: the same `--simulate` check with `--ffmpeg-location
/opt/homebrew/bin/ffmpeg` added reports `exe versions: ffmpeg 8.1, ffprobe 8.1`.

Fix: `start_download` now resolves ffmpeg via the existing
`resolve_binary_path("ffmpeg", None)` and passes it to yt-dlp explicitly via
`--ffmpeg-location` (`ytdlp/args.rs`) — only when a real absolute path was
found, never a bare `"ffmpeg"` fallback that would just duplicate yt-dlp's own
failing PATH search.

While in there, fixed a second latent bug in the same code path: `jobs.rs`
piped `child.stderr` but never read it. An OS pipe has a bounded buffer
(~64KB on macOS); if yt-dlp or the ffmpeg it spawns write more than that to
stderr without anyone draining it, the write blocks and the whole download
silently hangs. Added a dedicated stderr-draining thread that keeps a rolling
tail (last 40 lines) in memory, and wired that tail into a new
`errorMessage` field on the `job://progress` event/`JobState`, shown under a
failed row in `DownloadRow.tsx` — so a real failure now shows yt-dlp's actual
error text instead of a bare "Error" label, per the "verbose logs" approach
for confirming this bug is actually fixed.

Not re-verified through the full GUI/AppleScript loop this time (the
`--simulate`-based proof above directly isolates and confirms the root cause
without depending on YouTube's network/extractor state); worth a live
merge-format download through the real app as a final sanity check when
convenient.

### Milestone 2 — templates/config persistence, multi-URL, concurrency, path templating, options precedence, two-pass progress

Added `tauri-plugin-store` + `chrono`. New Rust modules: `commands/config.rs`
(`get_config`/`set_config`, `AppConfig { ytdlpPath, ffmpegPath, proxy,
concurrency }`), `commands/templates.rs` (`list_templates`/`save_template`/
`delete_template`/`reorder_templates`, seeds YouTube/Bilibili/TikTok on first
run, `consume_next_seq` for `{id:NNN}`), `ytdlp/path_template.rs` (resolves
`{date:YYYY-MM-DD}`/`{id:NNN}`/`{id:guid}` app-side, translates
`{original_filename}` to yt-dlp's native `%(title)s.%(ext)s`). `binaries.rs`
gained `resolve_configured` (reads the user's saved override path from
`AppConfig` — the layer nothing wired up until now) and `update_ytdlp` (`-U`,
manual-only, Config page). `jobs.rs` rewritten: `start_downloads` takes
`Vec<url>`, queues `PendingJob`s in `JobRegistry`, `fill_slots` spawns up to
`AppConfig.concurrency` (default 3) and backfills the freed slot on every
completion/cancellation; added `cancel_all`. `args.rs` gained `apply_mode`
(the three Options toggles are one mutually-exclusive strategy — a
format-selection mode strips any existing `-f`/`--format` via `shell_words`
tokenizing, not regex, then appends the mode's own flags) and wired the
`AppConfig.proxy` value through as `--proxy` (read fresh at spawn time, not
queue time, so a proxy change takes effect for still-queued jobs).

**Two-pass progress mapping** (`ytdlp/progress.rs::PassTracker`): predicts
`expects_merge` up front from the constructed format selector (contains `+`,
or no explicit `-f` at all — yt-dlp's modern default `bv*+ba/b` merges on
essentially any adaptive source) rather than reacting once a second pass is
already underway, because reacting late would mean the first pass was
already shown at raw scale and compressing the second pass afterward would
require the displayed percent to jump backward. A `last_percent` floor
guarantees the reported value never decreases even if the prediction is
wrong. Caught a real off-by-one via its own unit test: the first version
advanced `pass_index` *before* computing the current line's bounds, so a
pass's closing "finished" line jumped straight into the next pass's band
(50→95 instead of closing pass 1 at 50) — fixed by reading bounds before
advancing.

New frontend: `state/templatesSlice.ts`, `state/configSlice.ts`,
`components/Sidebar.tsx`/`TemplateRow.tsx`/`OptionsPanel.tsx`,
`pages/ConfigPage.tsx`. `MainPage` now a multi-line URL textarea, template
selection loads its fields (per-run edits, not written back automatically —
no "save template" action yet). `DownloadRow`/`JobProgressEvent` gained
`overallPercent` (the `PassTracker` output — the frontend no longer computes
its own downloaded/total ratio, which would visibly reset per merge pass)
and a `"queued"` phase.

**Bug caught during this pass, not just from unit tests**: `AppConfig.proxy`
was defined and had a Config page field, but nothing actually threaded it
into `build_download_args` — a real gap the plan called for for that only
surfaced once I checked a spawned process's actual argv via `ps -ef` rather
than trusting the config-persistence test alone. Fixed (see `args.rs`/
`jobs.rs` above); re-verified live afterward.

**Verified**, mixing live UI runs (via the same AppleScript/Accessibility
approach as M1, driving the real built app — not a mock) with unit tests and
direct process/filesystem inspection:
- Live: submitted 5 duplicate URLs with `-f 18` (avoids this environment's
  known Node/n-challenge issue — see M1 entry — and the merge case, which is
  unit-tested instead). Confirmed via `ps -ef`: exactly 3 yt-dlp children
  ran concurrently (the default cap), the output path had `--ffmpeg-location`
  and correctly resolved `{date}_{id:NNN}_{id:guid}_%(title)s.%(ext)s`
  (e.g. `2026-08-17_001_b6ee8c56-..._Big Buck Bunny....mp4`), and a 4th job
  was backfilled into a freed slot the moment one of the first 3 completed
  (`ps` showed a new PID replacing a finished one, still capped at 3). All
  3 completed files matched the expected size (28.5MB) from M1's verified
  download.
- Live: `templates.json`'s `YouTube` template's `nextSeq` was 11 after the
  first 10-job run, then correctly continued at `012` after killing and
  restarting the dev server mid-session and running one more job — confirms
  persistence survives a restart, not just a single session.
- Live: Config page proxy field — set `http://127.0.0.1:7890` (the mockup's
  own placeholder value), blurred, confirmed `config.json` persisted it, then
  confirmed a subsequently spawned job's real argv (`ps -ef`) included
  `--proxy http://127.0.0.1:7890`. Left running against that (non-functional,
  since nothing is actually listening on it) address to confirm the flag
  wiring, not proxied connectivity, which is yt-dlp's own concern; killed the
  resulting hung process afterward rather than letting it sit.
- Live: Cancel All — started a fresh 5-URL batch and clicked Cancel All
  immediately; `ps -ef` afterward showed zero running yt-dlp processes, and
  the Downloading page showed a mix of "Cancelled" (jobs that were still
  running or queued) and "Completed" (any that finished in the race window
  before the click landed) — correct either way.
- Live: Config page "check" buttons correctly showed yt-dlp/ffmpeg as found
  with real version strings on load.
- Unit tests (`cargo test`, 9 passing): `apply_mode`'s three modes (including
  correctly stripping an existing `-f`/`--format` pair), `path_template`'s
  `{id:NNN}`-style zero-padding, and `PassTracker`'s three scenarios (smooth
  single-pass, monotonic two-pass with no snap-back, and the floor holding
  even when the merge prediction is wrong).
- Not exercised: `update_ytdlp` ("Check for updates") — intentionally not
  clicked for real, since it would mutate the user's actual installed yt-dlp
  version as a side effect of a test, not something a test should do
  silently; its wiring (button present, command registered) was confirmed by
  inspection instead.

Environment note for automating this app specifically: React-controlled
text inputs don't reliably accept AppleScript's `set value of ... to
"..."` (the DOM/React state silently reverts it), and raw `keystroke`
character-by-character typing is vulnerable to macOS's smart-punctuation
substitution (typed `--` became an em-dash mid-test) and dropped
punctuation when a system dialog steals focus mid-sequence (see below) —
clicking the field, selecting all, then pasting via the clipboard
(`set the clipboard to "…"` + `keystroke "v" using command down`) was the
reliable method. A one-time macOS "bypass the system private window
picker"/screen-recording permission prompt (owned by process
`universalAccessAuthWarn`, unrelated to this app) appeared mid-run and
could not be dismissed programmatically (by design — these prompts reject
synthetic UI events) — it happened not to block subsequent `System Events`
scripting once acknowledged, but a future automation pass hitting it should
expect to need it dismissed by a human, not scripted around.

App-data store (`~/Library/Application Support/com.ytdlpty.app/`) was reset
after testing (deleted `templates.json`/`config.json`) so the next real
launch reseeds cleanly rather than inheriting test nextSeq counters and a
placeholder proxy value.

Not built (correctly out of scope — Milestone 3/4): Choose-format-first
flow (the toggle renders, disabled, in `OptionsPanel`), window
vibrancy/transparency.

Next up: Milestone 3 — `probe_formats` (`yt-dlp -j`, reusing the same shared
arg-building as downloads so the picker matches what will actually be
fetched), `ChooseFormatModal` with the per-video format table, and wiring
`chooseFormat` mode's per-URL format-id selection into `args.rs`.

### Follow-up — live-verified the merge path through the real app

The M2 write-up above noted the two-pass merge remapping (`PassTracker`) was
covered only by a unit test, not a live merge download, because this coding
session's shell had a poisoned `NODE_OPTIONS` that broke yt-dlp's Node-based
YouTube "n challenge" solver, blocking any adaptive-format test. That
assumption was wrong: the actual fix is `unset NODE_OPTIONS` combined with a
PATH that still includes a working `node` (an earlier attempt to work around
it also stripped PATH down far enough that `node` itself wasn't reachable,
compounding the failure into looking unfixable).

With that fixed, drove the real running app end-to-end: launched
`pnpm tauri dev` with `NODE_OPTIONS` unset and a full PATH, toggled "Best
video" mode in the real UI, submitted
`https://www.youtube.com/watch?v=aqz-KE-bpKQ` (Big Buck Bunny, 4K/60fps
source — the real "Best video" mode has no resolution cap, so this pulled the
full-size stream rather than a small test clip). Watched the real progress
bar advance through the video pass, audio pass, and merge phase (observed
partway through at 27% / 370.8 MiB downloaded) and reach "Completed" with no
error and no visible snap-back. Confirmed on disk: a 743MB file at
`~/Downloads/yt-dlp/2026-08-17_001_<guid>_Big Buck Bunny....mp4` — the
resolved path correctly substituted `{date:YYYY-MM-DD}`, `{id:NNN}`,
`{id:guid}`, and `{original_filename}`. `ffprobe` confirmed a valid merged
file: av1 video stream + aac audio stream, 634.6s duration. Deleted the test
output after verifying. Milestone 2's merge/two-pass-progress path is now
confirmed live, not just by unit test.

### QA pass — basic-feature audit of M1+M2

Systematic hands-on audit of every basic feature, driving the real running
app via AppleScript/Accessibility (clicks via a small CGEvent helper;
React-controlled inputs pasted from the clipboard per the M2 automation
note). Verified against `ps`/`templates.json`/`config.json`/the filesystem
rather than the UI alone. Store was backed up, cleared to force a genuine
first-launch, then restored afterwards.

**Verified working** (no change needed): first-launch seeding (3 templates,
corrected `--cookies-from-browser chrome`, no `-U`); template switching with
no state bleed; "+" add-template incl. persistence across a dev-server
restart; Options toggles as a mutually-exclusive group, with the disabled
"Choose format first" toggle correctly inert and the `-f`-replacement note
shown; multi-URL fan-out with a hard cap of 3 concurrent children (confirmed
by `ps`, parent = app pid) and queue backfill; per-row Cancel (row reads
"Cancelled", process dies, no zombie); error surfacing (bad video id showed
`ERROR: [youtube] …: Video unavailable` under the row, not a bare "Error");
Config page binary detection with resolved paths; proxy persisted to
`config.json` on blur; path templating (`{date}`/`{id:NNN}`/`{id:guid}`/
`{original_filename}` all substituted, sequence incremented 001→006 across
separate runs and survived a restart).

**Issues found and fixed:**
1. *Running jobs displayed as "Queued".* The backend spawns up to
   `concurrency` children — emitting a "downloading" event each — *inside*
   the synchronous `start_downloads` call, which only returns the job ids
   afterwards, so `addJob` always ran second and `updateJobProgress` dropped
   those events (`if (!existing) return {}`). Observed live: `ps` showed 3
   real children while all 5 rows read "Queued" through yt-dlp's whole
   extraction phase. Fixed by buffering events for unregistered jobs in
   `jobsSlice.pendingEvents` and applying them in `addJob`. Re-verified: 3
   rows show "0%" immediately, only the 4th reads "Queued", matching `ps`.
2. *No way back from the Config page.* The sidebar's "config" button doesn't
   toggle and template clicks only changed the selection, so opening Config
   stranded the user there until restart (confirmed live — clicking Bilibili
   highlighted it in the sidebar while the Config view stayed put). Fixed by
   an `onTemplateActivated` callback that returns the shell to the main view
   on select-or-add. Re-verified live.
3. *Template edits could never be saved.* Download-to/Parameters/Options
   edits were per-run only and silently discarded on every template switch,
   with no save affordance and no autosave — templates were unusable as
   presets. Added an explicit "Save to template" button (enabled only when
   the form is dirty), keeping unsaved edits per-run so a one-off tweak still
   doesn't mutate the preset. Verified live: edit → save → switch away →
   switch back reloads the saved values, and `templates.json` matches.
4. *`save_template` would have rewound the `{id:NNN}` counter.* `next_seq`
   advances in the backend on every job spawn, so any UI copy is stale by the
   time the user saves; writing it back would restart numbering and collide
   with files already on disk. Made the backend preserve `next_seq` on
   upsert. Verified: saving with 7 jobs already consumed left `nextSeq: 8`.
5. *`cancel_all` leaked zombie processes.* It killed children and cleared the
   registry without `wait()`ing, and dropping a `Child` does not reap it —
   `ps` showed three `<defunct>` entries after one Cancel All (the per-row
   `cancel_job` path already waited, and showed none). Fixed by draining and
   waiting in `kill_all_running`. Re-verified: 0 live, 0 zombies.
6. *ffmpeg's version banner swamped the UI.* `check_binary` kept ffmpeg's
   whole first line ("ffmpeg version 8.1 Copyright (c) 2000-2026 the FFmpeg
   developers"), wrapping the status text onto two lines on both the main and
   Config pages. Reduced to the version token; now reads "ffmpeg: found
   (8.1)".
7. *Internal jargon in user-facing UI.* The disabled toggle read "(coming in
   Milestone 3)" — meaningless to a user. Now "(coming soon)".

**Deferred (not built — needs a product decision, per the QA-not-features
scope):** templates can only be created and edited, never **renamed,
deleted, or reordered**. `delete_template`/`reorder_templates` exist as Rust
commands and `deleteTemplate` exists in the store, but nothing in the UI
calls them, and a new template is permanently stuck with its auto-generated
"New template N" name. Combined, "+" can only accumulate unremovable
clutter. Recommend a small per-row affordance (rename inline + a delete
control with confirmation) as its own scoped change.

Also observed, not a bug: the app doesn't pass `--no-config-locations`, so a
user's global yt-dlp config still applies (this machine's writes
`.info.json` sidecars). That seems correct — respecting user config — but
it's worth knowing when a download produces files the app didn't ask for.

Not exercised: "Check for updates" (`yt-dlp -U`) — deliberately never
clicked, since it mutates the user's real yt-dlp install as a side effect of
a test; its wiring was confirmed by inspection only, and it remains the one
Config control never run for real.

`cargo check`, `cargo test` (9 passing) and `pnpm exec tsc --noEmit` all
clean after the fixes. Test downloads deleted; the app-data store was
restored to its pre-audit contents.

### Live verification — three bugfixes + Milestone 3 (Choose format first)

Drove the real running app (AppleScript/Accessibility, CGEvent click +
scroll helpers, clipboard paste for React inputs) with `NODE_OPTIONS`
unset and a PATH keeping nvm's `node` reachable, per the `CLAUDE.md`
environment note. Everything below marked "confirmed" was observed in the
running app or in `ps`/`ffprobe`/the filesystem, not inferred from code.

**A. The three bugfixes, all confirmed live:**

- *Parameter parse errors surface.* Parameters `--user-agent "Mozilla`
  (unbalanced quote) + Download → the form showed "Parameters could not be
  parsed — check for an unbalanced quote." and **zero** yt-dlp children
  spawned. Previously this silently discarded every flag and downloaded
  anyway.
- *Concurrency cap holds under the backfill race.* Six URLs at `-f 18`,
  sampling `pgrep` every 0.1s for the whole run (400 samples): max observed
  concurrency was exactly **3**, never 4+, including at the moments slots
  freed and backfilled. All six jobs completed; `{id:NNN}` advanced
  005→010.
- *Template rename + delete.* Double-click and the hover ✎ both open the
  inline rename; Enter saved (persisted to `templates.json`), Escape and an
  emptied field both cancelled without writing. ✕ showed the inline
  "Delete <name>?" confirm (in-DOM, not a native dialog); Cancel kept the
  template, Delete removed it. Deleting the selected template fell back to
  the first remaining one. Deleting all showed the "No templates. Use + to
  add one." empty state, and "+" recovered from it. A dev-server restart
  reloaded exactly the post-edit set — no spurious reseed.

**B. Milestone 3 — Choose format first, confirmed live:**

- Toggle is enabled (no longer "coming soon") and mutually exclusive with
  Best video / Best audio; turning one on turns the others off.
- Download in this mode probes instead of downloading: modal opened with
  "Fetching available formats…", one probe process, no download started.
- The format table listed the **real adaptive set** (160/278/394, 133/242/
  395, 134/243/396, 135/244/397, 136/247, 298/302/398, 299/303/399,
  308/400, 315/401, plus audio 140/251/256/258/380 and progressive 18) —
  not a degraded progressive-only list. Storyboard entries (`sb*`, mhtml,
  both codecs "none") were absent, as intended by the filter in
  `commands/formats.rs`.
- Per-video selection: picking a row set that video's id in the list above;
  switching videos showed each one's own formats and preserved the other's
  choice; **Download stayed disabled while any probed video was still
  "not selected"** and enabled only once all were.
- **The core correctness claim.** Picking video-only `133` showed the
  "Video-only format — the best available audio will be merged in
  automatically" note, and `ps` confirmed the spawned argv carried
  `-f 133+bestaudio/133` (alongside `--no-playlist`,
  `--ffmpeg-location /opt/homebrew/bin/ffmpeg`, and a fully-substituted
  `-o` path). `ffprobe` on the 40MB result: `h264 426x240` video **and**
  `aac` audio — the pairing and merge both real.
- Progressive `18` produced a plain `-f 18` (no pairing), as it should.
- Two URLs in one batch got their own selectors in the same run —
  `-f 18` for one and `-f 160+bestaudio/160` for the other — and both files
  landed correct (`640x360 h264+aac`, `192x144 h264+aac`).
- Cancel in the modal closed it with nothing spawned and nothing on disk.
- A bad URL alongside a good one showed "failed" in red with
  `ERROR: [youtube] zzzzzzzzzzz: Video unavailable`, plus a "1 URL could
  not be read and will be skipped" line; its row was inert (clicking it did
  not change the table), it did **not** block the Download button, and the
  run spawned exactly one job — the good URL — rather than downloading the
  bad one at a default format.

**Bug found and fixed during this pass:** the picker opened with
"No video selected." and an empty table even when a single video had
probed fine — the user had to click the only row to see anything.
`ChooseFormatModal` seeded `activeUrl` via `useState` on its first render,
which happens while the probe is still running and `videos` is empty, so it
stuck at `null` and never recovered. Fixed by falling back to the first
selectable video when the stored id doesn't match
(`videos.find(...) ?? selectable[0] ?? null`), and re-verified live: the
table now populates as soon as results arrive.

**Minor finding, not fixed (a design call, not a defect):** navigating to
the Downloading page and back resets the form to the selected template's
saved values, so unsaved per-run edits (URLs, a toggle) are lost. That
follows from the documented "edits are per-run until Save to template"
model, but the loss is silent; worth revisiting alongside any future
unsaved-changes indicator.

**Still not exercised:** "Check for updates" (`yt-dlp -U`) — unchanged from
prior passes, since running it would mutate the real installed yt-dlp.

`cargo check`, `cargo test` (16 passing) and `pnpm exec tsc --noEmit` all
clean. Test downloads deleted and `~/Downloads/yt-dlp` removed; the
app-data store was backed up before testing and restored afterwards.

## 2026-08-17 — Milestone 4: macOS-native visual pass

Styling/presentation only; no behaviour changed. Verified by taking screenshots and
actually viewing them, per screen and per appearance — not by writing CSS and assuming.

**Semantic colour tokens + dark mode.** `src/styles/globals.css` previously hardcoded a
light palette and components hardcoded literals (`bg-white`, `text-black/50`,
`bg-neutral-900`). Replaced with a semantic token set (`--surface`, `--surface-sunken`,
`--glass-nav`, `--glass-overlay`, `--scrim`, `--text-primary/secondary/tertiary`,
`--border`, `--hover`, `--selected`, `--accent`, `--danger`, progress fills), fully
redefined under `prefers-color-scheme: dark`; every component now references tokens.

**Accessibility fix found by looking at it.** The mockup's green as a *fill* under white
text measures ~2.6:1 — well under the 4.5:1 the spec requires. Split into two tokens:
`--accent` (fill, darkened to #1d7a51 in light → 5.3:1 with white) and `--accent-fg`
(accent-coloured text). Dark mode inverts the relationship — a fill dark enough for white
text wouldn't separate from a dark surface, so the fill goes light (#4ec98a) and the text
on it near-black (7.7:1). Verified both selected-row states by eye.

**Window vibrancy — real, not a CSS approximation.** `transparent: true` +
`macos-private-api` + `windowEffects: ["sidebar"]`. The desktop is visibly blurring through
the sidebar in the screenshots. `titleBarStyle: "Overlay"` + `hiddenTitle` lets the sidebar
run to the top edge with the traffic lights floating over it (Finder/Mail style); sidebar
got `pt-9`, and the Downloading view `pt-10` plus a drag region, so nothing sits under the
traffic lights. Gating matters: clearing the window background is what lets vibrancy
through, so `main.tsx` adds `has-window-vibrancy` only on macOS — elsewhere the body keeps
a solid `--surface-fallback` rather than rendering transparent and unreadable.

**Three-layer rule applied**: sidebar is the only glass surface; content, form fields,
format table and download rows are opaque; the modal is an overlay-layer sheet over a
scrim. Tint audit caught a violation — the Config page's two `check` buttons were filled
accent green, out-shouting the real primary action; they are neutral now, with found/not-
found carried by text plus a status colour (never colour alone).

**Format table** (restyled from the mockup's terminal-dark look before this milestone, per
the user's note that the black background was a terminal-screenshot artifact): confirmed
running, in both appearances. All nine columns fit at the default 900px width with no
horizontal scroll — the previous layout forced a scrollbar that hid ACODEC entirely.
Numbers right-aligned and tabular, codecs shortened (`avc1.4d401e` → `avc1`, full value on
hover), selected row is a solid accent fill. `spec/ui.md` had gone stale on this point — it
still mandated the dark terminal table — so it was corrected there too, with the reasoning.

**Verified by eye**: main form, sidebar, Config, Downloading (in progress + completed), and
the Choose-format modal with a real probed adaptive format list — each in **both** light and
dark. System appearance was toggled via `System Events … set dark mode`, not
`defaults write` (the latter writes the key but doesn't apply it — the menu bar stayed light
and the app kept rendering light, which briefly looked like a dark-mode bug in our CSS).
Original appearance (light) restored.

**Packaging**: `pnpm tauri build` compiles and **bundles `yt-dlp-ty.app` successfully**. The
subsequent `.dmg` step fails with `Finder got an error: AppleEvent timed out (-1712)` — its
cosmetic pass drives Finder over AppleScript, which doesn't respond in this non-interactive
session. Environment/automation issue, not an app defect; expected to succeed in a normal
interactive session, or build only the `app` target to skip it.

**Finder-launch test passed — the binary-resolution design is now proven.** Launched the
built `.app` through LaunchServices (inherits no shell PATH) and both binaries resolved:
"yt-dlp: found (2026.07.04) · ffmpeg: found (8.1)". `config.json` was `{}` at the time, so
no saved override path shortcut the test — the candidate-path probing itself did the work.
This is the exact scenario that probing exists for and it had never actually been run.

**Deferred**: default Tauri app icon still in place — a custom icon is the user's design
call, not something to invent. `macos-private-api` disqualifies Mac App Store distribution
(already a recorded non-goal). Not exercised: `yt-dlp -U`, which would mutate the real
install.

### Follow-up — spec/table polish

Reviewed the packaged `.app` by screenshot and fixed one holdover: the URL and Parameters
textareas still had the webview's drag-to-resize grabber, which has no equivalent in a
native text field and is one of the clearest tells that a window is a web view
(`resize-none`).

Documented the format table properly in `spec/ui.md`. The earlier correction there
explained *why* the table is no longer dark terminal styling, but the Components section
never described what it now *is*, so the real decisions were undocumented and liable to be
"cleaned up" by whoever touches it next. Each is now recorded with the defect it fixed:
everything must fit the default 900px window with no horizontal scroll (the first layout
let columns self-size and hid ACODEC behind a scrollbar), codecs shorten to their family
with the full value on hover (the long ids caused that overflow), numbers are right-aligned
and humanised with an em dash for absent values rather than a blank cell, headers use
sentence case instead of yt-dlp's CLI uppercase, and audio-only rows dim the resolution
cell so scanning skips them without relying on colour alone.

Also corrected the Typography section, which still read "Monospace (format table)" — true
of the old terminal table, and an invitation to bring monospace back wholesale. It now
states the actual rule: mono only where character alignment earns it (format ids, path
template, Parameters), never on prose, with numeric columns using `tabular-nums` on the
proportional face. One code fix fell out of writing that up: `FormatTable` hardcoded
`rounded-[12px]` instead of the `--radius-md` token, so the spec's single-source geometry
rule was only coincidentally satisfied.

## 2026-08-17 (later) — v0.1.1: post-M4 fixes from real use

Hands-on testing after Milestone 4 turned up a batch of defects, several of
them in code shipped by earlier milestones. Grouped by what they broke.

**The window could not be dragged — two independent causes.** Milestone 4 set
`titleBarStyle: "Overlay"` + `hiddenTitle` so the sidebar could run to the top
edge, which removes the OS title bar you would normally drag by. Fixing that
took two goes:
1. The replacement `data-tauri-drag-region` was *bare*. Reading Tauri's own
   `src/window/scripts/drag.js` (2.11.5) settles what that means: a bare
   region only drags when the click lands on that exact element
   (`return el === composedPath[0]`); `"deep"` is what makes a subtree
   draggable. The sidebar is almost entirely covered by its children, so only
   the hairline gaps between rows worked. Both regions are now `"deep"`;
   Tauri still exempts buttons and inputs, so controls keep working.
2. Even then nothing moved, because the drag script invokes
   `plugin:window|start_dragging` over IPC and **`core:window:default` does
   not grant it** — it grants only `allow-internal-toggle-maximize`, the
   double-click-to-maximize half. The ACL denied the call silently, with no
   error anywhere in the UI. Added `core:window:allow-start-dragging` to
   `capabilities/default.json` (this one needs a rebuild; capabilities are
   compiled in, so hot reload can't deliver it).

Then it regressed a third time on the Downloading view, because `App.tsx` had
a duplicated early return for it that rendered neither the sidebar nor the
drag strip. Collapsed both branches into one shell that owns the drag strip
and the traffic-light inset, with the sidebar conditional — a new view can no
longer forget the window chrome. Note `main` deliberately paints no
background: the old branch did, and in a unified shell that would sit behind
the sidebar and kill the vibrancy.

**The format picker failed or lied on real sites.** Reported against PornHub.
The user's theory was that argument order mattered; running both orderings
disproved that (they fail identically — their confirming test had a `--j`
typo, which yt-dlp prefix-matched to a JS-runtime option and swallowed
`--no-color` as its value). The real cause: `-j` doesn't only dump metadata,
it also **resolves the format selector**, and aborts with "Requested format is
not available" whenever the default `bv*+ba/b` matches nothing — i.e. it fails
on exactly the videos the picker exists for, while `-F` merely lists and so
always worked by hand. `build_probe_args` now passes
`--ignore-no-formats-error`: selection is the user's job in this flow, and a
genuinely unavailable video still returns an empty list.

Fixing that exposed a worse bug of ours. The storyboard filter dropped any
format where **both** codecs were `"none"` — but plenty of extractors report
no codec metadata for direct progressive downloads (PornHub's
`240p`/`1080p`/`2160p` have null vcodec *and* null acodec). Six of twelve
formats, often the best ones to pick, were being discarded before reaching the
table. Storyboards are now identified by their `mhtml` container, which still
excludes YouTube's `sb0`-`sb3` sheets. Three regression tests pin this using
the real JSON shapes from both sites.

**Silent failures made diagnosable.** A probe failure kept only the *last*
non-empty stderr line, which with yt-dlp is the least informative part — the
actual cause is usually a WARNING several lines above the terminating ERROR.
Now keeps a 25-line tail. Added `CommandDetails`: both the picker and download
rows expose the exact shell-quoted invocation (resolved binary path included,
so you can see *which* yt-dlp ran and whether cookie/proxy flags reached it),
plus the failure log. The command is emitted once on the spawn event rather
than every progress tick, and the reducer preserves it as later ticks arrive
with null. The modal previously showed only `failed[0].error`, so with several
bad URLs you saw one message and couldn't reach the others; failures are
per-row now, and failed rows became selectable (they were `disabled`, which is
how you reach their log) while staying excluded from the Download gate.

**Two more backend defects found by reading, not by symptom:**
- `fill_slots` checked `running_count()` before the slow `Command::spawn`, so
  two jobs finishing at the same instant could both claim one slot and exceed
  the concurrency cap. Now holds `JobRegistry::lock_fill` across the whole
  check→pop→spawn→insert sequence.
- `apply_mode`/`build_download_args` used `shell_words::split(...)
  .unwrap_or_default()`, so a single unbalanced quote in Parameters silently
  discarded **every** flag — cookies, proxy, `--no-playlist` — and downloaded
  anyway, looking normal. Parameters are now tokenized once up front and a
  parse failure surfaces as a UI error; they travel as `Vec<String>` end to
  end so the failure mode can't return.

**Smaller UI corrections:** Parameters accepts one flag per line (already
worked — `shell_words` treats newlines as separators — now pinned by tests and
advertised by the placeholder, since nothing communicated it); the Parameters
field grows to share a bottom baseline with the action buttons, which moved
into the right column because as a full-width row underneath they forced a
band of dead space; "Saved" now appears on the Save button itself for ~1.8s
instead of as a floating note that shifted the layout; the config button no
longer uses the filled surface that `TemplateRow` uses for *selection* (it
read as a fourth, permanently-selected template) and gained a sliders icon —
a gear turned to mush at 15px; content is capped and centred on all three
pages rather than hugging the left edge on a stretched window, which also
aligned the Config title with the template title.

Verified: 22 Rust tests, `tsc --noEmit`, `cargo check` clean. The probe fix
and format filter were checked against live output from both sites. Not
verifiable from here: an actual mouse drag, and `yt-dlp -U`.

## 2026-08-18 — v0.2.0: SQLite storage, download history, UI corrections

### SQLite replaces the JSON stores

Templates, settings, and the new history now live in `ytdlpty.sqlite3` in the
app data dir, via `rusqlite` with the `bundled` feature. Direct SQL rather
than `tauri-plugin-sql`: every caller (templates, config, history, the job
runner) is already Rust-side, so routing persistence through the webview
would add an IPC hop and put raw SQL in the frontend for nothing. `bundled`
also means no system SQLite dependency, which matters for the Windows and
Fedora testing to come. WAL mode, since a long read (the history list) and a
write (a job finishing) genuinely overlap here.

Migration precedence is existing rows → legacy JSON → built-in seeds, so an
upgrade imports rather than reseeds. The legacy `templates.json` / `config.json`
are deliberately left in place as a fallback copy. Verified against the real
store, not a synthetic one: all seven customised templates came across with
their modes, download paths, order, and — critically — `next_seq` intact.
Resetting that counter would have restarted `{id:NNN}` numbering and collided
with files already on disk.

Command names and their wire shapes are unchanged, so the frontend contract
didn't move.

### Download history

One row per terminal outcome, written from the job runner. The output path and
platform come from yt-dlp itself via two `--print after_move:` flags (a `WHEN:`
prefix means neither `--quiet` nor `--simulate` is implied; two flags rather
than one delimited line because `\t` inside a single `--print` is emitted
literally). Size is read from the filesystem afterwards, which is authoritative
— yt-dlp's total is an estimate for adaptive streams and describes neither
source stream after a merge. `template_name` is snapshotted at queue time,
since the template can be renamed or deleted while the job is still queued.

Failures and cancellations are recorded too, but the view lists only completed
downloads: showing failures would need a status column that wasn't asked for,
and such rows have no filename or size to fill the columns that exist.

The view is a plain list like the Downloading page, reached from a History
button beside Config. Each row shows filename, size, finish time with
duration, `platform · template`, the URL, and the exact command — each of the
last two with its own labelled copy button ("copy link" / "copy command"),
because two bare "copy" buttons in one row are distinguishable only by
position. Scoped deliberately narrow: no thumbnails, no open/reveal action, no
search or export. It records what was fetched; it is not a media library.

### A migration bug worth remembering

Bumping `SCHEMA_VERSION` and adding the `ALTER TABLE` it described landed in
two separate edits, and the dev server rebuilt in between. The app started,
saw `current < 2`, bumped `user_version` to 2, and ran a migration that did
not exist yet. Once the counter said 2 the version-gated step could never run
again, leaving the `command` column permanently absent while the code selected
it — which silently drops every row through `filter_map(Result::ok)`.

Fixed by making additive migrations check the real schema (`PRAGMA
table_info`) instead of trusting the counter to be in step. That is
self-healing, costs one pragma at startup, and repaired the affected database
in place with all rows preserved. A version counter is a fine trigger for
*ordered* work but a poor witness to what the tables actually contain.

### UI corrections from real use

- **Format table columns.** The ID column was fixed-width and long site ids
  (`dash-1796271141547558v`) wrapped onto two lines, knocking the rest of the
  row out of vertical alignment. ID now takes the flexible column and the
  codec columns are fixed — they never needed more than four characters. An
  attempt at left-truncation (so ids sharing a prefix stayed distinguishable
  by their tail) was reverted: `direction: rtl` on mixed Latin/digit text
  produced a leading ellipsis *and* a clipped final glyph.
- **Format list scrolling** no longer depends on the modal's flex chain to
  bound it; the rows body caps at `50vh` and scrolls on its own. A percentage
  `max-h` that fails to resolve leaves the sheet unbounded, so the list is
  clipped with no scrollbar at all.
- **The modal covers the shell's drag strip**, so it now carries its own drag
  handle on the title row — otherwise the window can't be moved while a sheet
  is open.
- **"Options (override)" is now "Format"**, and the toggles name complete
  outcomes: "Best video + audio" (which is what the old "Best video" always
  did) and "Audio only". The old pair read as two combinable halves, which
  made the mutual exclusion look like a bug. Stored `mode` values unchanged.
- **Downloading queue** scrolls with its action bar pinned outside, so the
  page's bottom padding stops collapsing against the window edge once the
  queue overflows, and Cancel All is reachable without scrolling past every
  job.
- **Window opens at 1100×780** (minimum 880×600, centred). 1100 is where the
  format sheet reaches its max width; the old 760 minimum squeezed the ID
  column to about eight characters.
- `CopyButton` extracted, so the clipboard path exists once.

README gained a production-build section: per-platform output paths, the
runtime yt-dlp/ffmpeg requirement, where the database lives, and the two real
blockers — the `.dmg` step needing an interactive session, and the build being
unsigned so Gatekeeper blocks it elsewhere.

Verified: 22 Rust tests, `tsc --noEmit`, `cargo check` clean. Migration and the
history view were checked against the live database and by viewing the running
app. History capture is confirmed end-to-end by real downloads recording
correct filenames, platforms, sizes, and durations.

## 2026-08-19 — v0.2.1: scroll regions, queue rendering, probe cancel

### Bounded lists were unreadable in three different ways

All three reports looked like "content is cut off", and only one of them was.

**Scrollbars were invisible.** macOS overlay scrollbars fade out at rest, so a
list that scrolls perfectly well reads as a list that was truncated. Added a
`.scroll-area` class (thin 8px, `--border-strong` thumb, transparent track,
visible at rest) and applied it per region — the download queue, history, the
format rows, the modal's URL list, the sidebar, and the command/error `<pre>`
blocks. Deliberately not a global `*` rule: a blanket `::-webkit-scrollbar`
would also put a horizontal bar under the format table, whose grid columns are
tuned to fit the default window exactly.

**The format table had two live scrollers.** Its rows capped at `50vh` while
the modal body also scrolled, and at typical window heights both were active:
the wheel chained between them and the command block sat outside whichever one
was under the cursor. The rows are now the only scroller, sized by a `min-h-0`
chain (sheet `max-h-[85vh]` → body → table root → rows), with the command block
pinned below as a `shrink-0` sibling. The column header moved *inside* the
scroller as `sticky top-0`, because a space-consuming scrollbar takes width
from whatever it is attached to — a header outside the scroller would sit 8px
wider than the rows and knock every column out of alignment. This reverses the
"scrolls on its own rather than relying on an ancestor" note from v0.2.0; that
cap was a workaround for a flex chain missing `min-h-0`, and the chain is now
correct at every link.

**The download queue was the only real clipping bug.** Rows set `h-11` but a
flex item shrinks below its own height by default, so a 40-job queue compressed
every row to a few pixels — URL, percentage and Cancel button all clipped away,
no overflow, nothing to scroll. `shrink-0` on `DownloadRow` and `HistoryRow`
fixes it. Reproduced with a throwaway `VITE_FAKE_JOBS` harness seeding 40 jobs,
confirmed against the running app, harness removed.

### Cancel during a format probe did not cancel

Clicking Cancel while formats were still being fetched closed the sheet, and
then the in-flight `probeFormats` promise resolved and wrote its result into
state — reopening the sheet with the finished table, which read as the button
doing nothing. Each probe now takes a run token from a ref; Cancel bumps it and
a resolving probe that no longer owns the picker discards both its result and
its `setProbing(false)`, so a quick Download → Cancel → Download can't leave the
second probe out of its loading state. Note the yt-dlp probe children are still
not killed — they run to completion and their output is thrown away. Making
that a real cancel needs a probe registry and a `cancel_probe` command.

### Smaller corrections

- The format sheet covers the shell's title-bar drag strip, so the window could
  not be moved while it was open. The overlay now carries its own strip at the
  same height, with the sheet `relative` so it paints above it; the sheet's
  title row remains a second handle for short windows where the two overlap.
- The collapsed "command" line in a queue row sat flush against the card's
  bottom edge.
- `tauri-plugin-store` dropped from `Cargo.toml`, `package.json` and the
  builder. Nothing had read through it since v0.2.0 — the legacy-JSON import
  parses the file directly with `serde_json` — and the capability file never
  granted `store:*` anyway.
- `CLAUDE.md` still described templates and config as JSON via
  `tauri-plugin-store`; it now describes the SQLite database, its per-platform
  location, and the legacy files' status.
- `spec/ui.md` gained a "Scroll regions" entry: the class, one-scroller-per-
  region with `min-h-0` on every link, and why a column-aligned header belongs
  inside the scroller.

Verified: `cargo check`, `cargo test`, `tsc --noEmit`, and `pnpm build` clean,
plus the queue and format sheet checked in the running app at a short window
height.

## 2026-08-19 (later) — v0.2.2: partial probe cancel

Cancelling the format picker now tells the backend to stop, via a `ProbeEpoch`
counter in app state (`commands/formats.rs`). `probe_formats` reads the counter
once at start; `cancel_probe` bumps it; the batch loop compares the two between
chunks and stops spawning further ones. A counter rather than a flag so that
cancelling batch N cannot also abort a batch N+1 the user started a moment
later. Whatever was collected before the break is still returned — the frontend
run token added earlier is what discards the result of a probe nobody owns any
more, and the two mechanisms are not interchangeable: the token stops the UI
from acting on a result, the epoch stops the backend from doing more work.

**This is deliberately partial.** yt-dlp children already running are left to
finish; only the *next* chunk is prevented. With a single URL there is one
chunk holding one process, so nothing is actually stopped and the call is
bookkeeping only. It pays off for a multi-URL paste, where chunks after the
first are the bulk of the work.

### Deferred: killing the probe children

Recorded here so the design does not have to be rediscovered. Worth doing when
the app has enough users that wasted background work matters; not before.

`probe_one` uses `Command::output()`, which spawns and waits in one call and
never yields a `Child`, so there is nothing to kill. Making probes killable
means spawning them the way `commands/jobs.rs` already spawns downloads:

1. `registry.rs` — hold probe children keyed by a batch id,
   `HashMap<String, Vec<Child>>`, plus a `kill_probe(id)`. One probe fans out
   across URLs, so the unit is a group, not a single child.
2. `formats.rs` — `probe_one` spawns with piped stdio and registers its child.
   The trap: `output()` drains stdout and stderr *concurrently*, and reading
   them sequentially deadlocks once a chatty yt-dlp fills the stderr pipe
   buffer while the reader is blocked on stdout. `wait_with_output()` avoids
   that but consumes the `Child`, so it cannot be combined with keeping the
   child in a registry. `jobs.rs:225-234` already solves exactly this — take
   both pipes, spawn a stderr reader thread, register the child, drain stdout,
   then `registry.remove().wait()`. `wait` is mandatory: a killed-but-unwaited
   child lingers as a zombie (`jobs.rs:104`).
3. `lib.rs` — extend `cancel_probe` to kill the batch, keeping the epoch bump
   so the between-chunk stop still applies to queued chunks.
4. Frontend — mint a batch id per probe and pass it to both calls. The run
   token stays regardless: a killed probe still returns partial results that
   the UI must ignore.

Verified for this change: `cargo test` (22 passed), `cargo check`, `pnpm build`.
The epoch path is not covered by a test — it needs a real multi-chunk batch and
a cancel mid-flight, which is an integration test this repo has no harness for.

## 2026-08-21 — v0.2.3: queue no longer accumulates old results; existing files are skipped visibly

Two user-reported issues, plus one latent bug found while fixing the second.

**Finished jobs stayed on the Downloading page.** Nothing ever removed a job
from the store, so a failed (or completed) row from an earlier run sat above
the batch just started and read as part of it. `clearFinishedJobs` in
`state/jobsSlice.ts` drops every job in a terminal phase and is called from
`MainPage.startJobs` right before the backend call. Jobs still in flight are
deliberately kept: starting a second batch must not hide the first one's
running downloads. `pendingEvents` is left alone — an event is only ever
buffered for a job the store hasn't registered yet, so none of them belong to
a job being cleared.

**"Skipped — file exists" as its own status.** yt-dlp already refuses to
overwrite an existing final file (default `--no-force-overwrites`); it prints
`[download] <path> has already been downloaded`, fetches nothing, and exits 0
— which the app used to report as plain "Completed". The reader now watches
for that line (`progress::is_already_downloaded_line`) and reports phase
`skipped` when the job succeeded *and* nothing was transferred. Both halves
matter: a playlist where some entries were on disk and others were fetched is
a completed download, not a skipped one, which is what `saw_transfer` tracks.
The skip case also emits one synthetic `finished` progress line, suppressed so
a job that transferred no bytes doesn't flash "Merging…" before its label.
History records `skipped` as its own status; the visible list still filters to
`completed`, so a skip doesn't add a second row for a file already listed from
the run that fetched it.

*Name-based, not name+size, and deliberately so.* The requested size check has
nothing trustworthy to compare against: an interrupted download is still a
`.part` and never matches the final name in the first place, and the only size
available before fetching is one stream's `total_bytes` estimate — for the
merge case (two streams into one container) it describes neither the file on
disk nor the file that would be written, so size-gating would false-negative
on exactly the jobs it was meant to protect. A skip is only ever "this exact
output path already exists, complete".

**`--print` was silently killing all of stdout.** Found while looking for the
skip line: `--print` *implies `--quiet`*, and quiet suppresses far more than
the banner. With yt-dlp 2026.07.04 and the two `--print after_move:` flags this
app has been passing, stdout carried those two lines and nothing else — no
`--progress-template` lines, so the progress bar only ever moved on the
terminal event (0 → 100), and no `[Merger]` lines, so `is_merge_line` /
`PassTracker::observe_merge` had never once fired in the running app.
`build_download_args` now passes `--no-quiet`, which restores all three. A
regression test pins it, since the flag looks removable and takes three
features with it.

Verified: `cargo test` (26 passed), `tsc --noEmit`, and live yt-dlp runs with
the machine's real `~/.config/yt-dlp/config` active (cookies, proxy,
`--write-info-json`, `--merge-output-format mp4/mkv`) rather than
`--ignore-config`:

- Two-pass merge job (`-f bestvideo+bestaudio/best`, YouTube): 8 `downloading`
  + `finished` per pass, then one `[Merger]` line — the exact shape
  `PassTracker` bands assume, so the restored progress path is monotonic on
  real input.
- Re-running that same job: `has already been downloaded` on stdout for the
  merged `.mkv`, no `dl:` lines at all, file mtime unchanged, exit 0.
- Single-format job (generic extractor): same skip line, plus the one
  `dl:…|finished|NA|…` line the suppression guard exists for.

**Sidebar title aligned with the page heading.** `yt-dlp, thank you!` (now
lower-case, in `Sidebar.tsx` and as the window title in `tauri.conf.json`) sat
about 7px above the template name opposite it, because it was 19px with
`leading-tight` while page headings are 20px at the default 1.5 leading. Both
columns already start at the same y — the sidebar's `pt-9` equals the content
column's 36px drag band — so matching the type and adding MainPage's own 4px
`pt-1` offset puts the two titles on one baseline. 20px/semibold is also what
`spec/ui.md` specifies for the site name, which the 19px value had been quietly
undercutting.
