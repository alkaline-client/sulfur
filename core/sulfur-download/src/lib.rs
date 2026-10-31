//! Sulfur's download engine.
//!
//! The engine is tuned for the two shapes of traffic a game launcher produces:
//!
//! * **Thousands of small files** (Minecraft assets and libraries). These are
//!   fetched as whole files with high concurrency over a small pool of HTTP
//!   clients, so connection setup is amortised and several TCP connections share
//!   the load instead of one multiplexed HTTP/2 connection.
//! * **A few large files** (client jars, runtimes, modpacks). These are split
//!   into byte ranges that are downloaded in parallel. Splitting is dynamic: a
//!   worker that finishes early steals the second half of the largest remaining
//!   range, so a slow connection never becomes the tail of the download.
//!
//! All requests draw from one global connection budget ([`DownloadConfig::max_connections`]),
//! every file is written to a `.part` file, verified against its checksum and
//! only then renamed into place.

mod checksum;
mod config;
mod engine;
mod error;
mod mirror;
mod progress;
mod segmented;
mod task;

pub use checksum::{Checksum, verify_file};
pub use config::DownloadConfig;
pub use engine::{BatchReport, Downloader};
pub use error::DownloadError;
pub use mirror::{MirrorRule, MirrorSet};
pub use progress::{Progress, ProgressSnapshot, SpeedMeter};
pub use task::DownloadTask;
pub use tokio_util::sync::CancellationToken;

/// Percent-encodes a URL query value or path segment (RFC 3986 unreserved characters stay).
pub fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
