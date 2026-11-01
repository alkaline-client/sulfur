//! Search for mods, resource packs, data packs and shaders, and the folders
//! of one version they install into.

use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use sulfur_download::{CancellationToken, Checksum, DownloadTask, Progress};
use sulfur_minecraft::{launch, loader};
use sulfur_mods::{ModsClient, ProjectRef, Query};

use crate::{CoreError, InstalledMod, LoaderKind, ModHit, ModPage, ModQuery, ModRef, ProjectKind, Result, State, SulfurCore};

/// Search page size.
const PAGE: u32 = 20;
/// Upper bound on files one install pulls in, dependencies included.
const MAX_FILES: usize = 16;

/// Project operations for one installed version and one kind of project.
pub(crate) struct ModsJob {
    client: ModsClient,
    kind: sulfur_mods::ProjectKind,
    /// Where files of this kind go.
    dir: PathBuf,
}

impl ModsJob {
    /// `world` picks the save whose data packs are meant (data packs only).
    pub(crate) fn new(state: &State, version_id: &str, kind: ProjectKind, world: Option<&str>) -> Result<Self> {
        let settings = state.settings.read().unwrap();
        let isolated = settings.game.isolation_for(version_id) == sulfur_core::Isolation::Isolated;
        let key = settings.mods.curseforge_api_key.clone();
        drop(settings);
        let run_dir = state.game_dir().run_dir(version_id, isolated);
        let dir = match kind {
            ProjectKind::Mod => run_dir.join("mods"),
            ProjectKind::ResourcePack => run_dir.join("resourcepacks"),
            ProjectKind::Shader => run_dir.join("shaderpacks"),
            ProjectKind::DataPack => {
                let world = world.ok_or_else(|| CoreError::Other("choose a world for the data pack".into()))?;
                safe_join(&run_dir.join("saves"), world)?.join("datapacks")
            }
        };
        Ok(Self {
            client: ModsClient::new(state.downloader.read().unwrap().clone(), Some(key)),
            kind: kind.into(),
            dir,
        })
    }

    /// Installs a project and, for mods, its required dependencies; returns
    /// the new file names. `project` lists the project on each source it is
    /// on; the first with a fitting file is used.
    pub(crate) async fn install(
        &self,
        project: &[ProjectRef],
        game_version: &str,
        loader: Option<sulfur_mods::Loader>,
        cancel: &CancellationToken,
    ) -> Result<Vec<String>> {
        let mut chosen = None;
        let mut last_error = None;
        for candidate in project {
            match self.client.best_file(candidate.source, self.kind, &candidate.id, game_version, loader, cancel).await {
                Ok(Some(file)) => {
                    chosen = Some((candidate, file));
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(project = %candidate.id, error = %e, "file lookup failed");
                    last_error = Some(e);
                }
            }
        }
        let Some((first, first_file)) = chosen else {
            return Err(match last_error {
                Some(e) => e.into(),
                None => CoreError::NotFound(format!("no file of this project fits Minecraft {game_version}")),
            });
        };
        let source = first.source;
        let mut queue = VecDeque::from([(first.id.clone(), Some(first_file))]);
        let mut seen = HashSet::new();
        let mut tasks = Vec::new();
        let mut names = Vec::new();
        while let Some((id, known)) = queue.pop_front() {
            if !seen.insert(id.clone()) || tasks.len() >= MAX_FILES {
                continue;
            }
            let file = match known {
                Some(file) => Some(file),
                None => self.client.best_file(source, self.kind, &id, game_version, loader, cancel).await?,
            };
            let Some(file) = file else {
                tracing::warn!(project = %id, "dependency has no matching file");
                continue;
            };
            if self.kind == sulfur_mods::ProjectKind::Mod {
                queue.extend(file.required.iter().map(|d| (d.clone(), None)));
            }
            // File names come from Modrinth and CurseForge; skip any that would leave the folder.
            let Ok(dest) = safe_join(&self.dir, &file.file_name) else { continue };
            let mut task = DownloadTask::new(&file.url, dest)
                .size(file.size)
                .checksum(file.sha1.as_deref().and_then(Checksum::sha1_hex));
            for fallback in &file.fallbacks {
                task = task.fallback(fallback);
            }
            names.push(file.file_name.clone());
            tasks.push(task);
        }
        let downloader = self.client_downloader();
        downloader
            .run(tasks, Arc::new(Progress::default()), cancel.clone())
            .await
            .into_result()?;
        Ok(names)
    }

    fn client_downloader(&self) -> sulfur_download::Downloader {
        self.client.downloader().clone()
    }
}

/// Game version and mod loader of an installed version.
fn target(state: &State, version_id: &str) -> Result<(String, Option<LoaderKind>)> {
    let version = launch::load_version(&state.game_dir(), version_id)?;
    Ok((version.jar_id().to_owned(), loader::detect(&version).map(Into::into)))
}

#[uniffi::export]
impl SulfurCore {
    /// Searches projects; with `version_id`, only those for its game version
    /// (and, for mods, its loader). Several sources are searched together and
    /// their results merged.
    /// The categories a search for `kind` can narrow to, by their Modrinth
    /// names (both sources have each of them).
    pub fn mod_categories(&self, kind: ProjectKind) -> Vec<String> {
        sulfur_mods::categories(kind.into()).iter().map(|(name, _)| (*name).to_owned()).collect()
    }

    pub async fn search_mods(&self, query: ModQuery) -> Result<ModPage> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let (game_version, loader) = match &query.version_id {
                    Some(id) => {
                        let (game, loader) = target(&state, id)?;
                        (Some(game), loader.and_then(LoaderKind::mod_loader))
                    }
                    None => (None, None),
                };
                let key = state.settings.read().unwrap().mods.curseforge_api_key.clone();
                let client = ModsClient::new(state.downloader.read().unwrap().clone(), Some(key));
                let sources: Vec<sulfur_mods::Source> = query.sources.iter().map(|s| (*s).into()).collect();
                let page = client
                    .search_many(
                        &sources,
                        &Query {
                            kind: query.kind.into(),
                            text: query.text,
                            game_version,
                            loader,
                            category: query.category,
                            sort: query.sort.into(),
                            offset: query.offset,
                            limit: PAGE,
                        },
                        &CancellationToken::new(),
                    )
                    .await?;
                Ok(ModPage {
                    hits: page.hits.into_iter().map(Into::into).collect(),
                    total: page.total,
                })
            })
            .await?
    }

    /// Installs the best-matching file of a project (and, for mods, its
    /// required dependencies) into the version's folder for that kind; data
    /// packs go into `world`. `project` lists the project on each source it
    /// is on, preferred first. Returns the new file names.
    pub async fn install_mod(
        &self,
        project: Vec<ModRef>,
        kind: ProjectKind,
        version_id: String,
        world: Option<String>,
    ) -> Result<Vec<String>> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let (game_version, loader) = target(&state, &version_id)?;
                let loader = loader.and_then(LoaderKind::mod_loader);
                if kind == ProjectKind::Mod && loader.is_none() {
                    return Err(CoreError::Other(format!("{version_id} has no mod loader")));
                }
                let refs: Vec<ProjectRef> = project.into_iter().map(Into::into).collect();
                ModsJob::new(&state, &version_id, kind, world.as_deref())?
                    .install(&refs, &game_version, loader, &CancellationToken::new())
                    .await
            })
            .await?
    }

    /// Files in a version's folder for `kind`, sorted by name.
    pub fn installed_mods(&self, version_id: String, kind: ProjectKind, world: Option<String>) -> Vec<InstalledMod> {
        let Ok(job) = ModsJob::new(&self.state, &version_id, kind, world.as_deref()) else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(job.dir) else { return Vec::new() };
        let extension = if kind == ProjectKind::Mod { ".jar" } else { ".zip" };
        let mut mods: Vec<InstalledMod> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let enabled = name.ends_with(extension);
                (enabled || name.ends_with(&format!("{extension}.disabled"))).then(|| InstalledMod {
                    size: e.metadata().map(|m| m.len()).unwrap_or(0),
                    file_name: name,
                    enabled,
                })
            })
            .collect();
        mods.sort_by_key(|m| m.file_name.to_lowercase());
        mods
    }

    /// Worlds (save folders) of a version, sorted by name.
    pub fn worlds(&self, version_id: String) -> Vec<String> {
        let settings = self.state.settings.read().unwrap();
        let isolated = settings.game.isolation_for(&version_id) == sulfur_core::Isolation::Isolated;
        drop(settings);
        let saves = self.state.game_dir().run_dir(&version_id, isolated).join("saves");
        let Ok(entries) = std::fs::read_dir(saves) else { return Vec::new() };
        let mut worlds: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().join("level.dat").is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        worlds.sort_by_key(|w| w.to_lowercase());
        worlds
    }

    /// Enables (`name`) or disables (`name.disabled`) a file.
    pub fn set_mod_enabled(
        &self,
        version_id: String,
        kind: ProjectKind,
        world: Option<String>,
        file_name: String,
        enabled: bool,
    ) -> Result<()> {
        let dir = ModsJob::new(&self.state, &version_id, kind, world.as_deref())?.dir;
        let current = safe_join(&dir, &file_name)?;
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        let target = safe_join(&dir, &if enabled { base.to_owned() } else { format!("{base}.disabled") })?;
        if current != target {
            std::fs::rename(&current, &target).map_err(|e| CoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Deletes a file from a version's folder for `kind`.
    pub fn remove_mod(&self, version_id: String, kind: ProjectKind, world: Option<String>, file_name: String) -> Result<()> {
        let dir = ModsJob::new(&self.state, &version_id, kind, world.as_deref())?.dir;
        std::fs::remove_file(safe_join(&dir, &file_name)?).map_err(|e| CoreError::Io(e.to_string()))
    }
}

/// Appends a further page of results to `existing`, folding projects already
/// shown (from another source) into their earlier hit.
#[uniffi::export]
pub fn merge_mod_hits(existing: Vec<ModHit>, incoming: Vec<ModHit>) -> Vec<ModHit> {
    let existing = existing.into_iter().map(Into::into).collect();
    sulfur_mods::merge(existing, vec![incoming.into_iter().map(Into::into).collect()])
        .into_iter()
        .map(Into::into)
        .collect()
}

/// `dir/name` for a plain file name (no separators or `..`).
fn safe_join(dir: &std::path::Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." || name == "." {
        return Err(CoreError::Other(format!("invalid file name {name:?}")));
    }
    Ok(dir.join(name))
}
