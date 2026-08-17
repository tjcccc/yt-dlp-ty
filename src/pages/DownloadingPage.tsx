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
    // pt-10 clears the traffic lights: this view has no sidebar, so with the
    // overlay title bar the heading would otherwise sit right beneath them.
    // The header doubles as the drag region, since there's no title bar to
    // grab the window by.
    <div className="h-full flex flex-col gap-4 px-6 pb-6 pt-10">
      <h1 data-tauri-drag-region className="text-[20px] font-semibold">
        Downloading
      </h1>

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
