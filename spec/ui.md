# UI Design System — yt-dlp-ty

Project-layer UI spec, per the user's global `~/.claude/docs/ui-principles.md` ("UI
constitution": restraint over decoration, clear hierarchy, accessible contrast, no
glass-on-glass). This doc extends that global policy with project-specific tokens and
component rules; it must never contradict it. No prior visual system exists (this is a
from-scratch app), so this spec is the design direction, not a documentation-only pass —
and once established, later UI work should conform to it rather than improvise.

## Goal & Constraint

Visual target: **macOS 26 (Tahoe) "Liquid Glass"** — Apple's current native design
language. This app is a cross-platform **Tauri webview** app, so true Liquid Glass
(real-time light lensing/refraction, motion-reactive specular highlights) is an AppKit
system material that cannot be reproduced in CSS. The goal is a faithful **CSS
approximation** of the aesthetic and design language — translucency, concentric rounded
geometry, restrained semantic tint — not a pixel-identical clone of the system material.

Sourcing note: macOS-specific claims below (sidebar refraction/reflection, concentric
corners, transparent menu bar) are from Apple's own announcement
([Apple Newsroom, June 2025](https://www.apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design/)).
General Liquid Glass rules (materials, layering, tint semantics) are from a community
SwiftUI/iOS reference and are treated as directionally reliable for concepts, not for
iOS-specific numbers (touch targets, tab bars) which don't apply to a desktop window.

## Layer Rules (where glass is and isn't allowed)

Three layers, translated to CSS:

1. **Content layer — always opaque, never glass.** The URL textarea, Download-to field,
   Parameters textarea, and the format table are solid backgrounds. Per the global
   anti-pattern rule, decoration never substitutes for structure — data-entry and
   data-display stay legible and flat.

   The format table is styled as a **native macOS list view**, not terminal output. An
   earlier draft of this spec called for a "dark, monospace, terminal-style" table because
   the mockup showed one — but that mockup panel was a screenshot of `yt-dlp -F` in a
   terminal, i.e. provenance rather than design intent (confirmed by the user, 2026-08-17).
   A black console panel inside a light window mixes two visual languages on one screen,
   which the anti-patterns below forbid. Monospace is retained only where digit alignment
   earns it (format ids and numeric columns).
2. **Navigation layer — glass.** The sidebar (`backdrop-filter: blur(...)` + a
   semi-transparent fill) sits above the content, refracting/reflecting what's behind it in
   spirit (a subtle gradient/blur, not literal reflection).
3. **Overlay layer — glass.** Modal windows (Choose-format-first, Downloading) get an inset,
   translucent, rounded "sheet" treatment with a soft shadow.

**No glass-on-glass stacking** — a glass sheet never opens on top of the glass sidebar
without an opaque layer between them.

## Geometry

Corner radius scale, applied consistently rather than ad hoc per component, so nested
shapes read as concentric (mirrors macOS's controls-fit-the-window-corner principle):

- `--radius-sm: 8px` — small controls, toggle pills, table rows.
- `--radius-md: 12px` — inputs, buttons, cards.
- `--radius-lg: 20px` — sidebar panel, modal sheets, the app window itself.

## Color & Tint

Neutral, system-adaptive palette via CSS custom properties, switching on
`prefers-color-scheme`. Components must reference the semantic tokens defined in
`src/styles/globals.css` (`--surface`, `--text-secondary`, `--border`, …) rather than
literal utilities like `bg-white` or `text-black/50`, so both appearances stay defined in
one place. **Tint is semantic, not decorative** — reserved for primary action/selected
state only:

- Accent tint (green, matching the mockups): the Download button and an "on" toggle state.
- The accent exists as **two** tokens, because one green cannot do both jobs legibly:
  `--accent` is a *fill* carrying `--text-on-accent`, and `--accent-fg` is accent-coloured
  *text* on an ordinary surface. The mockup's pale green managed only ~2.6:1 against white
  text; the fill is darkened in light mode, and in dark mode the relationship inverts (light
  fill, near-black text) so the selected row still separates from a dark surface.
- Everywhere else: neutral grays/whites/blacks with alpha for the glass fills.
- Status colors (success/error/cancelled in the Downloading view) follow standard
  conventions (green/red/gray) — never rely on color alone; pair with text/icon.

## Typography

Font stack: `-apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif`.
Monospace: `ui-monospace, "SF Mono", Menlo, monospace` — applied narrowly, only where
character alignment earns it (format ids, the download-path template, the Parameters
field). Prose never gets it: monospace on a phrase like "not selected" reads as terminal
output and was part of what made the format table look like an embedded console. Numeric
columns use the proportional face with `tabular-nums` rather than a mono face, which keeps
digits aligned without changing the type.

Type scale — kept small and controlled per the global "avoid unnecessary size/weight
variation" rule:

- Heading (site name, page titles): 20px / semibold.
- Label (field labels, sidebar template names): 14px / medium.
- Body (input text, table cells): 13px / regular.

## Components

- **Toggle switches**: rounded pill, green fill when on, matching the mockup — used as a
  mutually-exclusive radio group for the "Options (override)" section (see plan §Options
  toggle precedence), not independent booleans.
- **Buttons**: primary action (Download) uses the accent tint per the glass-tint rule above;
  destructive actions (Cancel, Cancel All) use a restrained red, not full-saturation alarm
  red.
- **Cards/panels**: consistent padding and the geometry scale above; no decorative borders
  or shadows beyond what signals elevation (content vs. navigation vs. overlay layer).
- **Format table** (`src/components/FormatTable.tsx`): a native macOS list view — opaque
  `--surface` on a `--radius-md` bordered container, a `--surface-sunken` header row in
  11px `--text-tertiary`, 12px rows separated by hairline `--border` dividers, and
  `--hover` on hover. Selection is the accent *fill* with `--text-on-accent`, matching how
  a Finder list marks a selected row; nothing else in the table is tinted. Specifics worth
  preserving, each of which fixed a real defect:
  - **Everything must fit the default 900px window with no horizontal scroll.** Fixed
    widths for the seven leading columns, with the two codec columns sharing the remainder
    (`minmax(0,1fr)`). The first version let columns size themselves and pushed ACODEC
    behind a scrollbar, so the audio codec — half the reason to consult the table — was
    invisible until you scrolled.
  - **Codecs are shortened to their family** (`avc1.4d401e` → `avc1`), full string on
    hover via `title`. The long identifiers are what forced the overflow, and only the
    family is scannable.
  - **Numbers are right-aligned and humanised**: sizes as MB/GB rather than raw bytes,
    bitrate as `256k`, and an em dash for absent values — never a blank cell, which reads
    as a rendering bug rather than as missing data.
  - **Column headers use sentence case** (`Resolution`, `Size`, `Bitrate`), not yt-dlp's
    uppercase. Its `-F` output is a CLI convention, not this app's.
  - Audio-only rows dim the resolution cell to `--text-tertiary`, so scanning for a video
    stream skips them without needing colour alone to say so.

## Anti-Patterns (inherited from the global doc — do not violate)

- Do not apply glass/blur to content, lists, tables, or forms.
- Do not use tint decoratively outside primary-action/selected-state.
- Do not mix multiple visual languages on one screen.
- Do not let blur/translucency reduce text contrast below 4.5:1.
