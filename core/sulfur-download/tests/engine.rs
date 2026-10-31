//! End-to-end tests against a tiny local HTTP/1.1 server with Range support.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use sha2::{Digest, Sha256};
use sulfur_download::{CancellationToken, Checksum, DownloadConfig, DownloadTask, Downloader, MirrorSet, Progress};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const BIG: usize = 12 * 1024 * 1024;

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i.wrapping_mul(31) ^ (i >> 12)) as u8).collect()
}

struct Server {
    base: String,
    range_requests: Arc<AtomicUsize>,
}

async fn serve() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let big = Arc::new(body(BIG));
    let range_requests = Arc::new(AtomicUsize::new(0));
    let counter = range_requests.clone();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(handle(stream, big.clone(), counter.clone()));
        }
    });
    Server { base, range_requests }
}

async fn handle(mut stream: TcpStream, big: Arc<Vec<u8>>, ranges: Arc<AtomicUsize>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let range = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").map(str::to_owned));

    let (data, slow): (Vec<u8>, bool) = match path.as_str() {
        "/big" => (big.to_vec(), true),
        p if p.starts_with("/small/") => (body(p[7..].parse().unwrap()), false),
        _ => {
            let _ = stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await;
            return;
        }
    };
    let total = data.len();
    let (status, start, end) = match range {
        Some(r) => {
            ranges.fetch_add(1, Ordering::Relaxed);
            let (a, b) = r.trim().split_once('-').unwrap();
            let a: usize = a.parse().unwrap();
            let b: usize = if b.is_empty() { total - 1 } else { b.parse().unwrap() };
            ("206 Partial Content", a, b.min(total - 1))
        }
        None => ("200 OK", 0, total - 1),
    };
    let mut header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n",
        end + 1 - start
    );
    if status.starts_with("206") {
        header += &format!("Content-Range: bytes {start}-{end}/{total}\r\n");
    }
    header += "\r\n";
    if stream.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    for chunk in data[start..=end].chunks(64 * 1024) {
        if stream.write_all(chunk).await.is_err() {
            return; // client dropped a stolen range
        }
        if slow {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}

fn sha256(data: &[u8]) -> Checksum {
    Checksum::Sha256(Sha256::digest(data).into())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sulfur-dl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn config() -> DownloadConfig {
    DownloadConfig {
        max_connections: 16,
        segment_threshold: 1024 * 1024,
        min_segment_size: 256 * 1024,
        max_segments_per_file: 8,
        max_retries: 2,
        ..DownloadConfig::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn segmented_download_is_byte_exact() {
    let server = serve().await;
    let dir = temp_dir("big");
    let dest = dir.join("big.bin");
    let dl = Downloader::new(config(), MirrorSet::default()).unwrap();
    let progress = Arc::new(Progress::default());
    let task = DownloadTask::new(format!("{}/big", server.base), &dest)
        .size(Some(BIG as u64))
        .checksum(Some(sha256(&body(BIG))));

    let report = dl.run(vec![task], progress.clone(), CancellationToken::new()).await;

    assert!(report.is_success(), "{:?}", report.failed);
    assert_eq!(std::fs::read(&dest).unwrap(), body(BIG));
    assert!(server.range_requests.load(Ordering::Relaxed) > 1, "file was not split");
    let snap = progress.snapshot();
    assert_eq!((snap.bytes_done, snap.bytes_total), (BIG as u64, BIG as u64));
    // No stray part files.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_small_files_with_fallback_and_skip() {
    let server = serve().await;
    let dir = temp_dir("small");
    let dl = Downloader::new(config(), MirrorSet::default()).unwrap();
    let tasks: Vec<_> = (1..=200)
        .map(|i| {
            let len = i * 97;
            // Every tenth file's primary URL is missing; the fallback has it.
            let primary = if i % 10 == 0 { format!("{}/missing", server.base) } else { format!("{}/small/{len}", server.base) };
            DownloadTask::new(primary, dir.join(format!("f{i}")))
                .fallback(format!("{}/small/{len}", server.base))
                .size(Some(len as u64))
                .checksum(Some(sha256(&body(len))))
        })
        .collect();

    let report = dl.run(tasks.clone(), Arc::new(Progress::default()), CancellationToken::new()).await;
    assert!(report.is_success(), "{:?}", report.failed);
    assert_eq!(report.downloaded, 200);
    assert_eq!(std::fs::read(dir.join("f150")).unwrap(), body(150 * 97));

    // Corrupt one file: it must be re-downloaded, the rest skipped.
    std::fs::write(dir.join("f7"), vec![0u8; 7 * 97]).unwrap();
    let report = dl.run(tasks.clone(), Arc::new(Progress::default()), CancellationToken::new()).await;
    assert_eq!((report.downloaded, report.skipped), (1, 199));

    // A second directory reuses the verified files instead of downloading.
    let other = temp_dir("small-reuse");
    std::fs::write(dir.join("f9"), b"corrupt").unwrap();
    let reuse: Vec<_> = tasks
        .into_iter()
        .map(|t| {
            let name = t.dest.file_name().unwrap().to_owned();
            DownloadTask { dest: other.join(&name), ..t }.reuse_from(dir.join(name))
        })
        .collect();
    let report = dl.run(reuse, Arc::new(Progress::default()), CancellationToken::new()).await;
    assert_eq!((report.downloaded, report.reused), (1, 199));
    assert_eq!(std::fs::read(other.join("f9")).unwrap(), body(9 * 97));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checksum_mismatch_fails_without_leaving_files() {
    let server = serve().await;
    let dir = temp_dir("bad");
    let dl = Downloader::new(config(), MirrorSet::default()).unwrap();
    let task = DownloadTask::new(format!("{}/small/1000", server.base), dir.join("x"))
        .checksum(Some(sha256(b"something else")));
    let report = dl.run(vec![task], Arc::new(Progress::default()), CancellationToken::new()).await;
    assert_eq!(report.failed.len(), 1);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_stops_promptly() {
    let server = serve().await;
    let dir = temp_dir("cancel");
    let dl = Downloader::new(config(), MirrorSet::default()).unwrap();
    let cancel = CancellationToken::new();
    let task = DownloadTask::new(format!("{}/big", server.base), dir.join("big"));
    let progress = Arc::new(Progress::default());
    let run = tokio::spawn({
        let (cancel, progress) = (cancel.clone(), progress.clone());
        async move { dl.run(vec![task], progress, cancel).await }
    });
    // Cancel mid-transfer: as soon as data flows (a fixed delay can outlast the whole download).
    while progress.snapshot().bytes_done == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    cancel.cancel();
    let report = tokio::time::timeout(Duration::from_secs(5), run).await.unwrap().unwrap();
    assert_eq!(report.failed.len(), 1);
}
