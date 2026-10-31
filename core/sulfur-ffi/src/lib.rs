//! The Rust core as seen from the Kotlin UI.
//!
//! `SulfurCore` owns a Tokio runtime; async methods run on it and are awaited
//! from Kotlin coroutines through UniFFI. Long operations return objects
//! (`InstallTask`, `GameSession`) that the UI polls.

mod accounts;
mod error;
mod modpacks;
mod mods;
mod tasks;
mod types;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use sulfur_core::AppDirs;
use sulfur_download::{DownloadConfig, Downloader};
use sulfur_download::{CancellationToken, DownloadTask, Progress};
use sulfur_minecraft::{
    banners, groups,
    install::Installer,
    java, launch,
    layout::{self, GameDir},
    loader,
    manifest::{ManifestVersion, VersionManifest},
    news, omniarchive,
    platform::Platform,
    process::GameProcess,
    region::Region,
    runtime, sources,
};

pub use accounts::SignInTask;
pub use error::CoreError;
use mods::ModsJob;
pub use tasks::{GameSession, InstallTask};
pub use types::*;

uniffi::setup_scaffolding!();

type Result<T, E = CoreError> = std::result::Result<T, E>;

#[derive(uniffi::Object)]
pub struct SulfurCore {
    runtime: tokio::runtime::Runtime,
    state: Arc<State>,
}

struct State {
    dirs: AppDirs,
    settings: RwLock<sulfur_core::Settings>,
    downloader: RwLock<Downloader>,
    manifest: Mutex<Option<VersionManifest>>,
    installs: Mutex<HashMap<String, Arc<InstallTask>>>,
    banners: Mutex<Option<Vec<Banner>>>,
    archive: Mutex<Option<Vec<omniarchive::ArchivedJar>>>,
    news: Mutex<Option<Vec<NewsEntry>>>,
    region: Region,
    accounts: Mutex<sulfur_core::Accounts>,
    loader_versions: Mutex<HashMap<(loader::LoaderKind, String), FetchedLoaderVersions>>,
}

/// Loader versions for one loader and game version, and when they were fetched.
type FetchedLoaderVersions = (std::time::Instant, Vec<LoaderVersion>);

/// How long a list of loader versions is reused before it is fetched again.
const LOADER_VERSIONS_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

#[uniffi::export]
impl SulfurCore {
    /// Opens the core. `portable_root` keeps all data under one directory
    /// instead of the platform's standard locations.
    #[uniffi::constructor]
    pub fn new(portable_root: Option<String>) -> Result<Arc<Self>> {
        let dirs = match portable_root {
            Some(root) => AppDirs::portable(root),
            None => AppDirs::standard().map_err(|e| CoreError::Io(e.to_string()))?,
        };
        dirs.create_all().map_err(|e| CoreError::Io(e.to_string()))?;
        init_logging(&dirs);
        let settings = sulfur_core::Settings::load(&dirs.settings_file()).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "using default settings");
            sulfur_core::Settings::default()
        });
        let downloader = build_downloader(&settings)?;
        let mut accounts = sulfur_core::Accounts::load(&sulfur_core::Accounts::file(&dirs.config));
        State::adopt_offline_name(&mut accounts, &settings);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name("sulfur-core")
            .enable_all()
            .build()
            .map_err(|e| CoreError::Other(e.to_string()))?;
        tracing::info!(version = env!("CARGO_PKG_VERSION"), "core started");
        Ok(Arc::new(Self {
            runtime,
            state: Arc::new(State {
                dirs,
                settings: RwLock::new(settings),
                downloader: RwLock::new(downloader),
                manifest: Mutex::new(None),
                installs: Mutex::new(HashMap::new()),
                banners: Mutex::new(None),
                archive: Mutex::new(None),
                news: Mutex::new(None),
                region: Region::detect(),
                accounts: Mutex::new(accounts),
                loader_versions: Mutex::new(HashMap::new()),
            }),
        }))
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    pub fn paths(&self) -> CorePaths {
        let dirs = &self.state.dirs;
        CorePaths {
            config_dir: path_string(&dirs.config),
            data_dir: path_string(&dirs.data),
            logs_dir: path_string(&dirs.logs),
            game_dir: path_string(self.state.game_dir().root()),
            settings_file: path_string(dirs.settings_file()),
        }
    }

    pub fn settings(&self) -> Settings {
        Settings::from(&*self.state.settings.read().unwrap())
    }

    /// Saves settings. Download changes apply to the next operation.
    pub fn update_settings(&self, settings: Settings) -> Result<()> {
        let mut current = self.state.settings.write().unwrap();
        let before = current.download.clone();
        settings.apply_to(&mut current);
        current.save(&self.state.dirs.settings_file())?;
        if current.download != before {
            *self.state.downloader.write().unwrap() = build_downloader(&current)?;
        }
        Ok(())
    }

    /// Whether offline accounts may be used on this machine.
    pub fn account_policy(&self) -> AccountPolicy {
        let mainland_china = self.state.region == Region::MainlandChina;
        AccountPolicy {
            offline_allowed: self.state.offline_allowed(),
            mainland_china,
            microsoft_available: accounts::microsoft_available(),
        }
    }

    /// Other launchers' settings in `folders` (chosen by the user) and in
    /// their usual user-wide locations.
    pub fn find_launcher_configs(&self, folders: Vec<String>) -> Vec<ForeignConfig> {
        let folders: Vec<std::path::PathBuf> = folders.into_iter().map(Into::into).collect();
        sulfur_core::import::scan(&folders).into_iter().map(Into::into).collect()
    }

    /// Copies another launcher's settings into Sulfur's and saves them.
    /// `use_game_dir` makes its selected game folder Sulfur's; otherwise its
    /// folders only serve files that would be downloaded again.
    pub fn import_launcher_config(&self, config: ForeignConfig, use_game_dir: bool) -> Result<Settings> {
        let mut current = self.state.settings.write().unwrap();
        let before = current.download.clone();
        let config = sulfur_core::import::ForeignConfig::from(config);
        config.apply(&mut current, use_game_dir);
        current.save(&self.state.dirs.settings_file())?;
        // Its offline player comes along as an account (where offline play is allowed, it can be used).
        if let Some(name) = config.offline_name.as_deref().map(str::trim) {
            let mut accounts = self.state.accounts.lock().unwrap();
            if accounts.find_offline(name).is_none() && sulfur_core::check_offline_name(name).is_ok() {
                accounts.offline.push(sulfur_core::OfflineAccount { name: name.to_owned() });
                self.state.save_accounts(&accounts)?;
            }
        }
        if current.download != before {
            *self.state.downloader.write().unwrap() = build_downloader(&current)?;
        }
        Ok(Settings::from(&*current))
    }

    /// Client jars preserved by Omniarchive that Mojang no longer offers.
    /// Cached in memory and on disk; `refresh` asks the archive again.
    pub async fn archived_versions(&self, refresh: bool) -> Result<Vec<ArchivedVersion>> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let cache_file = state.dirs.cache.join("omniarchive.json");
                let from_disk = || -> Option<Vec<omniarchive::ArchivedJar>> {
                    serde_json::from_slice(&std::fs::read(&cache_file).ok()?).ok()
                };
                let cached = state.archive.lock().unwrap().clone().or_else(|| if refresh { None } else { from_disk() });
                let jars = match cached {
                    Some(jars) if !refresh => jars,
                    _ => {
                        let official = state.official_manifest().await?;
                        let downloader = state.downloader.read().unwrap().clone();
                        match omniarchive::list(&downloader, &official, &CancellationToken::new()).await {
                            Ok(jars) => {
                                if let Ok(bytes) = serde_json::to_vec(&jars) {
                                    let _ = std::fs::write(&cache_file, bytes);
                                }
                                jars
                            }
                            Err(e) => from_disk().ok_or(e)?,
                        }
                    }
                };
                *state.archive.lock().unwrap() = Some(jars.clone());
                let installed: std::collections::HashSet<_> =
                    state.game_dir().installed_versions().into_iter().collect();
                Ok(jars
                    .iter()
                    .map(|j| ArchivedVersion {
                        installed: installed.contains(&j.id),
                        ..j.into()
                    })
                    .collect())
            })
            .await?
    }

    /// Installs an Omniarchive jar listed by [`Self::archived_versions`].
    pub fn install_archived(&self, id: String) -> Result<Arc<InstallTask>> {
        let jar = self
            .state
            .archive
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|jars| jars.iter().find(|j| j.id == id).cloned())
            .ok_or_else(|| CoreError::NotFound(format!("unknown archived version {id}")))?;
        let mut installs = self.state.installs.lock().unwrap();
        if let Some(task) = installs.get(&id).filter(|t| !t.is_finished()) {
            return Ok(task.clone());
        }
        let task = InstallTask::new(id.clone());
        installs.insert(id, task.clone());
        let (state, job) = (self.state.clone(), task.clone());
        self.runtime.spawn(async move {
            let result = async {
                let official = state.official_manifest().await?;
                omniarchive::install(&state.installer(), &official, &jar, &job.monitor, &job.cancel).await?;
                Ok::<_, CoreError>(())
            }
            .await;
            if let Err(e) = &result {
                tracing::warn!(version = %jar.id, error = %e, "archived install failed");
            }
            *job.outcome.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
        });
        Ok(task)
    }

    /// Overrides isolation for one version; `None` follows the global default.
    pub fn set_version_isolation(&self, version_id: String, isolation: Option<Isolation>) -> Result<()> {
        let mut settings = self.state.settings.write().unwrap();
        match isolation {
            Some(i) => settings.game.version_isolation.insert(version_id, i.into()),
            None => settings.game.version_isolation.remove(&version_id),
        };
        settings.save(&self.state.dirs.settings_file())?;
        Ok(())
    }

    /// One version's overrides of the game and Java settings.
    pub fn version_settings(&self, version_id: String) -> VersionSettings {
        let settings = self.state.settings.read().unwrap();
        settings.game.version_settings.get(&version_id).map(Into::into).unwrap_or_default()
    }

    /// Replaces one version's overrides; all `None` removes them.
    pub fn set_version_settings(&self, version_id: String, overrides: VersionSettings) -> Result<()> {
        let overrides = sulfur_core::VersionSettings::from(overrides);
        let mut settings = self.state.settings.write().unwrap();
        if overrides.is_empty() {
            settings.game.version_settings.remove(&version_id);
        } else {
            settings.game.version_settings.insert(version_id, overrides);
        }
        settings.save(&self.state.dirs.settings_file())?;
        Ok(())
    }

    /// The desktop's wallpaper, converted to a format the UI decodes when
    /// needed. Runs a few short system commands; call it off the UI thread.
    pub fn desktop_wallpaper(&self) -> Option<DesktopWallpaper> {
        match sulfur_core::wallpaper::detect()? {
            sulfur_core::wallpaper::Wallpaper::Color(rgb) => Some(DesktopWallpaper { path: None, color: Some(rgb) }),
            sulfur_core::wallpaper::Wallpaper::Image(path) => {
                let usable = sulfur_core::wallpaper::decodable(&path, &self.state.dirs.cache.join("wallpaper"))?;
                Some(DesktopWallpaper { path: Some(path_string(usable)), color: None })
            }
        }
    }

    /// Java Edition news from the official launcher's feed, newest first.
    pub async fn launcher_news(&self, refresh: bool) -> Result<Vec<NewsEntry>> {
        if !refresh && let Some(cached) = self.state.news.lock().unwrap().clone() {
            return Ok(cached);
        }
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let downloader = state.downloader.read().unwrap().clone();
                let news: Vec<NewsEntry> = news::fetch(&downloader, &CancellationToken::new())
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect();
                *state.news.lock().unwrap() = Some(news.clone());
                Ok(news)
            })
            .await?
    }

    /// A text document for the Launch page: a local file, or an `https` page
    /// of at most 512 KiB.
    pub async fn read_document(&self, location: String) -> Result<String> {
        const LIMIT: usize = 512 * 1024;
        let location = location.trim().to_owned();
        if !location.starts_with("https://") {
            let path = std::path::PathBuf::from(location.strip_prefix("file://").unwrap_or(&location));
            let bytes = std::fs::read(&path).map_err(|e| CoreError::Io(format!("{}: {e}", path.display())))?;
            return Ok(String::from_utf8_lossy(&bytes[..bytes.len().min(LIMIT)]).into_owned());
        }
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let downloader = state.downloader.read().unwrap().clone();
                let bytes = downloader.fetch(&location, &CancellationToken::new()).await?;
                Ok(String::from_utf8_lossy(&bytes[..bytes.len().min(LIMIT)]).into_owned())
            })
            .await?
    }

    /// The Minecraft version list. Uses the in-memory copy unless `refresh`;
    /// falls back to the on-disk cache when offline.
    pub async fn version_list(&self, refresh: bool) -> Result<VersionList> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let cached = state.manifest.lock().unwrap().clone();
                let (manifest, from_cache) = match cached {
                    Some(m) if !refresh => (m, false),
                    _ => {
                        let installer = state.installer();
                        match installer.fetch_manifest(&Default::default()).await {
                            Ok(m) => (m, false),
                            Err(e) => match installer.cached_manifest() {
                                Some(m) => {
                                    tracing::warn!(error = %e, "using cached version list");
                                    (m, true)
                                }
                                None => return Err(e.into()),
                            },
                        }
                    }
                };
                *state.manifest.lock().unwrap() = Some(manifest.clone());
                let installed: std::collections::HashSet<_> =
                    state.game_dir().installed_versions().into_iter().collect();
                let remote = |v: &ManifestVersion| RemoteVersion {
                    installed: installed.contains(&v.id),
                    id: v.id.clone(),
                    kind: v.kind.into(),
                    release_time: v.release_time.clone(),
                };
                Ok(VersionList {
                    groups: groups::group(&manifest.versions)
                        .iter()
                        .map(|g| VersionGroup {
                            key: g.key.clone(),
                            kind: g.kind.into(),
                            name: g.name.map(String::from),
                            releases: g.releases.iter().map(remote).collect(),
                            previews: g.previews.iter().map(remote).collect(),
                        })
                        .collect(),
                    versions: manifest.versions.iter().map(remote).collect(),
                    latest_release: manifest.latest.release,
                    latest_snapshot: manifest.latest.snapshot,
                    from_cache,
                })
            })
            .await?
    }

    pub fn installed_versions(&self) -> Vec<InstalledVersion> {
        let dir = self.state.game_dir();
        let settings = self.state.settings.read().unwrap();
        dir.installed_versions()
            .into_iter()
            .filter_map(|id| {
                let version = launch::load_version(&dir, &id).ok()?;
                let isolation = settings.game.isolation_for(&id);
                Some(InstalledVersion {
                    loader: loader::detect(&version).map(Into::into),
                    game_version: version.jar_id().to_owned(),
                    kind: version.kind.clone().unwrap_or_else(|| "release".into()),
                    java_major: version.java_major(),
                    isolation: isolation.into(),
                    isolation_overridden: settings.game.version_isolation.contains_key(&id),
                    run_dir: path_string(dir.run_dir(&id, isolation == sulfur_core::Isolation::Isolated)),
                    id,
                })
            })
            .collect()
    }

    /// Starts installing a version from the version list (or returns the
    /// install already running for it).
    pub fn install(&self, version_id: String) -> Result<Arc<InstallTask>> {
        self.install_version(InstallRequest {
            game_version: version_id,
            loader: None,
            loader_version: None,
            name: None,
            fabric_api: false,
            installer_url: None,
        })
    }

    /// Installs a game version, optionally with a mod loader as a new version
    /// that inherits from it. The task is keyed by the resulting version id.
    pub fn install_version(&self, request: InstallRequest) -> Result<Arc<InstallTask>> {
        let loader = match (request.loader, request.loader_version.clone()) {
            (Some(kind), Some(version)) => Some(loader::LoaderRequest {
                kind: kind.into(),
                name: request
                    .name
                    .clone()
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| default_name(&request.game_version, kind, &version)),
                game_version: request.game_version.clone(),
                loader_version: version,
                installer_url: request.installer_url.clone(),
            }),
            (Some(_), None) => return Err(CoreError::Other("choose a loader version".into())),
            _ => None,
        };
        if request.loader == Some(LoaderKind::Cleanroom)
            && request.installer_url.is_none()
            && !self.state.settings.read().unwrap().mods.cleanroom_direct
        {
            return Err(CoreError::Other("pick a Cleanroom installer on its download page".into()));
        }
        let id = loader.as_ref().map_or(request.game_version.clone(), |l| l.name.clone());
        let mut installs = self.state.installs.lock().unwrap();
        if let Some(task) = installs.get(&id).filter(|t| !t.is_finished()) {
            return Ok(task.clone());
        }
        if loader.is_some() && self.state.game_dir().version_json(&id).is_file() {
            return Err(CoreError::Other(format!("a version named {id} already exists")));
        }
        let entry = {
            let manifest = self.state.manifest.lock().unwrap();
            let manifest = manifest.clone().or_else(|| self.state.installer().cached_manifest());
            manifest
                .and_then(|m| m.find(&request.game_version).cloned())
                .ok_or_else(|| CoreError::NotFound(format!("unknown version {}", request.game_version)))?
        };
        let task = InstallTask::new(id.clone());
        installs.insert(id, task.clone());

        let (state, job) = (self.state.clone(), task.clone());
        let fabric_api = request.fabric_api && request.loader == Some(LoaderKind::Fabric);
        self.runtime.spawn(async move {
            let result = install_job(&state, &entry, loader.as_ref(), fabric_api, &job).await;
            if let Err(e) = &result {
                tracing::warn!(version = %entry.id, error = %e, "install failed");
            }
            *job.outcome.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
        });
        Ok(task)
    }

    /// Versions of a loader available for a game version, newest first;
    /// empty when the loader does not support it. Kept for a while, so
    /// opening a version again does not ask every loader's server again.
    pub async fn loader_versions(&self, kind: LoaderKind, game_version: String) -> Result<Vec<LoaderVersion>> {
        let key = (loader::LoaderKind::from(kind), game_version);
        if let Some((at, versions)) = self.state.loader_versions.lock().unwrap().get(&key)
            && at.elapsed() < LOADER_VERSIONS_TTL
        {
            return Ok(versions.clone());
        }
        let downloader = self.state.downloader.read().unwrap().clone();
        let (kind, game_version) = key.clone();
        let found = self
            .runtime
            .spawn(async move { loader::versions(&downloader, kind, &game_version, &CancellationToken::new()).await })
            .await??;
        let versions: Vec<LoaderVersion> = found
            .into_iter()
            .map(|v| LoaderVersion {
                version: v.version,
                stable: v.stable,
            })
            .collect();
        self.state.loader_versions.lock().unwrap().insert(key, (std::time::Instant::now(), versions.clone()));
        Ok(versions)
    }

    /// Artwork for each version group. Cached in memory and on disk; `refresh`
    /// asks the network again.
    pub async fn banners(&self, refresh: bool) -> Result<Vec<Banner>> {
        if !refresh && let Some(cached) = self.state.banners.lock().unwrap().clone() {
            return Ok(cached);
        }
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                // Versioned: older caches lack newer artwork sources.
                let cache_file = state.dirs.cache.join("banners-v3.json");
                let from_disk = || -> Option<Vec<banners::Banner>> {
                    serde_json::from_slice(&std::fs::read(&cache_file).ok()?).ok()
                };
                let cached = if refresh { None } else { from_disk() };
                let found = match cached {
                    Some(found) => found,
                    None => {
                        let manifest = state.manifest.lock().unwrap().clone();
                        let manifest = manifest
                            .or_else(|| state.installer().cached_manifest())
                            .ok_or_else(|| CoreError::NotFound("version list not loaded".into()))?;
                        let groups = groups::group(&manifest.versions);
                        let downloader = state.downloader.read().unwrap().clone();
                        let found = banners::fetch(&downloader, &groups, &CancellationToken::new()).await;
                        if found.iter().any(|b| b.image_url.is_some()) {
                            if let Ok(bytes) = serde_json::to_vec(&found) {
                                let _ = std::fs::write(&cache_file, bytes);
                            }
                            found
                        } else {
                            // Offline: keep whatever was found last time.
                            from_disk().unwrap_or(found)
                        }
                    }
                };
                let found: Vec<Banner> = found
                    .into_iter()
                    .map(|b| Banner {
                        key: b.key,
                        image_url: b.image_url,
                        preview_url: b.preview_url,
                        summary: b.summary,
                    })
                    .collect();
                *state.banners.lock().unwrap() = Some(found.clone());
                Ok(found)
            })
            .await?
    }

    /// Downloads an image (banner, mod icon) into the cache once and returns its path.
    pub async fn cached_image(&self, url: String) -> Result<String> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let dest = state.dirs.cache.join("images").join(image_file_name(&url));
                if !dest.is_file() {
                    let downloader = state.downloader.read().unwrap().clone();
                    downloader
                        .run(vec![DownloadTask::new(&url, &dest)], Arc::new(Progress::default()), CancellationToken::new())
                        .await
                        .into_result()?;
                }
                Ok(path_string(dest))
            })
            .await?
    }

    /// Installs that are running or finished during this session.
    pub fn installs(&self) -> Vec<Arc<InstallTask>> {
        self.state.installs.lock().unwrap().values().cloned().collect()
    }

    pub async fn detect_java(&self) -> Vec<JavaRuntime> {
        let root = self.state.dirs.java_dir();
        let found = self.runtime.spawn(java::discover_in(runtime::executables(&root))).await.unwrap_or_default();
        found.into_iter().map(|j| java_record(j, &root)).collect()
    }

    /// Java major versions Sulfur can download.
    pub fn downloadable_java(&self) -> Vec<u32> {
        runtime::OFFERED.to_vec()
    }

    /// Downloads Java `major` into Sulfur's own runtimes folder (or returns
    /// the download already running). The task is keyed `Java <major>`.
    pub fn install_java(&self, major: u32) -> Arc<InstallTask> {
        let (task, started) = self.state.java_task(major);
        if started {
            let (state, job) = (self.state.clone(), task.clone());
            self.runtime.spawn(async move {
                let result = state.download_java(major, &job).await.map(drop);
                *job.outcome.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
            });
        }
        task
    }

    /// Launches an installed version with the current settings.
    pub async fn launch(&self, version_id: String) -> Result<Arc<GameSession>> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                // Offline play outside mainland China needs a Microsoft account that owns the game.
                let account = state.launch_account().await?;
                let dir = state.game_dir();
                let version = launch::load_version(&dir, &version_id)?;
                let settings = state.settings.read().unwrap().clone();
                let game = settings.game.launch_settings(&version_id);
                let java = state
                    .find_java(game.java_path.as_deref(), settings.game.auto_java, &version_id, version.java_major())
                    .await?;
                if game.log4j_fix {
                    state.ensure_log_config(&dir, &version).await;
                }
                let isolated = settings.game.isolation_for(&version_id) == sulfur_core::Isolation::Isolated;
                let options = launch::LaunchOptions {
                    java: java.path.clone(),
                    run_dir: dir.run_dir(&version_id, isolated),
                    account,
                    max_memory_mib: game.max_memory_mib,
                    min_memory_mib: game.min_memory_mib,
                    resolution: (!game.fullscreen).then_some((game.window_width, game.window_height)),
                    fullscreen: game.fullscreen,
                    extra_jvm_args: launch::split_args(&game.jvm_args),
                    extra_game_args: launch::split_args(&game.game_args),
                    gc: game.gc,
                    java_major: java.major,
                    server: Some(game.server_address.clone()).filter(|s| !s.is_empty()),
                    wrapper: launch::split_args(&game.wrapper_command),
                    env: launch::parse_env(&game.env_vars),
                    log4j_fix: game.log4j_fix,
                    launcher_name: "Sulfur".into(),
                    launcher_version: env!("CARGO_PKG_VERSION").into(),
                };
                launch::prepare_jar(&dir, &version)?;
                sulfur_minecraft::install::ensure_natives(&dir, &version, Platform::current())?;
                let command = launch::build(&dir, &version, &options, Platform::current())?;
                if !game.pre_launch_command.trim().is_empty() {
                    pre_launch(&game.pre_launch_command, &version_id, &options).await?;
                }
                let process = GameProcess::spawn(&command)?;
                Ok(Arc::new(GameSession { version_id, process }))
            })
            .await?
    }
}

/// Installs the game, then the loader and Fabric API if requested.
pub(crate) async fn install_job(
    state: &State,
    entry: &ManifestVersion,
    loader: Option<&loader::LoaderRequest>,
    fabric_api: bool,
    job: &InstallTask,
) -> Result<()> {
    let installer = state.installer();
    let version = installer.install(entry, &job.monitor, &job.cancel).await?;
    let Some(request) = loader else { return Ok(()) };
    job.monitor.set_stage(sulfur_minecraft::install::InstallStage::Loader);
    let settings = state.settings.read().unwrap().clone();
    let java = state
        .find_java(settings.game.java_path.as_deref(), settings.game.auto_java, &entry.id, version.java_major())
        .await?;
    installer
        .install_loader(request, &java.path, &state.dirs.cache, &job.monitor, &job.cancel)
        .await?;
    if fabric_api {
        let fabric_api = [sulfur_mods::ProjectRef {
            source: sulfur_mods::Source::Modrinth,
            id: FABRIC_API.into(),
            page_url: String::new(),
        }];
        let found = match ModsJob::new(state, &request.name, ProjectKind::Mod, None) {
            Ok(mods) => mods.install(&fabric_api, &entry.id, Some(sulfur_mods::Loader::Fabric), &job.cancel).await,
            Err(e) => Err(e),
        };
        if let Err(e) = found {
            // The version works without it; mods that need it will say so.
            tracing::warn!(error = %e, "could not add Fabric API");
        }
    }
    Ok(())
}

/// Runs the player's pre-launch command through the system shell in the
/// game's folder and waits for it. It sees the game's environment plus the
/// `INST_*` variables MultiMC and Prism set, so scripts written for them work.
async fn pre_launch(command: &str, version_id: &str, options: &launch::LaunchOptions) -> Result<()> {
    std::fs::create_dir_all(&options.run_dir).map_err(|e| CoreError::Io(format!("{}: {e}", options.run_dir.display())))?;
    #[cfg(windows)]
    let mut shell = {
        let mut shell = tokio::process::Command::new("cmd");
        shell.arg("/C").raw_arg(command).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        shell
    };
    #[cfg(not(windows))]
    let mut shell = {
        let mut shell = tokio::process::Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    };
    let status = shell
        .current_dir(&options.run_dir)
        .envs(options.env.iter().map(|(k, v)| (k, v)))
        .env("INST_NAME", version_id)
        .env("INST_ID", version_id)
        .env("INST_DIR", &options.run_dir)
        .env("INST_MC_DIR", &options.run_dir)
        .env("INST_JAVA", &options.java)
        .stdin(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| CoreError::Other(format!("the pre-launch command could not start: {e}")))?;
    if !status.success() {
        return Err(CoreError::Other(format!("the pre-launch command failed ({status})")));
    }
    Ok(())
}

/// Modrinth id of Fabric API.
const FABRIC_API: &str = "P7dR8mSH";

fn default_name(game_version: &str, kind: LoaderKind, loader_version: &str) -> String {
    let label = match kind {
        LoaderKind::Fabric => "fabric",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        LoaderKind::OptiFine => "optifine",
        LoaderKind::LegacyFabric => "legacyfabric",
        LoaderKind::Ornithe => "ornithe",
        LoaderKind::Quilt => "quilt",
        LoaderKind::Rift => "rift",
        LoaderKind::Cleanroom => "cleanroom",
    };
    // Forge versions already start with the game version.
    let loader_version = loader_version.strip_prefix(&format!("{game_version}-")).unwrap_or(loader_version);
    format!("{game_version}-{label}-{loader_version}")
}

/// A readable, collision-free cache file name for a URL.
fn image_file_name(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    let last = url.split(['?', '#']).next().unwrap_or_default().rsplit('/').next().unwrap_or_default();
    let clean: String = last
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .rev()
        .take(60)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{hash:016x}-{clean}")
}

/// The Cleanroom version of an installer: a link on the project's own hosts
/// (its download page links to GitHub releases and its Maven repository) or a
/// downloaded installer file. `None` for anything else.
#[uniffi::export]
pub fn cleanroom_installer_version(url: String) -> Option<String> {
    let path = std::path::Path::new(&url);
    let trusted = loader::is_cleanroom_installer_url(&url) || (path.is_absolute() && path.is_file());
    trusted.then(|| loader::cleanroom_installer_version(&url)).flatten()
}

/// Cleanroom's download page, shown inside Sulfur so the project keeps its visitors.
#[uniffi::export]
pub fn cleanroom_download_page() -> String {
    "https://download.cleanroommc.com/".into()
}

impl State {
    /// Mojang's version list: the copy in memory, else a fresh one, else the disk cache.
    async fn official_manifest(&self) -> Result<VersionManifest> {
        if let Some(m) = self.manifest.lock().unwrap().clone() {
            return Ok(m);
        }
        let installer = self.installer();
        let manifest = match installer.fetch_manifest(&CancellationToken::new()).await {
            Ok(m) => m,
            Err(e) => installer.cached_manifest().ok_or(e)?,
        };
        *self.manifest.lock().unwrap() = Some(manifest.clone());
        Ok(manifest)
    }

    /// The configured Java, or the best one for `required`: an installed
    /// runtime that fits (see [`java::pick_compatible`]), else a download of
    /// exactly that version when allowed, else the closest newer one.
    async fn find_java(
        &self,
        java_path: Option<&std::path::Path>,
        auto_java: bool,
        version_id: &str,
        required: u32,
    ) -> Result<java::JavaInstall> {
        let java = match java_path {
            Some(path) => java::probe(path)
                .await
                .ok_or_else(|| CoreError::Java(format!("{} is not a working Java", path.display())))?,
            None => {
                let platform = Platform::current();
                let all = java::discover_in(runtime::executables(&self.dirs.java_dir())).await;
                match java::pick_compatible(&all, required, platform) {
                    Some(java) => java.clone(),
                    None if auto_java && runtime::OFFERED.contains(&required) => {
                        let (task, started) = self.java_task(required);
                        let result = if started {
                            let result = self.download_java(required, &task).await;
                            *task.outcome.lock().unwrap() =
                                Some(result.as_ref().map(drop).map_err(|e| e.to_string()));
                            result
                        } else {
                            self.wait_for_java(&task, required).await
                        };
                        match result {
                            Ok(java) => java,
                            Err(e) => {
                                tracing::warn!(error = %e, "Java download failed");
                                java::pick(&all, required, platform)
                                    .cloned()
                                    .ok_or(sulfur_minecraft::McError::NoJava(required))?
                            }
                        }
                    }
                    None => java::pick(&all, required, platform)
                        .cloned()
                        .ok_or(sulfur_minecraft::McError::NoJava(required))?,
                }
            }
        };
        if java.major < required {
            return Err(CoreError::Java(format!(
                "{version_id} needs Java {required}, but {} is Java {}",
                java.path.display(),
                java.major
            )));
        }
        Ok(java)
    }

    /// The task for downloading Java `major`, and whether it is new (the
    /// caller then runs it). Listed with the installs so the UI shows progress.
    fn java_task(&self, major: u32) -> (Arc<InstallTask>, bool) {
        let id = format!("Java {major}");
        let mut installs = self.installs.lock().unwrap();
        if let Some(task) = installs.get(&id).filter(|t| !t.is_finished()) {
            return (task.clone(), false);
        }
        let task = InstallTask::new(id.clone());
        installs.insert(id, task.clone());
        (task, true)
    }

    async fn download_java(&self, major: u32, task: &InstallTask) -> Result<java::JavaInstall> {
        let root = self.dirs.java_dir();
        Ok(runtime::install(&self.installer(), &root, major, &task.monitor, &task.cancel).await?)
    }

    /// Waits for a Java download started elsewhere.
    async fn wait_for_java(&self, task: &InstallTask, major: u32) -> Result<java::JavaInstall> {
        while !task.is_finished() {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if let Some(Err(e)) = &*task.outcome.lock().unwrap() {
            return Err(CoreError::Java(e.clone()));
        }
        let all = java::discover_in(runtime::executables(&self.dirs.java_dir())).await;
        java::pick_compatible(&all, major, Platform::current())
            .cloned()
            .ok_or(CoreError::Java(format!("Java {major} was downloaded but does not run")))
    }

    /// Fetches the version's patched Log4j configuration if it is missing
    /// (versions installed before Sulfur downloaded it). Best effort.
    async fn ensure_log_config(&self, dir: &GameDir, version: &sulfur_minecraft::version::VersionJson) {
        let Some(file) = version.logging.as_ref().and_then(|l| l.client.as_ref()).map(|c| &c.file) else { return };
        if dir.log_config(&file.id).is_file() || version.log4j_version().is_none_or(|v| v >= (2, 17)) {
            return;
        }
        let task = sulfur_minecraft::install::log_config_task(dir, file);
        let downloader = self.downloader.read().unwrap().clone();
        let report = downloader.run(vec![task], Arc::new(Progress::default()), CancellationToken::new()).await;
        if let Err(e) = report.into_result() {
            tracing::warn!(error = %e, "could not fetch the Log4j configuration");
        }
    }

    fn game_dir(&self) -> GameDir {
        let settings = self.settings.read().unwrap();
        GameDir::new(settings.game.game_dir.clone().unwrap_or_else(|| self.dirs.default_game_dir()))
    }

    fn installer(&self) -> Installer {
        let settings = self.settings.read().unwrap();
        let mut reuse: Vec<GameDir> = settings.game.reuse_dirs.iter().map(GameDir::new).collect();
        if settings.game.reuse_official_dir {
            reuse.extend(layout::official_game_dir());
        }
        drop(settings);
        Installer::new(self.downloader.read().unwrap().clone(), self.game_dir(), Platform::current()).reuse_from(reuse)
    }
}

fn build_downloader(settings: &sulfur_core::Settings) -> Result<Downloader> {
    let config = DownloadConfig {
        max_connections: settings.download.max_connections as usize,
        max_segments_per_file: settings.download.segments_per_file as usize,
        ..DownloadConfig::default()
    };
    Ok(Downloader::new(config, sources::mirror_set(settings.download.source))?)
}

fn java_record(j: java::JavaInstall, managed_root: &std::path::Path) -> JavaRuntime {
    JavaRuntime {
        managed: j.path.starts_with(managed_root),
        path: path_string(j.path),
        version: j.version,
        major: j.major,
        arch: j.arch,
        vendor: j.vendor,
    }
}

/// Logs to `<data>/logs/sulfur.log` (truncated per start); `SULFUR_LOG` sets the filter.
fn init_logging(dirs: &AppDirs) {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let Ok(file) = std::fs::File::create(dirs.logs.join("sulfur.log")) else { return };
        let filter = std::env::var("SULFUR_LOG").unwrap_or_else(|_| "info".into());
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .try_init();
    });
}
