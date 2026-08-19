# yt-dlp-ty

A cross-platform desktop GUI for `yt-dlp`, styled to feel like a native macOS app (Tauri v2
+ React/TypeScript/Vite frontend, Tailwind CSS). See `DEVLOG.md` for current status and
`~/.claude/plans/synthetic-finding-forest.md` for the full build plan.

## Stack & Conventions

- Backend: Rust via Tauri v2 (`src-tauri/`). Frontend: React + TypeScript + Vite (`src/`).
- Package manager: **pnpm only** — do not add npm/yarn lockfiles.
- Templates (per-site presets), app config, and download history persist in one SQLite
  database in the app's data dir (`~/Library/Application Support/com.ytdlpty.app/` on
  macOS, resolved per-platform by Tauri's `app_data_dir()`), not in this repo. Accessed
  from Rust only, via `rusqlite` — see `src-tauri/src/db.rs`. The `templates.json` and
  `config.json` beside it are the pre-SQLite `tauri-plugin-store` files, read once on
  migration and then left alone as a backup copy.
- All yt-dlp argument assembly and output-path templating logic lives in Rust
  (`src-tauri/src/ytdlp/`) as the single source of truth. Any frontend preview of args/paths
  is display-only — if it and Rust disagree, Rust wins.

## Binary Policy

`yt-dlp` and `ffmpeg` are **system-installed dependencies the app detects**, never bundled.
A `.app` launched from Finder does not inherit the user's shell PATH, so binary resolution
must check common install locations (Homebrew, `~/.local/bin`, etc.) and support a
user-supplied override path — do not regress to a bare `Command::new("yt-dlp")` assuming
PATH is inherited.

## Local Dev/Test Environment

This machine's default shell has a `NODE_OPTIONS` env var (set by the coding-agent tooling,
not this project) that breaks Node's filesystem access. yt-dlp uses a Node-based JS solver
for YouTube's "n challenge" on any adaptive format (i.e. everything except legacy
progressive formats like `-f 18`) — with that `NODE_OPTIONS` inherited, adaptive-format
downloads and `yt-dlp -j`/`-F` format listing fail with a spurious "Requested format is not
available" error that has nothing to do with this app's code. Before running `pnpm tauri dev`
or any live yt-dlp test against real adaptive YouTube formats, unset it while keeping a PATH
that still includes a working `node`:
```
bash -c 'unset NODE_OPTIONS; export PATH="<nvm-node-bin>:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"; <command>'
```
Dropping PATH down to just system dirs (rather than unsetting `NODE_OPTIONS` while keeping
`node` reachable) hides `node` entirely and looks like the same failure — see `DEVLOG.md`'s
Milestone 2 merge-verification entries for how this was misdiagnosed once already.

## Docs

- `spec/ui.md` — the UI design system. Read before touching any component styling.
- `README.md` — user-facing description and setup.
- `DEVLOG.md` — running log of what's been built, one entry per milestone.

## Non-Goals (for now)

- No bundled yt-dlp/ffmpeg binaries.
- No Mac App Store distribution — native vibrancy uses Tauri's `macos-private-api`, which
  disqualifies MAS submission. Direct-download distribution only.
- No auto-update system unless explicitly requested later.
