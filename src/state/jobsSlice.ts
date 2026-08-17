import type { JobPhase, JobProgressEvent } from "../types";

export interface JobState {
  jobId: string;
  url: string;
  phase: JobPhase;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speedBps: number | null;
  etaSeconds: number | null;
  overallPercent: number | null;
  errorMessage: string | null;
}

export interface JobsSlice {
  jobs: Record<string, JobState>;
  /// Progress events that arrived before their job was registered — see
  /// `updateJobProgress`. Keyed by jobId, holding only the latest event.
  pendingEvents: Record<string, JobProgressEvent>;
  addJob: (job: JobState) => void;
  updateJobProgress: (event: JobProgressEvent) => void;
}

type Set = (fn: (state: JobsSlice) => Partial<JobsSlice>) => void;

function applyEvent(job: JobState, event: JobProgressEvent): JobState {
  return {
    ...job,
    phase: event.phase,
    downloadedBytes: event.downloadedBytes,
    totalBytes: event.totalBytes,
    speedBps: event.speedBps,
    etaSeconds: event.etaSeconds,
    overallPercent: event.overallPercent,
    errorMessage: event.errorMessage,
  };
}

export const createJobsSlice = (set: Set): JobsSlice => ({
  jobs: {},
  pendingEvents: {},

  // A job's first events can arrive before the frontend knows the job
  // exists: the backend spawns up to `concurrency` children (emitting a
  // "downloading" event for each) *inside* the synchronous `start_downloads`
  // call, which only returns the job ids afterwards — so `addJob` always
  // runs second. Any buffered event is applied here so a running job never
  // sits on a stale "Queued" label.
  addJob: (job) =>
    set((state) => {
      const buffered = state.pendingEvents[job.jobId];
      const { [job.jobId]: _dropped, ...pendingEvents } = state.pendingEvents;
      return {
        jobs: { ...state.jobs, [job.jobId]: buffered ? applyEvent(job, buffered) : job },
        pendingEvents,
      };
    }),

  updateJobProgress: (event) =>
    set((state) => {
      const existing = state.jobs[event.jobId];
      if (!existing) {
        // Not registered yet — keep the latest event so `addJob` can apply
        // it, rather than dropping the job's real state on the floor.
        return { pendingEvents: { ...state.pendingEvents, [event.jobId]: event } };
      }
      return { jobs: { ...state.jobs, [event.jobId]: applyEvent(existing, event) } };
    }),
});
