import { useEffect, useState } from "react";
import { CopyButton } from "../components/CopyButton";
import { clearHistory, listHistory } from "../lib/tauri";
import type { HistoryEntry } from "../types";

function formatSize(bytes: number | null): string {
  if (bytes == null) return "—";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  if (mb >= 1) return `${mb.toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/// Local time, and the year only when it isn't the current one — a history
/// of mostly-recent downloads shouldn't repeat "2026" on every row.
function formatWhen(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleString(undefined, {
    year: sameYear ? undefined : "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(ms: number | null): string | null {
  if (ms == null || ms < 0) return null;
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const duration = formatDuration(entry.durationMs);
  // platform and template are the same kind of fact — where it came from and
  // which preset fetched it — so they read as one line rather than two cells.
  const provenance = [entry.platform, entry.templateName].filter(Boolean).join(" · ");

  return (
    <div className="rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 flex flex-col gap-1">
      <div className="flex items-baseline gap-3">
        <span className="text-[13px] font-medium truncate" title={entry.filename ?? undefined}>
          {entry.filename ?? "(filename unknown)"}
        </span>
        <span className="ml-auto shrink-0 text-[12px] tabular-nums text-[var(--text-secondary)]">
          {formatSize(entry.sizeBytes)}
        </span>
        <span
          className="shrink-0 text-[12px] tabular-nums text-[var(--text-tertiary)]"
          title={duration ? `Took ${duration}` : undefined}
        >
          {formatWhen(entry.finishedAt)}
          {duration ? ` · ${duration}` : ""}
        </span>
      </div>

      <div className="flex items-center gap-3">
        {provenance && (
          <span className="shrink-0 text-[12px] text-[var(--text-secondary)]">{provenance}</span>
        )}
        <span
          className="text-[11px] font-mono text-[var(--text-tertiary)] truncate select-text"
          title={entry.url}
        >
          {entry.url}
        </span>
        <CopyButton value={entry.url} label="copy link" className="ml-auto shrink-0" />
      </div>

      {entry.command && (
        <div className="flex items-center gap-3">
          {/* Labelled, so the two copy buttons in a row are told apart by
              what they sit next to rather than by position. */}
          <span className="shrink-0 text-[11px] text-[var(--text-tertiary)]">command</span>
          <span
            className="text-[11px] font-mono text-[var(--text-tertiary)] truncate select-text"
            title={entry.command}
          >
            {entry.command}
          </span>
          <CopyButton value={entry.command} label="copy command" className="ml-auto shrink-0" />
        </div>
      )}
    </div>
  );
}

/// Reads like the Downloading view — a plain list of rows — because it
/// answers the same kind of question, just after the fact.
export function HistoryPage() {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [confirmingClear, setConfirmingClear] = useState(false);

  const load = () => listHistory().then(setEntries);

  useEffect(() => {
    load();
  }, []);

  return (
    <div className="h-full flex flex-col gap-4 px-6 pt-1 pb-6 w-full max-w-3xl mx-auto">
      <h1 className="text-[20px] font-semibold">History</h1>

      {/* Scrolls on its own with the action bar pinned outside it — see
          DownloadingPage: page-level bottom padding collapses against the
          window edge once the content overflows a shared scroll container. */}
      <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-2">
        {entries === null ? (
          <p className="text-[13px] text-[var(--text-tertiary)]">Loading…</p>
        ) : entries.length === 0 ? (
          <p className="text-[13px] text-[var(--text-tertiary)]">
            No completed downloads yet.
          </p>
        ) : (
          entries.map((entry) => <HistoryRow key={entry.id} entry={entry} />)
        )}
      </div>

      {entries !== null && entries.length > 0 && (
        <div className="shrink-0 flex items-center justify-end gap-3">
          {confirmingClear ? (
            // Inline two-step confirm, matching TemplateRow: a native dialog
            // blocks the webview's event loop and is unreachable to the UI
            // automation this app is tested with.
            <>
              <span className="text-[12px] text-[var(--danger)]">
                Delete all {entries.length} history entries?
              </span>
              <button
                onClick={async () => {
                  setConfirmingClear(false);
                  await clearHistory();
                  load();
                }}
                className="text-[13px] px-3 py-1.5 rounded-md bg-[var(--danger)] text-[var(--text-on-accent)] hover:opacity-90"
              >
                Clear
              </button>
              <button
                onClick={() => setConfirmingClear(false)}
                className="text-[13px] px-3 py-1.5 rounded-md border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--hover)]"
              >
                Cancel
              </button>
            </>
          ) : (
            <button
              onClick={() => setConfirmingClear(true)}
              className="text-[13px] px-3 py-1.5 rounded-md border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--hover)]"
            >
              Clear history
            </button>
          )}
        </div>
      )}
    </div>
  );
}
