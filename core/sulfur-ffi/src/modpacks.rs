//! Modrinth modpacks: searching them, and installing a `.mrpack` as a version
//! of its own (the game, its loader, then the pack's files and overrides in
//! the version's isolated folder).

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use sulfur_download::{CancellationToken, Checksum, DownloadTask};
use sulfur_minecraft::loader;
use sulfur_mods::{ModsClient, Query, mrpack::PackIndex};

use crate::{
    CoreError, InstallTask, LoaderKind, ModPage, ModSort, ModpackInfo, ModpackVersion, Result, State, SulfurCore,
    TaskKind, install_job,
};

/// Search page size.
const PAGE: u32 = 20;

/// Where a pack comes from.
enum PackSource {
    Remote(Box<ModpackVersion>),
    Local(PathBuf),
}

#[uniffi::export]
impl SulfurCore {
    /// Searches Modrinth for modpacks.
    pub async fn search_modpacks(&self, text: String, sort: ModSort, offset: u32) -> Result<ModPage> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let client = ModsClient::new(state.downloader.read().unwrap().clone(), None);
                let query = Query {
                    kind: sulfur_mods::ProjectKind::Modpack,
                    text,
                    sort: sort.into(),
                    offset,
                    limit: PAGE,
                    ..Default::default()
                };
                let page = client.search(sulfur_mods::Source::Modrinth, &query, &CancellationToken::new()).await?;
                Ok(ModPage {
                    hits: page.hits.into_iter().map(Into::into).collect(),
                    total: page.total,
                })
            })
            .await?
    }

    /// Releases of a Modrinth modpack, newest first.
    pub async fn modpack_versions(&self, project_id: String) -> Result<Vec<ModpackVersion>> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let client = ModsClient::new(state.downloader.read().unwrap().clone(), None);
                let versions = client.pack_versions(&project_id, &CancellationToken::new()).await?;
                Ok(versions.into_iter().map(Into::into).collect())
            })
            .await?
    }

    /// Reads a `.mrpack` file without installing it.
    pub fn read_modpack_file(&self, path: String) -> Result<ModpackInfo> {
        let index = PackIndex::read(Path::new(&path))?;
        let loader = index.loader()?;
        Ok(ModpackInfo {
            game_version: index.minecraft()?.to_owned(),
            loader: loader.map(|(l, _)| l.into()),
            loader_version: loader.map(|(_, v)| v.to_owned()),
            files: index.client_files()?.len() as u32,
            name: index.name,
            version: index.version_id,
            summary: index.summary,
        })
    }

    /// Installs a modpack release as version `name`. The task is keyed by `name`.
    pub fn install_modpack(&self, version: ModpackVersion, name: String) -> Result<Arc<InstallTask>> {
        self.start_pack(PackSource::Remote(Box::new(version)), name)
    }

    /// Installs a `.mrpack` file from disk as version `name`.
    pub fn install_modpack_file(&self, path: String, name: String) -> Result<Arc<InstallTask>> {
        self.start_pack(PackSource::Local(PathBuf::from(path)), name)
    }
}

impl SulfurCore {
    fn start_pack(&self, source: PackSource, name: String) -> Result<Arc<InstallTask>> {
        let name = name.trim().to_owned();
        if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
            return Err(CoreError::Other(format!("{name:?} is not a valid version name")));
        }
        let dir = self.state.game_dir();
        let mut installs = self.state.installs.lock().unwrap();
        if installs.get(&name).is_some_and(|t| !t.is_finished()) || dir.version_json(&name).is_file() {
            return Err(CoreError::Other(format!("a version named {name} already exists")));
        }
        // Only a folder this install creates is removed if it fails.
        let fresh = !dir.version_dir(&name).exists();
        let task = InstallTask::new(name.clone(), TaskKind::Modpack);
        installs.insert(name.clone(), task.clone());
        let (state, job) = (self.state.clone(), task.clone());
        self.runtime.spawn(async move {
            let result = pack_job(&state, source, &name, &job).await;
            if let Err(e) = &result {
                tracing::warn!(version = %name, error = %e, "modpack install failed");
                if fresh {
                    let _ = std::fs::remove_dir_all(state.game_dir().version_dir(&name));
                    let mut settings = state.settings.write().unwrap();
                    if settings.game.version_isolation.remove(&name).is_some() {
                        let _ = settings.save(&state.dirs.settings_file());
                    }
                }
            }
            *job.outcome.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
        });
        Ok(task)
    }
}

async fn pack_job(state: &State, source: PackSource, name: &str, job: &InstallTask) -> Result<()> {
    let downloader = state.downloader.read().unwrap().clone();
    let pack = match source {
        PackSource::Local(path) => path,
        PackSource::Remote(version) => {
            let file_name: String = version
                .file_name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+') { c } else { '_' })
                .collect();
            let dest = state.dirs.cache.join("modpacks").join(file_name);
            let task = DownloadTask::new(&version.url, &dest)
                .size(version.size)
                .checksum(version.sha1.as_deref().and_then(Checksum::sha1_hex));
            downloader
                .run(vec![task], job.monitor.progress.clone(), job.cancel.clone())
                .await
                .into_result()?;
            dest
        }
    };
    let index = PackIndex::read(&pack)?;
    let files = index.client_files()?;
    let game = index.minecraft()?.to_owned();
    let entry = state
        .official_manifest()
        .await?
        .find(&game)
        .cloned()
        .ok_or_else(|| CoreError::NotFound(format!("unknown Minecraft version {game}")))?;

    let request = match index.loader()? {
        Some((kind, version)) => {
            let kind = LoaderKind::from(kind).into();
            Some(loader::LoaderRequest {
                kind,
                game_version: game.clone(),
                loader_version: loader_version(&downloader, kind, &game, version, &job.cancel).await?,
                name: name.to_owned(),
                installer_url: None,
            })
        }
        None => None,
    };
    install_job(state, &entry, request.as_ref(), false, job).await?;
    let dir = state.game_dir();
    if request.is_none() {
        // A vanilla pack still gets a version of its own, so its files stay apart.
        let json = serde_json::json!({ "id": name, "inheritsFrom": game, "type": "release" });
        let path = dir.version_json(name);
        std::fs::create_dir_all(dir.version_dir(name)).map_err(|e| CoreError::Io(e.to_string()))?;
        std::fs::write(&path, json.to_string()).map_err(|e| CoreError::Io(e.to_string()))?;
    }
    {
        let mut settings = state.settings.write().unwrap();
        settings.game.version_isolation.insert(name.to_owned(), sulfur_core::Isolation::Isolated);
        settings.save(&state.dirs.settings_file())?;
    }

    let run_dir = dir.run_dir(name, true);
    job.monitor.set_stage(sulfur_minecraft::install::InstallStage::Downloading);
    let tasks = files
        .iter()
        .map(|(path, file)| {
            let mut urls = file.downloads.iter();
            let first = urls.next().expect("checked by client_files");
            let mut task = DownloadTask::new(first, run_dir.join(path))
                .size(file.file_size)
                .checksum(file.hashes.get("sha1").and_then(|h| Checksum::sha1_hex(h)));
            for url in urls {
                task = task.fallback(url);
            }
            task
        })
        .collect();
    downloader
        .run(tasks, job.monitor.progress.clone(), job.cancel.clone())
        .await
        .into_result()?;

    job.monitor.set_stage(sulfur_minecraft::install::InstallStage::Extracting);
    let written = tokio::task::spawn_blocking(move || sulfur_mods::mrpack::extract_overrides(&pack, &run_dir)).await??;
    tracing::info!(version = name, files = files.len(), overrides = written, "modpack installed");
    Ok(())
}

/// The loader version Sulfur installs for a pack's: packs name Forge builds
/// without the game version (`47.3.0` is `1.20.1-47.3.0` on Forge's Maven).
async fn loader_version(
    downloader: &sulfur_download::Downloader,
    kind: loader::LoaderKind,
    game: &str,
    wanted: &str,
    cancel: &CancellationToken,
) -> Result<String> {
    match kind {
        loader::LoaderKind::NeoForge if game == "1.20.1" => {
            Err(CoreError::Other("NeoForge for Minecraft 1.20.1 is not supported yet".into()))
        }
        loader::LoaderKind::Forge => {
            let listed = loader::versions(downloader, kind, game, cancel).await.unwrap_or_default();
            let full = format!("{game}-{wanted}");
            Ok(listed
                .into_iter()
                .map(|v| v.version)
                .find(|v| *v == wanted || *v == full || v.starts_with(&format!("{full}-")))
                .unwrap_or(full))
        }
        _ => Ok(wanted.to_owned()),
    }
}
