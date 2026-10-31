//! Downloads every URL listed in a file (one per line) and reports throughput.
//!
//! ```text
//! cargo run --release -p sulfur-download --example fetch_list -- URLS.txt OUT_DIR [CONNECTIONS] [HTTP_CLIENTS]
//! ```

use std::{sync::Arc, time::Instant};

use sulfur_download::{CancellationToken, DownloadConfig, DownloadTask, Downloader, MirrorSet, Progress};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let urls = std::fs::read_to_string(&args[0])?;
    let out = std::path::PathBuf::from(&args[1]);
    let mut config = DownloadConfig::default();
    if let Some(n) = args.get(2) {
        config.max_connections = n.parse()?;
    }
    if let Some(n) = args.get(3) {
        config.http_clients = n.parse()?;
    }
    let tasks: Vec<_> = urls
        .lines()
        .filter(|l| !l.is_empty())
        .map(|url| DownloadTask::new(url, out.join(url.rsplit('/').next().unwrap())))
        .collect();
    let count = tasks.len();
    let dl = Downloader::new(config.clone(), MirrorSet::default())?;
    let progress = Arc::new(Progress::default());
    let started = Instant::now();
    let report = dl.run(tasks, progress.clone(), CancellationToken::new()).await;
    let secs = started.elapsed().as_secs_f64();
    let s = progress.snapshot();
    println!(
        "{count} files, {} failed, {:.1} MiB in {secs:.2}s: {:.1} files/s, {:.2} MiB/s (connections {}, clients {})",
        report.failed.len(),
        s.bytes_done as f64 / 1048576.0,
        count as f64 / secs,
        s.bytes_done as f64 / 1048576.0 / secs,
        config.max_connections,
        config.http_clients,
    );
    Ok(())
}
