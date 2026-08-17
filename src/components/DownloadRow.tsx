import type { JobState } from "../state/jobsSlice";
import { cancelJob } from "../lib/tauri";

function formatBytes(bytes: number | null): string {
  if (bytes == null) return "";
  const mib = bytes / (1024 * 1024);
  return `${mib.toFixed(1)} MiB`;
}

const PHASE_LABEL: Record<JobState["phase"], string> = {
  queued: "Queued",
  downloading: "",
  merging: "Merging…",
  completed: "Completed",
  error: "Error",
  cancelled: "Cancelled",
};

const PHASE_FILL: Record<JobState["phase"], string> = {
  queued: "bg-[var(--progress-fill-neutral)]",
  downloading: "bg-[var(--progress-fill)]",
  merging: "bg-[var(--progress-fill-strong)]",
  completed: "bg-[var(--progress-fill-strong)]",
  error: "bg-[var(--danger-bg-hover)]",
  cancelled: "bg-[var(--progress-fill-neutral)]",
};

export function DownloadRow({ job }: { job: JobState }) {
  // overallPercent is the remapped, monotonically non-decreasing value from
  // the backend's PassTracker — a raw downloaded/total ratio would visibly
  // reset partway through any video+audio merge (see DEVLOG two-pass entry).
  const pct = job.phase === "completed" ? 100 : Math.round(job.overallPercent ?? 0);

  const isActive = job.phase === "queued" || job.phase === "downloading" || job.phase === "merging";
  const label = PHASE_LABEL[job.phase] || `${pct}%`;

  return (
    <div className="rounded-md border border-[var(--border)] bg-[var(--surface)] overflow-hidden">
      <div className="relative h-11">
        <div
          className={`absolute inset-y-0 left-0 transition-[width] duration-300 ${PHASE_FILL[job.phase]}`}
          style={{ width: `${pct}%` }}
        />
        <div className="relative h-full flex items-center justify-between px-3 gap-3">
          <span className="text-[13px] truncate">{job.url}</span>
          <div className="flex items-center gap-3 shrink-0">
            {job.phase === "downloading" && (
              <span className="text-[12px] text-[var(--text-secondary)]">{formatBytes(job.downloadedBytes)}</span>
            )}
            <span className="text-[13px] text-[var(--text-secondary)] tabular-nums">{label}</span>
            {isActive && (
              <button
                onClick={() => cancelJob(job.jobId)}
                className="text-[12px] px-2 py-1 rounded-sm bg-[var(--danger-bg)] text-[var(--danger)] hover:bg-[var(--danger-bg-hover)]"
              >
                Cancel
              </button>
            )}
          </div>
        </div>
      </div>
      {job.phase === "error" && job.errorMessage && (
        <pre className="text-[11px] text-[var(--danger)] bg-[var(--danger-bg)] px-3 py-2 whitespace-pre-wrap break-words max-h-32 overflow-y-auto">
          {job.errorMessage}
        </pre>
      )}
    </div>
  );
}
