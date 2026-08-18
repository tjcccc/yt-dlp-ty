# yt-dlp-ty

A pretty, cross-platform desktop GUI for [yt-dlp](https://github.com/yt-dlp/yt-dlp),
styled to feel like a native macOS app.

**Status: working, in development.** Downloading, templates, config, and the format picker
all run. Remaining: the macOS "Liquid Glass" visual pass and packaging (Milestone 4). See
`DEVLOG.md` for progress and `spec/ui.md` for the design system.

## Features

- **Site templates** — saved per-site presets (YouTube, Bilibili, TikTok, ...) in a sidebar,
  each with its own default URL box, output-path template, raw yt-dlp parameters, and
  format-selection options. Rename by double-clicking; add with `+`; delete with `✕`.
  Edits to the form are per-run until you press **Save to template**.
- **Multi-URL downloads** — paste multiple URLs (one per line); each becomes its own tracked
  job, 3 running at a time with the rest queued.
- **Templated output paths** — e.g.
  `~/Downloads/yt-dlp/{date:YYYY-MM-DD}_{id:NNN}_{id:guid}_{original_filename}`.
  `{date:YYYY-MM-DD}` is today's date, `{id:NNN}` a per-template counter (zero-padded to the
  number of `N`s), `{id:guid}` a random uuid, and `{original_filename}` the video's own title.
- **Options (override)** — one mutually exclusive choice: *Best video*, *Best audio*, or
  *Choose format first*. Any of them replaces a `-f`/`--format` flag in your Parameters.
- **Choose format first** — lists each video's real formats (resolution, codec, size) to pick
  from before downloading. Video-only formats automatically get the best audio merged in.
- **Live download progress** — per-video progress bars with cancel / cancel-all. A
  video+audio download reports one smooth 0-100%, not one bar per stream.
- **Config page** — detect installed `yt-dlp`/`ffmpeg` (or point at a custom binary path),
  set a proxy, and manually check for yt-dlp updates.

## Prerequisites

This app does not bundle `yt-dlp` or `ffmpeg` — install them yourself first, e.g. via
[Homebrew](https://brew.sh):

```sh
brew install yt-dlp ffmpeg
```

The app's Config page detects these automatically once installed, or lets you point at a
custom binary path.

## Running it

```sh
pnpm install
pnpm tauri dev
```

The first `pnpm tauri dev` compiles the Rust backend and takes a few minutes; later runs are
fast. Stack: Tauri v2 (Rust) + React + TypeScript + Vite + Tailwind, pnpm only.

Downloads land under `~/Downloads/yt-dlp/` by default (configurable per template). Checks:

```sh
pnpm exec tsc --noEmit          # frontend types
cd src-tauri && cargo test      # backend unit tests
```

## Building a production app

```sh
pnpm install
pnpm tauri build
```

Output lands under `src-tauri/target/release/bundle/`:

- **macOS** — `macos/yt-dlp-ty.app`, plus a `.dmg` if that step succeeds
- **Windows** — `msi/` and `nsis/` installers
- **Linux** — `deb/`, `rpm/`, and `appimage/`

The first build compiles the whole Rust dependency tree and takes a while;
later builds are much faster. `src-tauri/target/` grows to several GB — it is
a disposable cache, safe to delete with `cargo clean`.

`yt-dlp` and `ffmpeg` are **not** bundled. A built app detects them at
runtime, so they must be installed on the target machine (see Prerequisites).
A Finder- or Explorer-launched app inherits no shell `PATH`, which is why the
app probes common install locations and offers a manual path override on the
Config page.

### Known issues

- **The macOS `.dmg` step can fail** with `Finder got an error: AppleEvent
  timed out (-1712)`. That step drives Finder over AppleScript to lay the
  disk image out, and it needs an interactive session — it fails when run
  headless or over SSH. The `.app` itself is already built and usable at that
  point; `pnpm tauri build --bundles app` skips the `.dmg` entirely.
- **The build is unsigned and un-notarised.** macOS Gatekeeper will refuse to
  open it on any machine other than the one that built it, with a message
  about an unidentified developer. Right-click the app and choose *Open* to
  bypass it once. Proper signing and notarisation (an Apple Developer
  account, `codesign`, and `notarytool`) is the real fix and is not set up
  here.
- **macOS window vibrancy uses `macos-private-api`**, which disqualifies the
  app from the Mac App Store. Direct distribution only — a recorded non-goal,
  not an oversight.

### Where your data lives

Templates, settings, and download history are stored in a SQLite database in
the platform app-data directory (on macOS,
`~/Library/Application Support/com.ytdlpty.app/ytdlpty.sqlite3`). Deleting it
resets the app to its seeded templates. Nothing is written into the repo.

## License

See `LICENSE`.
