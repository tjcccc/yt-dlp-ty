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
/// window size (the previous layout forced a horizontal scrollbar that hid
/// ACODEC entirely).
const COLUMNS =
  "grid grid-cols-[4rem_3.5rem_6.5rem_3rem_5.5rem_4rem_4.5rem_minmax(0,1fr)_minmax(0,1fr)] gap-x-3 items-center";

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
    <div className="rounded-md border border-[var(--border)] bg-[var(--surface)] overflow-hidden">
      <div
        className={`${COLUMNS} px-3 py-1.5 text-[11px] font-medium text-[var(--text-tertiary)] bg-[var(--surface-sunken)] border-b border-[var(--border)]`}
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
              <span className={`font-mono tabular-nums ${selected ? "" : "text-[var(--text-primary)]"}`}>
                {f.formatId}
              </span>
              <span className={selected ? "" : "text-[var(--text-secondary)]"}>{f.ext}</span>
              <span className={audioOnly && !selected ? "text-[var(--text-tertiary)]" : ""}>
                {f.resolution}
              </span>
              <span className="text-right tabular-nums">{formatFps(f.fps)}</span>
              <span className="text-right tabular-nums">{formatSize(f.filesize)}</span>
              <span className="text-right tabular-nums">{formatTbr(f.tbr)}</span>
              <span className={selected ? "" : "text-[var(--text-secondary)]"}>{f.proto}</span>
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
  );
}
