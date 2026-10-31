//! Parallel byte-range downloads with dynamic work stealing.
//!
//! A file starts as one range owned by the request that discovered its size.
//! Helper workers join as connection permits become available; each one takes
//! the second half of the largest range still in flight. The owner of a range
//! notices its end moved and stops early, so ranges never overlap and the
//! slowest connection only ever holds a small tail. A range that stops
//! receiving data is taken over whole by the next idle worker, and failed
//! ranges are resumed (not restarted) in further rounds.

use std::{
    io::SeekFrom,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
    time::{Duration, Instant},
};

use reqwest::{Response, StatusCode};
use tokio::{
    io::{AsyncSeekExt, AsyncWriteExt, BufWriter},
    sync::OwnedSemaphorePermit,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    DownloadError, Progress,
    engine::{Inner, backoff, check_status, content_range, until_cancelled},
};

const WRITE_BUFFER: usize = 256 * 1024;
/// A range that has not advanced for this long may be taken over by an idle worker.
const STALL_AFTER: Duration = Duration::from_secs(4);

pub(crate) struct Job {
    pub url: String,
    pub part: PathBuf,
    pub total: u64,
    pub progress: Arc<Progress>,
    pub counted: Arc<AtomicU64>,
    pub cancel: CancellationToken,
}

#[derive(Debug)]
struct Segment {
    /// Next byte to write.
    pos: u64,
    /// Exclusive end; may shrink when another worker steals the tail.
    end: u64,
    /// Whether a live worker is responsible for this segment.
    owned: bool,
    /// Last time this segment received data.
    advanced: Instant,
}

impl Segment {
    fn new(pos: u64, end: u64) -> Self {
        Self {
            pos,
            end,
            owned: true,
            advanced: Instant::now(),
        }
    }
}

struct Ranges {
    segments: Mutex<Vec<Segment>>,
    min_split: u64,
}

impl Ranges {
    fn new(total: u64, min_split: u64) -> Self {
        Self {
            segments: Mutex::new(vec![Segment::new(0, total)]),
            min_split: min_split.max(64 * 1024),
        }
    }

    /// Picks work for an idle worker: an abandoned segment first, then the
    /// whole remainder of a stalled one, otherwise the second half of the
    /// largest one still in progress.
    fn claim(&self) -> Option<usize> {
        let mut segs = self.segments.lock().unwrap();
        if let Some(i) = segs.iter().position(|s| !s.owned && s.pos < s.end) {
            segs[i].owned = true;
            segs[i].advanced = Instant::now();
            return Some(i);
        }
        if let Some(i) = segs.iter().position(|s| s.pos < s.end && s.advanced.elapsed() > STALL_AFTER) {
            // The stalled owner finds its range empty on its next chunk and moves on.
            let (pos, end) = (segs[i].pos, segs[i].end);
            segs[i].end = pos;
            segs.push(Segment::new(pos, end));
            return Some(segs.len() - 1);
        }
        let (i, remaining) = segs
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.end - s.pos))
            .max_by_key(|&(_, r)| r)?;
        if remaining < 2 * self.min_split {
            return None;
        }
        let mid = segs[i].pos + remaining / 2;
        let end = std::mem::replace(&mut segs[i].end, mid);
        segs.push(Segment::new(mid, end));
        Some(segs.len() - 1)
    }

    /// Reserves up to `len` bytes of segment `i`. Returns how many may be
    /// written, or `None` once the segment is finished.
    fn reserve(&self, i: usize, len: usize) -> Option<usize> {
        let mut segs = self.segments.lock().unwrap();
        let seg = &mut segs[i];
        let allowed = (seg.end - seg.pos).min(len as u64);
        seg.pos += allowed;
        seg.advanced = Instant::now();
        (allowed > 0).then_some(allowed as usize)
    }

    fn position(&self, i: usize) -> (u64, u64) {
        let segs = self.segments.lock().unwrap();
        (segs[i].pos, segs[i].end)
    }

    fn release(&self, i: usize) {
        self.segments.lock().unwrap()[i].owned = false;
    }

    fn release_all(&self) {
        self.segments.lock().unwrap().iter_mut().for_each(|s| s.owned = false);
    }

    fn is_complete(&self) -> bool {
        self.segments.lock().unwrap().iter().all(|s| s.pos >= s.end)
    }

    fn unfinished(&self) -> usize {
        self.segments.lock().unwrap().iter().filter(|s| s.pos < s.end).count()
    }
}

pub(crate) async fn download(
    inner: Arc<Inner>,
    job: Job,
    first: Response,
    permit: OwnedSemaphorePermit,
) -> Result<(), DownloadError> {
    let io = |e| DownloadError::io(&job.part, e);
    // Preallocate so each worker can write at its own offset.
    let file = tokio::fs::File::create(&job.part).await.map_err(io)?;
    file.set_len(job.total).await.map_err(io)?;
    drop(file);

    let cfg = &inner.config;
    let job = Arc::new(job);
    let ranges = Arc::new(Ranges::new(job.total, cfg.min_segment_size));
    let mut first = Some((first, permit));
    let mut round = 0;
    loop {
        let error = run_round(&inner, &job, &ranges, first.take()).await;
        if ranges.is_complete() {
            return Ok(());
        }
        round += 1;
        let error = error.unwrap_or_else(|| DownloadError::BadRange(job.url.clone()));
        if !error.is_transient() || round > cfg.max_retries {
            return Err(error);
        }
        // Keep what was downloaded; the next round resumes the remaining ranges.
        tracing::debug!(url = %job.url, round, error = %error, "resuming segmented download");
        backoff(round, &job.cancel).await?;
    }
}

/// Runs workers until no range can be claimed. Returns the first error seen.
async fn run_round(
    inner: &Arc<Inner>,
    job: &Arc<Job>,
    ranges: &Arc<Ranges>,
    first: Option<(Response, OwnedSemaphorePermit)>,
) -> Option<DownloadError> {
    let cfg = &inner.config;
    let finished = CancellationToken::new();
    let mut workers = JoinSet::new();
    let mut claimers = (cfg.max_segments_per_file.max(1))
        .min((job.total / ranges.min_split).max(1) as usize)
        .max(ranges.unfinished());
    if let Some((resp, permit)) = first {
        workers.spawn(worker(inner.clone(), job.clone(), ranges.clone(), Some(resp), permit));
        claimers -= 1;
    } else {
        // Resumed round: hand abandoned ranges back to fresh workers.
        ranges.release_all();
    }
    for _ in 0..claimers {
        let (inner, job, ranges, finished) = (inner.clone(), job.clone(), ranges.clone(), finished.clone());
        workers.spawn(async move {
            // Give up waiting for a connection once the file is done.
            let permit = tokio::select! {
                permit = inner.acquire(&job.cancel) => permit?,
                _ = finished.cancelled() => return Ok(()),
            };
            worker(inner, job, ranges, None, permit).await
        });
    }

    let mut first_error = None;
    while let Some(joined) = workers.join_next().await {
        match joined {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                first_error.get_or_insert(e);
            }
            Err(e) => {
                first_error.get_or_insert(DownloadError::io(&job.part, std::io::Error::other(e)));
            }
        }
        if ranges.is_complete() {
            finished.cancel();
        }
    }
    first_error
}

async fn worker(
    inner: Arc<Inner>,
    job: Arc<Job>,
    ranges: Arc<Ranges>,
    mut first: Option<Response>,
    _permit: OwnedSemaphorePermit,
) -> Result<(), DownloadError> {
    loop {
        // The initial worker owns segment 0 and already has its response.
        let index = if first.is_some() { 0 } else {
            match ranges.claim() {
                Some(i) => i,
                None => return Ok(()),
            }
        };
        let mut failures = 0;
        loop {
            let attempt = async {
                let resp = match first.take() {
                    Some(resp) => resp,
                    None => {
                        let (pos, end) = ranges.position(index);
                        if pos >= end {
                            return Ok(());
                        }
                        open_range(&inner, &job, pos, end).await?
                    }
                };
                stream_segment(&job, &ranges, index, resp).await
            };
            match attempt.await {
                Ok(()) => break,
                Err(DownloadError::Cancelled) => return Err(DownloadError::Cancelled),
                Err(e) if e.is_transient() && failures < inner.config.max_retries => {
                    failures += 1;
                    tracing::debug!(url = %job.url, segment = index, error = %e, "retrying segment");
                    backoff(failures, &job.cancel).await?;
                }
                Err(e) => {
                    // Hand the remainder to another worker.
                    ranges.release(index);
                    return Err(e);
                }
            }
        }
    }
}

async fn open_range(inner: &Inner, job: &Job, pos: u64, end: u64) -> Result<Response, DownloadError> {
    let request = inner.range_request(&job.url, pos, end - 1);
    let resp = check_status(until_cancelled(&job.cancel, request.send()).await??)?;
    match content_range(&resp) {
        Some((start, _, _)) if resp.status() == StatusCode::PARTIAL_CONTENT && start == pos => Ok(resp),
        _ => Err(DownloadError::BadRange(job.url.clone())),
    }
}

async fn stream_segment(
    job: &Job,
    ranges: &Ranges,
    index: usize,
    mut resp: Response,
) -> Result<(), DownloadError> {
    let io = |e| DownloadError::io(&job.part, e);
    let (start, _) = ranges.position(index);
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&job.part)
        .await
        .map_err(io)?;
    let mut file = BufWriter::with_capacity(WRITE_BUFFER, file);
    file.seek(SeekFrom::Start(start)).await.map_err(io)?;

    while let Some(chunk) = until_cancelled(&job.cancel, resp.chunk()).await?? {
        let Some(allowed) = ranges.reserve(index, chunk.len()) else {
            break;
        };
        file.write_all(&chunk[..allowed]).await.map_err(io)?;
        job.counted.fetch_add(allowed as u64, Relaxed);
        job.progress.add_bytes(allowed as u64);
        if allowed < chunk.len() {
            // Our tail was stolen (or the segment is done); drop the connection.
            break;
        }
    }
    file.flush().await.map_err(io)?;

    let (pos, end) = ranges.position(index);
    if pos < end {
        // Body ended early; the retry resumes from `pos`.
        return Err(DownloadError::SizeMismatch {
            path: job.part.clone(),
            expected: end,
            actual: pos,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stealing_splits_largest_range_without_overlap() {
        let r = Ranges::new(8 << 20, 1 << 20);
        let a = r.claim().unwrap(); // splits [0, 8M) at 4M
        assert_eq!(r.position(0), (0, 4 << 20));
        assert_eq!(r.position(a), (4 << 20, 8 << 20));
        // Owner of segment 0 writes 3.5 MiB; only 0.5 MiB remains, too small to split.
        assert_eq!(r.reserve(0, 7 << 19), Some(7 << 19));
        let b = r.claim().unwrap(); // splits segment `a` instead
        assert_eq!(r.position(a), (4 << 20, 6 << 20));
        assert_eq!(r.position(b), (6 << 20, 8 << 20));
        // A reservation never crosses the (shrunk) end.
        assert_eq!(r.reserve(0, 1 << 20), Some(1 << 19));
        assert_eq!(r.reserve(0, 1), None);
    }

    #[test]
    fn stalled_segments_are_taken_over_whole() {
        let r = Ranges::new(1 << 20, 1 << 20);
        assert_eq!(r.claim(), None); // too small to split, not stalled
        r.segments.lock().unwrap()[0].advanced -= STALL_AFTER * 2;
        let t = r.claim().unwrap();
        assert_eq!(r.position(0), (0, 0));
        assert_eq!(r.position(t), (0, 1 << 20));
        assert_eq!(r.reserve(0, 10), None);
    }

    #[test]
    fn abandoned_segments_are_reclaimed_first() {
        let r = Ranges::new(4 << 20, 1 << 20);
        let a = r.claim().unwrap();
        r.release(a);
        assert_eq!(r.claim(), Some(a));
        assert!(!r.is_complete());
    }
}
