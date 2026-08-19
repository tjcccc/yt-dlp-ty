import { useState } from "react";
import { CommandDetails } from "../components/CommandDetails";
import { FormatTable } from "../components/FormatTable";
import { isVideoOnly, type ChosenFormat, type FormatEntry, type VideoFormats } from "../types";

/// Overlay-layer glass sheet per spec/ui.md, over a dimmed backdrop.
///
/// One row per input URL (a URL is the unit a download job is spawned for),
/// each showing its picked format id or "not selected"; selecting a row
/// reveals that video's full format table below. Download stays disabled
/// until every row has a selection — a partial batch would silently fall
/// back to the default format for the rest, which is the opposite of what
/// this flow promises.
export function ChooseFormatModal({
  videos,
  loading,
  onCancel,
  onConfirm,
}: {
  videos: VideoFormats[];
  loading: boolean;
  onCancel: () => void;
  onConfirm: (chosen: Record<string, ChosenFormat>) => void;
}) {
  const [chosen, setChosen] = useState<Record<string, ChosenFormat>>({});
  const selectable = videos.filter((v) => !v.error);
  const [activeUrl, setActiveUrl] = useState<string | null>(selectable[0]?.url ?? null);

  // `activeUrl`'s initial value is captured on the modal's first render,
  // which happens while the probe is still running and `videos` is empty —
  // so it seeds to null and never recovers on its own. Falling back to the
  // first selectable video keeps the table populated as soon as results
  // land, instead of showing "No video selected" next to a list that
  // plainly has one.
  const active = videos.find((v) => v.url === activeUrl) ?? selectable[0] ?? null;
  const allChosen = selectable.length > 0 && selectable.every((v) => chosen[v.url]);
  const failed = videos.filter((v) => v.error);

  const pick = (url: string, format: FormatEntry) => {
    setChosen((prev) => ({
      ...prev,
      [url]: { formatId: format.formatId, videoOnly: isVideoOnly(format) },
    }));
  };

  return (
    <div className="fixed inset-0 z-50 bg-[var(--scrim)] flex items-center justify-center p-8">
      {/* The scrim covers the shell's title-bar drag strip, so the band people
          actually reach for stops working the moment the sheet opens — the
          window becomes unmovable while choosing a format. This restores it at
          the same height as the strip in App.tsx. It sits under the sheet in
          paint order, and where the two overlap it only covers the sheet's own
          title row, which is a drag region anyway. */}
      <div
        data-tauri-drag-region="deep"
        className="absolute top-0 left-0 right-0 h-9"
      />
      {/* max-h in viewport units, not `max-h-full`: a percentage max-height
          has to resolve against an ancestor, and getting that wrong leaves
          the sheet unbounded so the flex-1 body never becomes scrollable —
          the list just gets clipped with no way to reach the rest. */}
      <div className="relative w-full max-w-4xl max-h-[85vh] flex flex-col gap-4 rounded-lg bg-[var(--glass-overlay)] backdrop-blur-2xl shadow-2xl border border-[var(--border)] p-6">
        {/* Second grab area, for when the sheet itself covers the strip above
            (short window, tall sheet). Its title row is the natural handle —
            same as a real macOS sheet. */}
        <div data-tauri-drag-region="deep" className="shrink-0">
          <h2 className="text-[20px] font-semibold">Choose format first</h2>
        </div>

        {loading ? (
          <p className="text-[13px] text-[var(--text-secondary)] py-8 text-center">
            Fetching available formats…
          </p>
        ) : (
          <>
            <div className="rounded-md border border-[var(--border)] bg-[var(--surface)] overflow-y-auto scroll-area max-h-44 shrink-0">
              {videos.map((v) => {
                const pickedId = chosen[v.url]?.formatId;
                return (
                  <button
                    key={v.url}
                    // Failed rows stay selectable on purpose: selecting one is
                    // how its command and error output get shown below. They
                    // still can't have a format chosen — `selectable`, which
                    // gates the Download button, already excludes them.
                    onClick={() => setActiveUrl(v.url)}
                    className={`w-full flex items-center justify-between gap-4 px-3 py-2 text-left ${
                      // Compare against `active`, not `activeUrl` — while the
                      // latter is still null the fallback below shows the
                      // first video's table, and highlighting nothing would
                      // leave the list disagreeing with what's displayed.
                      v.url === active?.url ? "bg-[var(--selected)]" : "hover:bg-[var(--hover)]"
                    }`}
                  >
                    <span className={`text-[13px] truncate ${v.error ? "text-[var(--text-tertiary)]" : ""}`}>
                      {v.title}
                    </span>
                    <span
                      className={`text-[12px] shrink-0 ${
                        // Monospace only for an actual format id, where digit
                        // alignment helps — "not selected" is prose and reads
                        // as terminal output in a mono face.
                        pickedId && !v.error ? "font-mono text-[var(--accent-fg)]" : ""
                      } ${v.error ? "text-[var(--danger)]" : pickedId ? "" : "text-[var(--text-tertiary)]"}`}
                    >
                      {v.error ? "failed" : (pickedId ?? "not selected")}
                    </span>
                  </button>
                );
              })}
            </div>

            {failed.length > 0 && (
              <p className="text-[12px] text-[var(--danger)] -mt-1">
                {failed.length} URL{failed.length > 1 ? "s" : ""} could not be read and will be
                skipped — select one to see why.
              </p>
            )}

            {/* No scrolling of its own — it hands its height to the format
                table, which is the single scroller in this region. Two nested
                scrollers here meant the wheel chained between them and the
                command block below sat outside the one under the cursor,
                reading as a table whose last row had been cut off. */}
            <div className="flex-1 min-h-0 flex flex-col gap-2">
              {!active ? (
                <p className="text-[13px] text-[var(--text-tertiary)]">No video selected.</p>
              ) : (
                <>
                  {active.error ? (
                    <p className="text-[13px] text-[var(--danger)]">
                      Could not read this URL.
                    </p>
                  ) : (
                    <FormatTable
                      formats={active.formats}
                      selectedFormatId={chosen[active.url]?.formatId ?? null}
                      onSelect={(format) => pick(active.url, format)}
                    />
                  )}
                  {/* Always available, not just on failure: seeing the exact
                      invocation is how you confirm the cookie/proxy flags
                      really reached the probe that produced this list. Stays
                      pinned under the table rather than scrolling with it. */}
                  <div className="shrink-0">
                    <CommandDetails command={active.command} log={active.error} />
                  </div>
                </>
              )}
            </div>

            {active && chosen[active.url]?.videoOnly && (
              <p className="text-[12px] text-[var(--text-secondary)]">
                Video-only format — the best available audio will be merged in automatically.
              </p>
            )}
          </>
        )}

        <div className="flex items-center justify-end gap-2 pt-1">
          <button
            onClick={onCancel}
            className="text-[13px] px-3 py-2 rounded-md border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--hover)]"
          >
            Cancel
          </button>
          <button
            onClick={() => onConfirm(chosen)}
            disabled={!allChosen}
            className="text-[14px] font-medium px-4 py-2 rounded-md bg-[var(--accent)] text-[var(--text-on-accent)] hover:bg-[var(--accent-hover)] disabled:opacity-40"
            title={allChosen ? undefined : "Choose a format for every video first"}
          >
            Download
          </button>
        </div>
      </div>
    </div>
  );
}
