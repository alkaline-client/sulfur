//! Long-running operations exposed as objects the UI polls.
//!
//! Polling a snapshot at frame rate keeps the FFI surface simple (no callbacks
//! into the JVM) and lets the UI animate progress smoothly.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use sulfur_download::{CancellationToken, SpeedMeter};
use sulfur_minecraft::{install::InstallMonitor, process::GameProcess};

use crate::types::{InstallSnapshot, InstallStage, LogChunk, TaskKind};

/// Numbers tasks in the order they started.
static NEXT_TASK: AtomicU64 = AtomicU64::new(1);

#[derive(uniffi::Object)]
pub struct InstallTask {
    pub(crate) version_id: String,
    pub(crate) kind: TaskKind,
    /// Higher for tasks started later.
    order: u64,
    /// What the task is doing right now, beyond its stage (a build's last line).
    pub(crate) detail: Mutex<Option<String>>,
    pub(crate) monitor: Arc<InstallMonitor>,
    pub(crate) cancel: CancellationToken,
    /// `None` while running, then the outcome.
    pub(crate) outcome: Mutex<Option<Result<(), String>>>,
    speed: Mutex<SpeedMeter>,
}

impl InstallTask {
    pub(crate) fn new(version_id: String, kind: TaskKind) -> Arc<Self> {
        Arc::new(Self {
            version_id,
            kind,
            order: NEXT_TASK.fetch_add(1, Ordering::Relaxed),
            detail: Mutex::new(None),
            monitor: Arc::default(),
            cancel: CancellationToken::new(),
            outcome: Mutex::new(None),
            speed: Mutex::new(SpeedMeter::new(Duration::from_secs(2))),
        })
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.outcome.lock().unwrap().is_some()
    }

    pub(crate) fn order(&self) -> u64 {
        self.order
    }
}

#[uniffi::export]
impl InstallTask {
    pub fn version_id(&self) -> String {
        self.version_id.clone()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn snapshot(&self) -> InstallSnapshot {
        let progress = self.monitor.progress.snapshot();
        let (stage, error) = match &*self.outcome.lock().unwrap() {
            None => (
                match self.monitor.stage() {
                    sulfur_minecraft::install::InstallStage::Preparing => InstallStage::Preparing,
                    sulfur_minecraft::install::InstallStage::Downloading => InstallStage::Downloading,
                    sulfur_minecraft::install::InstallStage::Extracting => InstallStage::Extracting,
                    sulfur_minecraft::install::InstallStage::Loader => InstallStage::Loader,
                    sulfur_minecraft::install::InstallStage::Building => InstallStage::Building,
                    sulfur_minecraft::install::InstallStage::Done => InstallStage::Done,
                },
                None,
            ),
            Some(Ok(())) => (InstallStage::Done, None),
            Some(Err(_)) if self.cancel.is_cancelled() => (InstallStage::Cancelled, None),
            Some(Err(e)) => (InstallStage::Failed, Some(e.clone())),
        };
        let running = matches!(
            stage,
            InstallStage::Preparing
                | InstallStage::Downloading
                | InstallStage::Extracting
                | InstallStage::Loader
                | InstallStage::Building
        );
        let speed = self.speed.lock().unwrap().sample(progress.bytes_done);
        InstallSnapshot {
            version_id: self.version_id.clone(),
            kind: self.kind,
            stage,
            files_total: progress.files_total,
            files_done: progress.files_done,
            files_skipped: progress.files_skipped,
            files_reused: progress.files_reused,
            bytes_total: progress.bytes_total,
            bytes_done: progress.bytes_done,
            bytes_per_second: if running { speed } else { 0.0 },
            fraction: if stage == InstallStage::Done { 1.0 } else { progress.fraction() },
            detail: if running { self.detail.lock().unwrap().clone() } else { None },
            error,
        }
    }
}

/// A launched game.
#[derive(uniffi::Object)]
pub struct GameSession {
    pub(crate) version_id: String,
    pub(crate) process: GameProcess,
}

#[uniffi::export]
impl GameSession {
    pub fn version_id(&self) -> String {
        self.version_id.clone()
    }

    pub fn pid(&self) -> Option<u32> {
        self.process.pid()
    }

    pub fn is_running(&self) -> bool {
        self.process.is_running()
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.process.exit_code()
    }

    pub fn kill(&self) {
        self.process.kill();
    }

    /// Game output from line `from` on.
    pub fn logs_since(&self, from: u64) -> LogChunk {
        let (next, lines) = self.process.logs_since(from);
        LogChunk { next, lines }
    }
}
