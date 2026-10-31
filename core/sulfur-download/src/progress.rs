use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
    time::{Duration, Instant},
};

/// Lock-free counters shared between the engine and observers (e.g. the UI).
///
/// Totals grow as batches are added, so one `Progress` can track a multi-step
/// operation such as "metadata, then libraries and assets".
#[derive(Debug, Default)]
pub struct Progress {
    files_total: AtomicU64,
    files_done: AtomicU64,
    files_skipped: AtomicU64,
    files_reused: AtomicU64,
    files_failed: AtomicU64,
    bytes_total: AtomicU64,
    bytes_done: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub files_total: u64,
    /// Finished files, including skipped ones.
    pub files_done: u64,
    /// Files that were already present and valid.
    pub files_skipped: u64,
    /// Files linked or copied from another directory instead of downloaded.
    pub files_reused: u64,
    pub files_failed: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
}

impl ProgressSnapshot {
    /// Completion in `0.0..=1.0`, by bytes when sizes are known, else by files.
    pub fn fraction(&self) -> f64 {
        if self.bytes_total > 0 {
            (self.bytes_done as f64 / self.bytes_total as f64).min(1.0)
        } else if self.files_total > 0 {
            self.files_done as f64 / self.files_total as f64
        } else {
            0.0
        }
    }
}

impl Progress {
    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files_total: self.files_total.load(Relaxed),
            files_done: self.files_done.load(Relaxed),
            files_skipped: self.files_skipped.load(Relaxed),
            files_reused: self.files_reused.load(Relaxed),
            files_failed: self.files_failed.load(Relaxed),
            bytes_total: self.bytes_total.load(Relaxed),
            bytes_done: self.bytes_done.load(Relaxed),
        }
    }

    pub(crate) fn add_files(&self, n: u64) {
        self.files_total.fetch_add(n, Relaxed);
    }
    pub(crate) fn add_total_bytes(&self, n: u64) {
        self.bytes_total.fetch_add(n, Relaxed);
    }
    pub(crate) fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Relaxed);
    }
    pub(crate) fn sub_bytes(&self, n: u64) {
        self.bytes_done.fetch_sub(n, Relaxed);
    }
    pub(crate) fn file_done(&self) {
        self.files_done.fetch_add(1, Relaxed);
    }
    pub(crate) fn file_skipped(&self, size: u64) {
        self.files_skipped.fetch_add(1, Relaxed);
        self.files_done.fetch_add(1, Relaxed);
        self.bytes_done.fetch_add(size, Relaxed);
    }
    pub(crate) fn file_reused(&self, size: u64) {
        self.files_reused.fetch_add(1, Relaxed);
        self.files_done.fetch_add(1, Relaxed);
        self.bytes_done.fetch_add(size, Relaxed);
    }
    pub(crate) fn file_failed(&self) {
        self.files_failed.fetch_add(1, Relaxed);
    }
}

/// Moving-window throughput estimate. Feed it `bytes_done` periodically.
#[derive(Debug)]
pub struct SpeedMeter {
    window: Duration,
    samples: VecDeque<(Instant, u64)>,
}

impl SpeedMeter {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            samples: VecDeque::new(),
        }
    }

    /// Records a sample and returns bytes per second over the window.
    pub fn sample(&mut self, bytes_done: u64) -> f64 {
        let now = Instant::now();
        self.samples.push_back((now, bytes_done));
        while let Some(&(t, _)) = self.samples.front() {
            if now.duration_since(t) > self.window && self.samples.len() > 2 {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        let (t0, b0) = self.samples[0];
        let dt = now.duration_since(t0).as_secs_f64();
        if dt < 0.05 {
            return 0.0;
        }
        bytes_done.saturating_sub(b0) as f64 / dt
    }
}

impl Default for SpeedMeter {
    fn default() -> Self {
        Self::new(Duration::from_secs(3))
    }
}
