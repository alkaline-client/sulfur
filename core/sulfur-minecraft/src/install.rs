//! Installing a version: metadata first, then everything else in one batch.

use std::{
    fs::File,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use sulfur_download::{CancellationToken, Checksum, DownloadTask, Downloader, Progress};

use crate::{
    McError, Result,
    assets::AssetIndex,
    layout::GameDir,
    library::{self, ResolvedLibrary},
    manifest::{ManifestVersion, VERSION_MANIFEST_URL, VersionManifest},
    platform::Platform,
    rules::RuleContext,
    version::VersionJson,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InstallStage {
    Preparing = 0,
    Downloading = 1,
    Extracting = 2,
    /// Installing a mod loader on top of the game.
    Loader = 3,
    /// Not set by the installer itself: callers may chain more steps (a loader
    /// after the game), so they report completion.
    Done = 4,
}

/// Observable state of a running install, shared with the UI.
#[derive(Debug, Default)]
pub struct InstallMonitor {
    pub progress: Arc<Progress>,
    stage: AtomicU8,
}

impl InstallMonitor {
    pub fn stage(&self) -> InstallStage {
        match self.stage.load(Ordering::Relaxed) {
            0 => InstallStage::Preparing,
            1 => InstallStage::Downloading,
            2 => InstallStage::Extracting,
            3 => InstallStage::Loader,
            _ => InstallStage::Done,
        }
    }

    pub fn set_stage(&self, stage: InstallStage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }
}

pub struct Installer {
    downloader: Downloader,
    dir: GameDir,
    platform: Platform,
    reuse: Vec<GameDir>,
}

impl Installer {
    pub fn new(downloader: Downloader, dir: GameDir, platform: Platform) -> Self {
        Self {
            downloader,
            dir,
            platform,
            reuse: Vec::new(),
        }
    }

    /// Game directories whose verified files are reused instead of downloaded
    /// (e.g. the official launcher's). Their layout must match ours.
    pub fn reuse_from(mut self, dirs: impl IntoIterator<Item = GameDir>) -> Self {
        self.reuse = dirs.into_iter().filter(|d| d.root() != self.dir.root()).collect();
        self
    }

    /// Adds the same file in every reuse directory as a reuse candidate.
    fn with_reuse(&self, mut task: DownloadTask) -> DownloadTask {
        for other in &self.reuse {
            if let Some(path) = self.dir.rebase(&task.dest, other) {
                task.reuse.push(path);
            }
        }
        task
    }

    pub fn dir(&self) -> &GameDir {
        &self.dir
    }

    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Fetches the version list and caches it in the game directory.
    pub async fn fetch_manifest(&self, cancel: &CancellationToken) -> Result<VersionManifest> {
        let bytes = self.downloader.fetch(VERSION_MANIFEST_URL, cancel).await?;
        let manifest: VersionManifest = serde_json::from_slice(&bytes).map_err(|source| McError::Json {
            path: VERSION_MANIFEST_URL.into(),
            source,
        })?;
        let cache = self.dir.manifest_cache();
        if let Some(parent) = cache.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = tokio::fs::write(&cache, &bytes).await {
            tracing::warn!(error = %e, "could not cache version manifest");
        }
        Ok(manifest)
    }

    /// The last fetched version list, for offline use.
    pub fn cached_manifest(&self) -> Option<VersionManifest> {
        crate::error::read_json(&self.dir.manifest_cache()).ok()
    }

    /// Downloads and prepares everything needed to launch `entry`.
    pub async fn install(
        &self,
        entry: &ManifestVersion,
        monitor: &InstallMonitor,
        cancel: &CancellationToken,
    ) -> Result<VersionJson> {
        let progress = &monitor.progress;
        monitor.set_stage(InstallStage::Preparing);

        let json_path = self.dir.version_json(&entry.id);
        let json_task = DownloadTask::new(&entry.url, &json_path).checksum(Checksum::sha1_hex(&entry.sha1));
        self.batch(vec![json_task], progress, cancel).await?;
        self.install_json(VersionJson::read(&json_path)?, monitor, cancel).await
    }

    /// Downloads and prepares everything a version JSON already in place
    /// needs (client jar, libraries, assets, natives).
    pub async fn install_json(
        &self,
        version: VersionJson,
        monitor: &InstallMonitor,
        cancel: &CancellationToken,
    ) -> Result<VersionJson> {
        let progress = &monitor.progress;
        monitor.set_stage(InstallStage::Preparing);
        let index_ref = version
            .asset_index
            .clone()
            .ok_or_else(|| McError::Invalid(format!("{} has no asset index", version.id)))?;
        let index_path = self.dir.asset_index(&index_ref.id);
        let index_task = DownloadTask::new(&index_ref.url, &index_path)
            .size(Some(index_ref.size))
            .checksum(Checksum::sha1_hex(&index_ref.sha1));
        self.batch(vec![index_task], progress, cancel).await?;
        let index = AssetIndex::read(&index_path)?;

        // Client jar, libraries and assets go out as one batch so small and
        // large files share the connection budget.
        let ctx = RuleContext::new(self.platform);
        let mut libraries = library::resolve_all(&version, &ctx);
        self.fill_maven_checksums(&mut libraries, cancel).await;
        let mut tasks = Vec::with_capacity(libraries.len() + index.objects.len() + 1);
        if let Some(client) = version.downloads.as_ref().and_then(|d| d.client.as_ref()) {
            tasks.push(
                DownloadTask::new(&client.url, self.dir.version_jar(&version.id))
                    .size(client.size)
                    .checksum(client.sha1.as_deref().and_then(Checksum::sha1_hex)),
            );
        }
        if let Some(file) = version.logging.as_ref().and_then(|l| l.client.as_ref()).map(|c| &c.file) {
            tasks.push(log_config_task(&self.dir, file));
        }
        tasks.extend(libraries.iter().map(|lib| {
            DownloadTask::new(&lib.url, self.dir.library(&lib.path))
                .size(lib.size)
                .checksum(lib.sha1.as_deref().and_then(Checksum::sha1_hex))
        }));
        tasks.extend(index.objects.values().map(|obj| {
            DownloadTask::new(obj.url(), self.dir.asset_object(&obj.hash))
                .size(Some(obj.size))
                .checksum(Checksum::sha1_hex(&obj.hash))
        }));
        monitor.set_stage(InstallStage::Downloading);
        self.batch(tasks, progress, cancel).await?;

        monitor.set_stage(InstallStage::Extracting);
        let (dir, id) = (self.dir.clone(), version.id.clone());
        tokio::task::spawn_blocking(move || {
            extract_natives(&dir, &id, &libraries)?;
            materialize_legacy_assets(&dir, &index_ref.id, &index)
        })
        .await
        .map_err(|e| McError::Invalid(format!("install worker failed: {e}")))??;

        Ok(version)
    }

    /// Platform patches point at Maven Central, which Mojang's metadata has no
    /// checksums for; fetch the published `.sha1` files so they are verified too.
    async fn fill_maven_checksums(&self, libraries: &mut [ResolvedLibrary], cancel: &CancellationToken) {
        use futures_util::{StreamExt, stream};
        let missing: Vec<(usize, String)> = libraries
            .iter()
            .enumerate()
            .filter(|(_, l)| l.sha1.is_none() && l.url.starts_with(library::MAVEN_CENTRAL_URL))
            .map(|(i, l)| (i, format!("{}.sha1", l.url)))
            .collect();
        let found: Vec<_> = stream::iter(missing)
            .map(|(i, url)| async move { (i, self.downloader.fetch(&url, cancel).await) })
            .buffer_unordered(16)
            .collect()
            .await;
        for (i, result) in found {
            let Ok(bytes) = result else { continue };
            let text = String::from_utf8_lossy(&bytes);
            let hex = text.split_whitespace().next().unwrap_or_default();
            if Checksum::sha1_hex(hex).is_some() {
                libraries[i].sha1 = Some(hex.to_owned());
            }
        }
    }

    pub(crate) async fn batch(&self, tasks: Vec<DownloadTask>, progress: &Arc<Progress>, cancel: &CancellationToken) -> Result<()> {
        let tasks = tasks.into_iter().map(|t| self.with_reuse(t)).collect();
        let report = self.downloader.run(tasks, progress.clone(), cancel.clone()).await;
        report.into_result()?;
        Ok(())
    }
}

/// Download of a version's Log4j configuration.
pub fn log_config_task(dir: &GameDir, file: &crate::version::LoggingFile) -> DownloadTask {
    DownloadTask::new(&file.url, dir.log_config(&file.id))
        .size(Some(file.size))
        .checksum(Checksum::sha1_hex(&file.sha1))
}

/// Unpacks the native archives of a resolved version into its own natives
/// folder unless that already has files. Install only does this for vanilla
/// versions; versions made by loaders (Forge 1.12.2, Cleanroom, Legacy Fabric…)
/// run with their own id and need their own copy, made at first launch.
pub fn ensure_natives(dir: &GameDir, version: &VersionJson, platform: Platform) -> Result<()> {
    let natives = dir.natives_dir(&version.id);
    if std::fs::read_dir(&natives).is_ok_and(|mut entries| entries.next().is_some()) {
        return Ok(());
    }
    let libraries = library::resolve_all(version, &RuleContext::new(platform));
    if libraries.iter().any(|l| l.extract) {
        extract_natives(dir, &version.id, &libraries)?;
    }
    Ok(())
}

/// Unpacks legacy native archives into `versions/<id>/natives`.
fn extract_natives(dir: &GameDir, id: &str, libraries: &[ResolvedLibrary]) -> Result<()> {
    let natives = dir.natives_dir(id);
    let _ = std::fs::remove_dir_all(&natives);
    std::fs::create_dir_all(&natives).map_err(|e| McError::io(&natives, e))?;
    for lib in libraries.iter().filter(|l| l.extract) {
        let path = dir.library(&lib.path);
        let file = File::open(&path).map_err(|e| McError::io(&path, e))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| McError::io(&path, e.into()))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| McError::io(&path, e.into()))?;
            let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
            if entry.is_dir() || lib.extract_exclude.iter().any(|p| name.starts_with(p.as_str())) {
                continue;
            }
            // `enclosed_name` rejects absolute paths and `..` (zip slip).
            let Some(relative) = entry.enclosed_name() else { continue };
            let out = natives.join(relative);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| McError::io(parent, e))?;
            }
            let mut target = File::create(&out).map_err(|e| McError::io(&out, e))?;
            std::io::copy(&mut entry, &mut target).map_err(|e| McError::io(&out, e))?;
        }
    }
    Ok(())
}

/// Copies objects to the name-based locations pre-1.7 versions read from.
fn materialize_legacy_assets(dir: &GameDir, index_id: &str, index: &AssetIndex) -> Result<()> {
    let target = if index.map_to_resources {
        // Copied again into the version's run directory at launch if isolated.
        dir.root().join("resources")
    } else if index.is_virtual {
        dir.virtual_assets(index_id)
    } else {
        return Ok(());
    };
    for (name, object) in &index.objects {
        let out = target.join(name);
        if out.metadata().is_ok_and(|m| m.len() == object.size) {
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| McError::io(parent, e))?;
        }
        let src = dir.asset_object(&object.hash);
        std::fs::copy(&src, &out).map_err(|e| McError::io(&src, e))?;
    }
    Ok(())
}
