use std::time::Duration;

/// Tuning knobs for a [`crate::Downloader`].
#[derive(Clone, Debug)]
pub struct DownloadConfig {
    /// Upper bound on concurrent HTTP requests across all files and segments.
    pub max_connections: usize,
    /// Number of independent HTTP clients (connection pools) requests are
    /// spread over. Only useful with HTTP/2, where each client multiplexes its
    /// requests over few connections; with HTTP/1.1 one pool already opens a
    /// connection per request and keeps them warm for the next ones.
    pub http_clients: usize,
    /// Files at least this large are downloaded in parallel byte ranges.
    pub segment_threshold: u64,
    /// A range is never split into pieces smaller than this.
    pub min_segment_size: u64,
    /// Maximum parallel ranges for a single file.
    pub max_segments_per_file: usize,
    /// Retries per file (or per segment) for transient failures.
    pub max_retries: u32,
    pub connect_timeout: Duration,
    /// Maximum time without receiving any data before a request is abandoned.
    pub read_timeout: Duration,
    pub user_agent: String,
    /// Hash files that already exist before deciding to skip them.
    pub verify_existing: bool,
    /// Speak HTTP/1.1 only (one request per connection) instead of letting
    /// servers pick HTTP/2. Many parallel connections ramp up and recover from
    /// loss independently, where HTTP/2 streams share one connection's
    /// congestion window (and one proxy tunnel); measured faster for installs.
    pub http1_only: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_connections: 256,
            http_clients: 1,
            segment_threshold: 4 * 1024 * 1024,
            min_segment_size: 1024 * 1024,
            max_segments_per_file: 8,
            max_retries: 5,
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(10),
            user_agent: concat!("Sulfur/", env!("CARGO_PKG_VERSION")).to_owned(),
            verify_existing: true,
            http1_only: true,
        }
    }
}
