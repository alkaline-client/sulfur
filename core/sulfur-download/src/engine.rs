use std::{
    collections::HashSet,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};

use bytes::Bytes;
use reqwest::{Client, Response, StatusCode, header};
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    Checksum, DownloadConfig, DownloadError, DownloadTask, MirrorSet, Progress, segmented, verify_file,
};

const WRITE_BUFFER: usize = 256 * 1024;

/// Cheaply clonable handle to the download engine.
#[derive(Clone)]
pub struct Downloader {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    clients: Vec<Client>,
    next_client: AtomicUsize,
    pub(crate) config: DownloadConfig,
    mirrors: MirrorSet,
    connections: Arc<Semaphore>,
}

/// Outcome of [`Downloader::run`].
#[derive(Debug, Default)]
pub struct BatchReport {
    pub downloaded: usize,
    pub skipped: usize,
    pub reused: usize,
    pub failed: Vec<(PathBuf, DownloadError)>,
}

impl BatchReport {
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// Converts the report into an error carrying the first failure, if any.
    pub fn into_result(mut self) -> Result<Self, DownloadError> {
        if self.failed.is_empty() {
            return Ok(self);
        }
        let (_, first) = self.failed.swap_remove(0);
        Err(first)
    }
}

enum Outcome {
    Downloaded,
    Skipped,
    Reused,
}

impl Downloader {
    pub fn new(config: DownloadConfig, mirrors: MirrorSet) -> Result<Self, DownloadError> {
        // reqwest is built without a bundled crypto provider; use ring process-wide.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let clients = (0..config.http_clients.max(1))
            .map(|_| build_client(&config))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inner: Arc::new(Inner {
                clients,
                next_client: AtomicUsize::new(0),
                connections: Arc::new(Semaphore::new(config.max_connections.max(1))),
                config,
                mirrors,
            }),
        })
    }

    pub fn config(&self) -> &DownloadConfig {
        &self.inner.config
    }

    /// One of the engine's HTTP clients (same proxy, TLS and user agent), for
    /// requests that are not downloads, such as signing in.
    pub fn http_client(&self) -> Client {
        self.inner.client().clone()
    }

    /// Fetches a small resource (metadata JSON and the like) into memory,
    /// with mirror fallback and retries.
    pub async fn fetch(&self, url: &str, cancel: &CancellationToken) -> Result<Bytes, DownloadError> {
        self.fetch_with_headers(url, &[], cancel).await
    }

    /// [`fetch`](Self::fetch) with extra request headers (API keys and the like).
    pub async fn fetch_with_headers(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        cancel: &CancellationToken,
    ) -> Result<Bytes, DownloadError> {
        let inner = &self.inner;
        let sources = inner.mirrors.sources(url);
        inner
            .retrying(&sources, cancel, move |src| async move {
                let _permit = inner.acquire(cancel).await?;
                let mut request = inner.client().get(&src);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }
                let resp = until_cancelled(cancel, request.send()).await??;
                let resp = check_status(resp)?;
                Ok(until_cancelled(cancel, resp.bytes()).await??)
            })
            .await
    }

    /// Downloads every task, sharing the global connection budget.
    ///
    /// Files that already exist and match their checksum are skipped. Failures
    /// do not abort the batch; they are collected in the report.
    pub async fn run(
        &self,
        tasks: Vec<DownloadTask>,
        progress: Arc<Progress>,
        cancel: CancellationToken,
    ) -> BatchReport {
        let mut seen = HashSet::new();
        let mut tasks: Vec<_> = tasks.into_iter().filter(|t| seen.insert(t.dest.clone())).collect();
        // Largest first: long transfers start early and small files fill the gaps.
        tasks.sort_by_key(|t| std::cmp::Reverse(t.size.unwrap_or(0)));
        progress.add_files(tasks.len() as u64);
        progress.add_total_bytes(tasks.iter().filter_map(|t| t.size).sum());

        // More file slots than connections, so hashing and disk I/O overlap with network.
        let slots = Arc::new(Semaphore::new(self.inner.config.max_connections.max(1) * 2));
        let mut set = JoinSet::new();
        let mut report = BatchReport::default();
        for task in tasks {
            let Ok(slot) = until_cancelled(&cancel, slots.clone().acquire_owned()).await else {
                report.failed.push((task.dest, DownloadError::Cancelled));
                continue;
            };
            let (this, task_progress, task_cancel) = (self.clone(), progress.clone(), cancel.clone());
            set.spawn(async move {
                let _slot = slot;
                let result = this.download_one(&task, &task_progress, &task_cancel).await;
                (task.dest, result)
            });
            while let Some(joined) = set.try_join_next() {
                record(&mut report, &progress, joined);
            }
        }
        while let Some(joined) = set.join_next().await {
            record(&mut report, &progress, joined);
        }
        report
    }

    async fn download_one(
        &self,
        task: &DownloadTask,
        progress: &Arc<Progress>,
        cancel: &CancellationToken,
    ) -> Result<Outcome, DownloadError> {
        if self.is_valid(&task.dest, task).await {
            progress.file_skipped(task.size.unwrap_or(0));
            return Ok(Outcome::Skipped);
        }
        if self.reuse_existing(task).await {
            progress.file_reused(task.size.unwrap_or(0));
            return Ok(Outcome::Reused);
        }
        let mut sources = self.inner.mirrors.sources(&task.url);
        for url in &task.fallbacks {
            sources.extend(self.inner.mirrors.sources(url));
        }
        if let Some(parent) = task.dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| DownloadError::io(parent, e))?;
        }
        let part = part_path(&task.dest);
        // Bytes this file has contributed to `progress`; rolled back when an attempt fails.
        let counted = Arc::new(AtomicU64::new(0));
        // Size learned from the server for tasks without a known size.
        let learned = AtomicU64::new(0);

        let (part_ref, counted_ref, learned_ref) = (&part, &counted, &learned);
        let result = self
            .inner
            .retrying(&sources, cancel, move |url| async move {
                progress.sub_bytes(counted_ref.swap(0, Relaxed));
                self.attempt(task, &url, part_ref, progress, counted_ref, learned_ref, cancel).await
            })
            .await;

        let result = match result {
            Ok(()) => tokio::fs::rename(&part, &task.dest)
                .await
                .map_err(|e| DownloadError::io(&task.dest, e)),
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => {
                progress.file_done();
                Ok(Outcome::Downloaded)
            }
            Err(e) => {
                progress.sub_bytes(counted.swap(0, Relaxed));
                let _ = tokio::fs::remove_file(&part).await;
                Err(e)
            }
        }
    }

    /// Whether `path` holds the file `task` describes.
    async fn is_valid(&self, path: &Path, task: &DownloadTask) -> bool {
        let Ok(meta) = tokio::fs::metadata(path).await else {
            return false;
        };
        if !meta.is_file() || task.size.is_some_and(|size| meta.len() != size) {
            return false;
        }
        match &task.checksum {
            Some(checksum) if self.inner.config.verify_existing => {
                let (path, checksum) = (path.to_owned(), checksum.clone());
                tokio::task::spawn_blocking(move || verify_file(&path, &checksum).unwrap_or(false))
                    .await
                    .unwrap_or(false)
            }
            _ => true,
        }
    }

    /// Hard-links (or copies, across volumes) a verified copy from `task.reuse`.
    async fn reuse_existing(&self, task: &DownloadTask) -> bool {
        // Only trust copies we can verify.
        if task.checksum.is_none() && task.size.is_none() {
            return false;
        }
        for source in &task.reuse {
            if !self.is_valid(source, task).await {
                continue;
            }
            if let Some(parent) = task.dest.parent()
                && tokio::fs::create_dir_all(parent).await.is_err()
            {
                return false;
            }
            let part = part_path(&task.dest);
            let linked = tokio::fs::hard_link(source, &part).await.is_ok()
                || tokio::fs::copy(source, &part).await.is_ok();
            if linked && tokio::fs::rename(&part, &task.dest).await.is_ok() {
                return true;
            }
            let _ = tokio::fs::remove_file(&part).await;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    async fn attempt(
        &self,
        task: &DownloadTask,
        url: &str,
        part: &Path,
        progress: &Arc<Progress>,
        counted: &Arc<AtomicU64>,
        learned: &AtomicU64,
        cancel: &CancellationToken,
    ) -> Result<(), DownloadError> {
        let inner = &self.inner;
        let cfg = &inner.config;
        let permit = inner.acquire(cancel).await?;

        // Only ask for ranges when the file could be large enough to split.
        let try_ranges =
            cfg.max_segments_per_file > 1 && task.size.is_none_or(|s| s >= cfg.segment_threshold);
        let mut request = inner.client().get(url);
        if try_ranges {
            request = request.header(header::RANGE, "bytes=0-");
        }
        let resp = check_status(until_cancelled(cancel, request.send()).await??)?;

        let ranged_total = if resp.status() == StatusCode::PARTIAL_CONTENT {
            match content_range(&resp) {
                Some((0, _, total)) => total,
                _ => return Err(DownloadError::BadRange(url.to_owned())),
            }
        } else {
            None
        };
        let length = ranged_total.or(resp.content_length());
        if let Some(actual) = length {
            match task.size {
                Some(expected) if expected != actual => {
                    return Err(DownloadError::SizeMismatch {
                        path: task.dest.clone(),
                        expected,
                        actual,
                    });
                }
                None if learned.swap(actual, Relaxed) == 0 => progress.add_total_bytes(actual),
                _ => {}
            }
        }

        match ranged_total {
            Some(total) if total >= cfg.segment_threshold && cfg.max_segments_per_file > 1 => {
                let job = segmented::Job {
                    // Helpers reuse the final URL so they skip any redirects.
                    url: resp.url().to_string(),
                    part: part.to_owned(),
                    total,
                    progress: progress.clone(),
                    counted: counted.clone(),
                    cancel: cancel.clone(),
                };
                segmented::download(self.inner.clone(), job, resp, permit).await?;
                verify_part(part, &task.dest, task.checksum.as_ref()).await
            }
            _ => {
                let _permit = permit;
                stream_whole(resp, task, part, length, progress, counted, cancel).await
            }
        }
    }
}

impl Inner {
    fn client(&self) -> &Client {
        let i = self.next_client.fetch_add(1, Relaxed);
        &self.clients[i % self.clients.len()]
    }

    pub(crate) async fn acquire(
        &self,
        cancel: &CancellationToken,
    ) -> Result<OwnedSemaphorePermit, DownloadError> {
        until_cancelled(cancel, self.connections.clone().acquire_owned())
            .await?
            .map_err(|_| DownloadError::Cancelled)
    }

    pub(crate) fn range_request(&self, url: &str, start: u64, end_inclusive: u64) -> reqwest::RequestBuilder {
        self.client()
            .get(url)
            .header(header::RANGE, format!("bytes={start}-{end_inclusive}"))
    }

    /// Runs `op` against the sources until it succeeds.
    ///
    /// Transient failures rotate to the next source after a backoff; a source
    /// that refuses the file (e.g. 404 on a mirror) is dropped immediately.
    async fn retrying<T, F>(
        &self,
        sources: &[String],
        cancel: &CancellationToken,
        mut op: impl FnMut(String) -> F,
    ) -> Result<T, DownloadError>
    where
        F: Future<Output = Result<T, DownloadError>>,
    {
        let mut sources: Vec<&str> = sources.iter().map(String::as_str).collect();
        let mut index = 0;
        let mut failures = 0;
        loop {
            let Some(&url) = sources.get(index % sources.len().max(1)) else {
                return Err(DownloadError::NoSource(PathBuf::new()));
            };
            let err = match op(url.to_owned()).await {
                Ok(value) => return Ok(value),
                Err(e) => e,
            };
            if matches!(err, DownloadError::Cancelled) {
                return Err(err);
            }
            if err.is_source_specific() && sources.len() > 1 {
                tracing::debug!(%url, error = %err, "source refused file, trying next");
                sources.remove(index % sources.len());
                continue;
            }
            if !err.is_transient() || failures >= self.config.max_retries {
                return Err(err);
            }
            failures += 1;
            index += 1;
            tracing::debug!(%url, attempt = failures, error = %err, "retrying download");
            backoff(failures, cancel).await?;
        }
    }
}

fn build_client(config: &DownloadConfig) -> reqwest::Result<Client> {
    let builder = Client::builder();
    let builder = if config.http1_only { builder.http1_only() } else { builder };
    builder
        .user_agent(&config.user_agent)
        .connect_timeout(config.connect_timeout)
        .read_timeout(config.read_timeout)
        .tcp_nodelay(true)
        .pool_max_idle_per_host(config.max_connections)
        .http2_adaptive_window(true)
        .build()
}

fn record(
    report: &mut BatchReport,
    progress: &Progress,
    joined: Result<(PathBuf, Result<Outcome, DownloadError>), tokio::task::JoinError>,
) {
    match joined {
        Ok((_, Ok(Outcome::Downloaded))) => report.downloaded += 1,
        Ok((_, Ok(Outcome::Skipped))) => report.skipped += 1,
        Ok((_, Ok(Outcome::Reused))) => report.reused += 1,
        Ok((dest, Err(e))) => {
            progress.file_failed();
            if !matches!(e, DownloadError::Cancelled) {
                tracing::warn!(path = %dest.display(), error = %e, "download failed");
            }
            report.failed.push((dest, e));
        }
        Err(join) => {
            progress.file_failed();
            report
                .failed
                .push((PathBuf::new(), DownloadError::io("", std::io::Error::other(join))));
        }
    }
}

/// Streams a whole response body into `part`, hashing on the fly.
async fn stream_whole(
    mut resp: Response,
    task: &DownloadTask,
    part: &Path,
    length: Option<u64>,
    progress: &Progress,
    counted: &AtomicU64,
    cancel: &CancellationToken,
) -> Result<(), DownloadError> {
    let io = |e| DownloadError::io(part, e);
    let file = tokio::fs::File::create(part).await.map_err(io)?;
    let mut file = BufWriter::with_capacity(WRITE_BUFFER, file);
    let mut hasher = task.checksum.as_ref().map(Checksum::hasher);
    let mut written = 0u64;
    while let Some(chunk) = until_cancelled(cancel, resp.chunk()).await?? {
        if let Some(h) = hasher.as_mut() {
            h.update(&chunk);
        }
        file.write_all(&chunk).await.map_err(io)?;
        let n = chunk.len() as u64;
        written += n;
        counted.fetch_add(n, Relaxed);
        progress.add_bytes(n);
    }
    file.flush().await.map_err(io)?;
    if let Some(expected) = length.filter(|&len| len != written) {
        return Err(DownloadError::SizeMismatch {
            path: task.dest.clone(),
            expected,
            actual: written,
        });
    }
    if let (Some(h), Some(expected)) = (hasher, &task.checksum)
        && !h.matches(expected)
    {
        return Err(DownloadError::ChecksumMismatch(task.dest.clone()));
    }
    Ok(())
}

async fn verify_part(part: &Path, dest: &Path, checksum: Option<&Checksum>) -> Result<(), DownloadError> {
    let Some(checksum) = checksum.cloned() else {
        return Ok(());
    };
    let path = part.to_owned();
    let ok = tokio::task::spawn_blocking(move || verify_file(&path, &checksum))
        .await
        .map_err(|e| DownloadError::io(part, std::io::Error::other(e)))?
        .map_err(|e| DownloadError::io(part, e))?;
    if ok { Ok(()) } else { Err(DownloadError::ChecksumMismatch(dest.to_owned())) }
}

/// Parses `Content-Range: bytes start-end/total` (total may be `*`).
pub(crate) fn content_range(resp: &Response) -> Option<(u64, u64, Option<u64>)> {
    let value = resp.headers().get(header::CONTENT_RANGE)?.to_str().ok()?;
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()))
}

pub(crate) fn check_status(resp: Response) -> Result<Response, DownloadError> {
    if resp.status().is_success() {
        Ok(resp)
    } else {
        Err(DownloadError::Status {
            status: resp.status().as_u16(),
            url: resp.url().to_string(),
        })
    }
}

pub(crate) async fn until_cancelled<F: Future>(
    cancel: &CancellationToken,
    future: F,
) -> Result<F::Output, DownloadError> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(DownloadError::Cancelled),
        out = future => Ok(out),
    }
}

/// Exponential backoff with jitter: 250 ms, 500 ms, 1 s, ... capped at 8 s.
pub(crate) async fn backoff(attempt: u32, cancel: &CancellationToken) -> Result<(), DownloadError> {
    let base = 250u64.saturating_mul(1 << attempt.saturating_sub(1).min(5)).min(8_000);
    let delay = Duration::from_millis(base + fastrand::u64(0..250));
    until_cancelled(cancel, tokio::time::sleep(delay)).await
}

/// Unique temporary path next to `dest`, so concurrent batches never share a part file.
fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(format!(".{:08x}.part", fastrand::u32(..)));
    PathBuf::from(name)
}
