import { DownloadRow } from "../components/DownloadRow";
import { cancelAll } from "../lib/tauri";
import { useStore } from "../state/store";

export function DownloadingPage({ onBack }: { onBack: () => void }) {
  // Select the stable `jobs` record itself, then derive the array in the
  // component body — a selector that returns a fresh array reference every
  // call (e.g. `useStore(s => Object.values(s.jobs))`) breaks React's
  // getSnapshot-caching check for useSyncExternalStore and blanks the page.
  const jobsRecord = useStore((s) => s.jobs);
  const jobs = Object.values(jobsRecord);

  return (
    // Same wrapper geometry as the other pages, so the title lands in the
    // same spot across views. The traffic-light inset and the drag region are
    // both provided by the shell in App.tsx now — the heading no longer has
    // to double as the grab handle, which only worked when clicked exactly on
    // its text.
    <div className="h-full flex flex-col gap-4 px-6 pt-1 pb-6 w-full max-w-3xl mx-auto">
      <h1 className="text-[20px] font-semibold">Downloading</h1>

      <div className="flex flex-col gap-2">
        {jobs.map((job) => (
          <DownloadRow key={job.jobId} job={job} />
        ))}
      </div>

      <div className="mt-auto flex justify-end gap-3">
        <button
          onClick={() => cancelAll()}
          className="text-[13px] px-3 py-1.5 rounded-md bg-[var(--danger-bg)] text-[var(--danger)] hover:bg-[var(--danger-bg-hover)]"
        >
          Cancel All
        </button>
        <button
          onClick={onBack}
          className="text-[13px] px-3 py-1.5 rounded-md border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--hover)]"
        >
          Back
        </button>
      </div>
    </div>
  );
}
