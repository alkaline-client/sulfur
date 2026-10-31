//! A running game process with captured output.

use std::{
    collections::VecDeque,
    process::Stdio,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::{Notify, watch},
};

use crate::{McError, Result, launch::LaunchCommand};

const LOG_CAPACITY: usize = 10_000;

/// Bounded log buffer addressed by monotonically increasing line numbers.
#[derive(Default)]
struct LogBuffer {
    lines: VecDeque<String>,
    /// Line number of `lines[0]`.
    first: u64,
}

pub struct GameProcess {
    pid: Option<u32>,
    logs: Arc<Mutex<LogBuffer>>,
    /// `None` while running, then `Some(exit code)` (`None` code if killed by a signal).
    exit: watch::Receiver<Option<Option<i32>>>,
    kill: Arc<Notify>,
}

impl GameProcess {
    /// Starts the game. Must be called inside a Tokio runtime.
    pub fn spawn(cmd: &LaunchCommand) -> Result<Self> {
        let mut command = Command::new(&cmd.program);
        command
            .args(&cmd.args)
            .envs(cmd.env.iter().map(|(k, v)| (k, v)))
            .current_dir(&cmd.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        std::fs::create_dir_all(&cmd.working_dir).map_err(|e| McError::io(&cmd.working_dir, e))?;
        let mut child = command.spawn().map_err(McError::Spawn)?;

        let logs = Arc::new(Mutex::new(LogBuffer::default()));
        let readers = [
            child.stdout.take().map(|s| tokio::spawn(pump(s, logs.clone()))),
            child.stderr.take().map(|s| tokio::spawn(pump(s, logs.clone()))),
        ];
        let (exit_tx, exit) = watch::channel(None);
        let kill = Arc::new(Notify::new());
        let pid = child.id();
        let kill_rx = kill.clone();
        tokio::spawn(async move {
            let status = tokio::select! {
                status = child.wait() => status,
                _ = kill_rx.notified() => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };
            // Let readers drain the last lines before reporting the exit.
            for reader in readers.into_iter().flatten() {
                let _ = reader.await;
            }
            let _ = exit_tx.send(Some(status.ok().and_then(|s| s.code())));
        });
        tracing::info!(pid, program = %cmd.program.display(), "game started");
        Ok(Self { pid, logs, exit, kill })
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn is_running(&self) -> bool {
        self.exit.borrow().is_none()
    }

    /// Exit code once the process has ended.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit.borrow().flatten()
    }

    pub fn kill(&self) {
        self.kill.notify_one();
    }

    /// Waits for the process to end and returns its exit code.
    pub async fn wait(&self) -> Option<i32> {
        let mut exit = self.exit.clone();
        exit.wait_for(Option::is_some).await.ok().and_then(|e| *e).flatten()
    }

    /// Log lines from line number `from` on, and the number to continue from.
    pub fn logs_since(&self, from: u64) -> (u64, Vec<String>) {
        let logs = self.logs.lock().unwrap();
        let start = from.max(logs.first);
        let skip = (start - logs.first) as usize;
        let lines: Vec<String> = logs.lines.iter().skip(skip).cloned().collect();
        (start + lines.len() as u64, lines)
    }
}

/// Reads output until the process closes it. Lines are read as bytes: Java
/// writes in the system's code page (GBK on Chinese Windows), and stopping at
/// the first byte that is not UTF-8 would leave the pipe full and the game
/// blocked on its next log line.
async fn pump(stream: impl AsyncRead + Unpin, logs: Arc<Mutex<LogBuffer>>) {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\n', '\r']).to_owned();
        let mut logs = logs.lock().unwrap();
        if logs.lines.len() == LOG_CAPACITY {
            logs.lines.pop_front();
            logs.first += 1;
        }
        logs.lines.push_back(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_reading_past_bytes_that_are_not_utf8() {
        let logs = Arc::new(Mutex::new(LogBuffer::default()));
        // "游戏" in GBK, then more output.
        let output: &[u8] = b"first\r\n\xd3\xce\xcf\xb7 started\nlast";
        pump(output, logs.clone()).await;
        let logs = logs.lock().unwrap();
        assert_eq!(logs.lines.len(), 3);
        assert_eq!(logs.lines[0], "first");
        assert!(logs.lines[1].ends_with(" started"));
        assert_eq!(logs.lines[2], "last");
    }
}
