import type { FormatEntry } from "../types";

/// Content layer per spec/ui.md: opaque, never glass — this is a dense data
/// table and translucency would cost the legibility it depends on.
///
/// Styled as a native macOS list view rather than terminal output. The
/// mockup's dark table came from a screenshot of `yt-dlp -F` in a terminal,
/// which was provenance rather than design intent; a black panel inside a
/// light window reads as an embedded console and breaks the "one system"
/// rule the spec asks for.

function formatSize(bytes: number | null): string {
  if (bytes == null) return "—";
  const mib = bytes / (1024 * 1024);
  if (mib >= 1024) return `${(mib / 1024).toFixed(2)} GB`;
  return `${mib.toFixed(1)} MB`;
}

function formatTbr(tbr: number | null): string {
  return tbr == null ? "—" : `${Math.round(tbr)}k`;
}

function formatFps(fps: number | null): string {
  return fps == null || fps === 0 ? "—" : String(Math.round(fps));
}

/// Codec strings run long (`avc1.4d401e`, `av01.0.09M.08`, `mp4a.40.2`) and
/// only the family is worth scanning; the full value stays available on
/// hover rather than being truncated with an ellipsis mid-column.
function shortCodec(codec: string): string {
  if (codec === "none") return "—";
  const family = codec.split(".")[0];
  return family || codec;
}

/// Fixed leading columns keep the numbers aligned; the two codec columns
/// share the remaining width so nothing overflows the modal at the default
/// window size (an earlier layout forced a horizontal scrollbar that hid
/// ACODEC entirely).
///
/// ID takes the flexible column and everything else is fixed. Site format
/// ids are not all short like YouTube's `137` — Instagram emits
/// `dash-1796271141547558v`, which wrapped onto two lines at the original
/// 4rem and knocked the rest of the row out of vertical alignment. It's the
/// one column whose content genuinely varies, so it should absorb the spare
/// width; the codec columns held it before and never needed more than four
/// characters (`vp09`, `mp4a`, `—`).
const COLUMNS =
  "grid grid-cols-[minmax(0,1fr)_2.75rem_5rem_2.5rem_4.5rem_4rem_4rem_4.5rem_4.5rem] gap-x-2 items-center";

export function FormatTable({
  formats,
  selectedFormatId,
  onSelect,
}: {
  formats: FormatEntry[];
  selectedFormatId: string | null;
  onSelect: (format: FormatEntry) => void;
}) {
  return (
    <div className="flex-1 min-h-0 flex flex-col rounded-md border border-[var(--border)] bg-[var(--surface)] overflow-hidden">
      {/* The rows are the *only* scroller in the modal body: this fills the
          height the sheet's flex chain gives it (sheet max-h → body flex-1 →
          this) instead of taking a fixed viewport cap of its own. The cap was
          there because relying on an ancestor had clipped rows before, but at
          typical window heights it exceeded the space the body actually had,
          so table and body both scrolled and the command block sat outside
          the one the wheel reached. Every link in that chain needs
          `min-h-0`, or a flex item refuses to shrink below its content and
          the clip comes back.

          The header scrolls *inside* this box as a sticky row rather than
          sitting above it: the scrollbar takes real width from whatever it is
          attached to, so a header outside the scroller would end up 8px wider
          than the rows and every column would sit off by that much. */}
      <div className="flex-1 min-h-0 overflow-y-auto scroll-area">
        <div
          className={`${COLUMNS} sticky top-0 z-10 px-3 py-1.5 text-[11px] font-medium text-[var(--text-tertiary)] bg-[var(--surface-sunken)] border-b border-[var(--border)]`}
        >
          <span>ID</span>
          <span>Ext</span>
          <span>Resolution</span>
          <span className="text-right">FPS</span>
          <span className="text-right">Size</span>
          <span className="text-right">Bitrate</span>
          <span>Proto</span>
          <span>Video</span>
          <span>Audio</span>
        </div>

        <div className="divide-y divide-[var(--border)]">
          {formats.map((f) => {
            const selected = f.formatId === selectedFormatId;
            const audioOnly = f.vcodec === "none";
            return (
              <button
                key={f.formatId}
                onClick={() => onSelect(f)}
                aria-pressed={selected}
                className={`${COLUMNS} w-full text-left px-3 py-1.5 text-[12px] transition-colors ${
                  selected
                    ? "bg-[var(--accent)] text-[var(--text-on-accent)]"
                    : "hover:bg-[var(--hover)] text-[var(--text-primary)]"
                }`}
              >
                {/* Plain trailing ellipsis. An earlier attempt truncated from
                    the left, on the theory that ids sharing a prefix need
                    their tail to stay distinguishable — but with the column
                    now wide enough that case is rare, and RTL rendered badly
                    in practice: a leading `…` *and* a clipped final glyph. */}
                <span
                  title={f.formatId}
                  className={`font-mono tabular-nums truncate ${
                    selected ? "" : "text-[var(--text-primary)]"
                  }`}
                >
                  {f.formatId}
                </span>
                <span className={selected ? "" : "text-[var(--text-secondary)]"}>{f.ext}</span>
                <span className={audioOnly && !selected ? "text-[var(--text-tertiary)]" : ""}>
                  {f.resolution}
                </span>
                <span className="text-right tabular-nums">{formatFps(f.fps)}</span>
                <span className="text-right tabular-nums">{formatSize(f.filesize)}</span>
                <span className="text-right tabular-nums">{formatTbr(f.tbr)}</span>
                {/* `m3u8_native` overruns this column; clip it rather than
                    let it push the codec columns around. */}
                <span
                  title={f.proto}
                  className={`truncate ${selected ? "" : "text-[var(--text-secondary)]"}`}
                >
                  {f.proto}
                </span>
                <span className="truncate" title={f.vcodec}>
                  {shortCodec(f.vcodec)}
                </span>
                <span className="truncate" title={f.acodec}>
                  {shortCodec(f.acodec)}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
