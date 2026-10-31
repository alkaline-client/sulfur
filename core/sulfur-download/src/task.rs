use std::path::PathBuf;

use crate::Checksum;

/// One file to download.
#[derive(Clone, Debug)]
pub struct DownloadTask {
    pub url: String,
    /// Extra sources tried after `url` and its mirrors.
    pub fallbacks: Vec<String>,
    pub dest: PathBuf,
    /// Expected size, if known. Enables up-front progress totals and segmentation.
    pub size: Option<u64>,
    pub checksum: Option<Checksum>,
    /// Existing copies elsewhere on disk (e.g. another launcher's game
    /// directory). A copy that verifies is linked or copied instead of downloaded.
    pub reuse: Vec<PathBuf>,
}

impl DownloadTask {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            fallbacks: Vec::new(),
            dest: dest.into(),
            size: None,
            checksum: None,
            reuse: Vec::new(),
        }
    }

    pub fn size(mut self, size: Option<u64>) -> Self {
        self.size = size;
        self
    }

    pub fn checksum(mut self, checksum: Option<Checksum>) -> Self {
        self.checksum = checksum;
        self
    }

    pub fn fallback(mut self, url: impl Into<String>) -> Self {
        self.fallbacks.push(url.into());
        self
    }

    pub fn reuse_from(mut self, path: impl Into<PathBuf>) -> Self {
        self.reuse.push(path.into());
        self
    }
}
