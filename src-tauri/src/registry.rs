use std::collections::{HashMap, VecDeque};
use std::process::Child;
use std::sync::{Arc, Mutex};

/// A download that hasn't been spawned yet — waiting for a concurrency slot.
/// Holds everything `spawn_next` needs to actually start the process later,
/// so nothing has to be re-resolved (URL, already-templated output path,
/// raw parameters, the options-toggle mode still to apply, and the
/// once-resolved ffmpeg path) at spawn time.
pub struct PendingJob {
    pub job_id: String,
    pub url: String,
    pub download_to: String,
    /// Already mode-applied (see `ytdlp::args::apply_mode`) — the
    /// options-toggle strategy is the same for every URL in a batch, so
    /// it's resolved once up front rather than per pending job.
    pub parameters: Vec<String>,
    pub ffmpeg_path: Option<String>,
}

/// Shared job registry. Cheaply `Clone`-able (an `Arc` clone) so a copy can
/// be moved into the background thread that reads a job's stdout, while the
/// original stays managed as Tauri app state for `cancel_job`/`cancel_all`
/// to reach. Tracks both currently-running children and jobs still waiting
/// on a concurrency slot.
#[derive(Clone)]
pub struct JobRegistry {
    children: Arc<Mutex<HashMap<String, Child>>>,
    pending: Arc<Mutex<VecDeque<PendingJob>>>,
    fill_lock: Arc<Mutex<()>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Mutex::new(VecDeque::new())),
            fill_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Serializes the whole check-count → pop → spawn → insert sequence in
    /// `fill_slots`. `running_count()` only means anything if no other
    /// thread can spawn between the check and the matching `insert`, and
    /// there is a real window there: spawning a process takes long enough
    /// that two jobs finishing at the same moment would both observe a free
    /// slot and both start work, running more downloads than the configured
    /// cap allows.
    ///
    /// Poisoning is recovered from rather than propagated: this guards a
    /// `()` with no invariant to corrupt, so a panic elsewhere shouldn't
    /// wedge the queue permanently.
    pub fn lock_fill(&self) -> std::sync::MutexGuard<'_, ()> {
        self.fill_lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, job_id: String, child: Child) {
        self.children.lock().unwrap().insert(job_id, child);
    }

    pub fn remove(&self, job_id: &str) -> Option<Child> {
        self.children.lock().unwrap().remove(job_id)
    }

    pub fn running_count(&self) -> usize {
        self.children.lock().unwrap().len()
    }

    pub fn push_pending(&self, job: PendingJob) {
        self.pending.lock().unwrap().push_back(job);
    }

    pub fn pop_pending(&self) -> Option<PendingJob> {
        self.pending.lock().unwrap().pop_front()
    }

    /// Removes a not-yet-spawned job from the queue (Cancel on a queued
    /// row). Returns true if it was actually queued.
    pub fn remove_pending(&self, job_id: &str) -> bool {
        let mut queue = self.pending.lock().unwrap();
        let before = queue.len();
        queue.retain(|j| j.job_id != job_id);
        queue.len() != before
    }

    /// Empties the queue and returns everything that was in it, so the
    /// caller can emit a terminal "cancelled" event per job.
    pub fn drain_pending(&self) -> Vec<PendingJob> {
        self.pending.lock().unwrap().drain(..).collect()
    }

    /// Kills every currently-running child and clears the registry,
    /// returning the ids that were running so the caller can emit terminal
    /// events for them.
    pub fn kill_all_running(&self) -> Vec<String> {
        let mut children = self.children.lock().unwrap();
        let ids: Vec<String> = children.keys().cloned().collect();
        for (_, mut child) in children.drain() {
            let _ = child.kill();
            // `wait` is required, not optional: dropping a `Child` does not
            // reap it, so a killed-but-unwaited process lingers as a zombie
            // (`<defunct>`) for the lifetime of the app. `cancel_job` already
            // does this for the single-job path.
            let _ = child.wait();
        }
        ids
    }
}
