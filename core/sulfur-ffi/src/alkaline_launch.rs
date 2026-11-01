//! Playing Alkaline: it shows as an installed version, and Play downloads
//! its newest release when it is missing or older (or builds it when only
//! its source is there), fetches Java 25 and the 1.8.9 jar it reads its
//! assets from, and starts it.

use std::{sync::Arc, time::Duration};

use sulfur_download::CancellationToken;
use sulfur_minecraft::{alkaline, install::InstallStage, launch, platform::Platform, process::GameProcess};

use crate::{
    AlkalineRelease, CoreError, GameSession, InstallTask, Result, State, SulfurCore, TaskKind, install_job, pre_launch,
};

/// How long Play waits to hear about a newer release before starting the one installed.
const UPDATE_CHECK: Duration = Duration::from_secs(5);

#[uniffi::export]
impl SulfurCore {
    /// The newest Alkaline release from its feed (GitHub unless set), or
    /// `None` while there is none. Kept until `refresh`.
    pub async fn alkaline_release(&self, refresh: bool) -> Result<Option<AlkalineRelease>> {
        let state = self.state.clone();
        self.runtime
            .spawn(async move {
                let release = state.latest_alkaline(refresh).await?;
                Ok(release.map(|r| AlkalineRelease {
                    available: r.download.is_some(),
                    version: r.version,
                    notes: r.notes,
                    published: r.published,
                    page_url: r.page_url,
                }))
            })
            .await?
    }

    /// The Alkaline release Sulfur installed, if any.
    pub fn alkaline_installed(&self) -> Option<String> {
        alkaline::installed_release()
    }

    /// Downloads the newest Alkaline release (or returns the task already
    /// running for Alkaline). The task is keyed `Alkaline`.
    pub fn install_alkaline(&self) -> Arc<InstallTask> {
        let (task, started) = self.state.task(alkaline::VERSION_ID, TaskKind::Game);
        if started {
            let (state, job) = (self.state.clone(), task.clone());
            self.runtime.spawn(async move {
                let result = state.download_alkaline(&job, true).await;
                *job.outcome.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
            });
        }
        task
    }
}

impl State {
    pub(crate) async fn launch_alkaline(&self) -> Result<Arc<GameSession>> {
        let id = alkaline::VERSION_ID;
        let settings = self.settings.read().unwrap().clone();
        let explicit = settings.game.alkaline_dir.as_deref();
        let mut client = alkaline::locate(explicit);
        if client.is_none() && explicit.is_none() {
            // Nothing installed yet: the newest release.
            self.install_alkaline_now(true).await?;
            client = alkaline::locate(None);
        } else if client == Some(alkaline::Client::Installed(alkaline::release_dir())) && alkaline::installed_release().is_some() {
            // A release Sulfur installed: updated first when a newer one is out, else started as it is.
            if let Err(e) = self.install_alkaline_now(false).await {
                tracing::warn!(error = %e, "starting the installed Alkaline without updating it");
            }
        }
        let client = client.ok_or_else(|| {
            CoreError::NotFound("Alkaline was not found; choose its folder in Settings → Game".into())
        })?;
        let game = settings.game.launch_settings(id);
        let java = self.find_java(game.java_path.as_deref(), settings.game.auto_java, id, alkaline::JAVA_MAJOR).await?;
        if let alkaline::Client::Source(root) = &client
            && client.needs_build()
        {
            self.build_alkaline(root, &java.path).await?;
        }
        let minecraft_jar = self.vanilla_jar(alkaline::GAME_VERSION).await;

        let mut jvm_args = launch::gc_args(game.gc, java.major);
        jvm_args.extend(launch::split_args(&game.jvm_args));
        let options = alkaline::LaunchOptions {
            java: java.path.clone(),
            minecraft_jar,
            max_memory_mib: game.max_memory_mib,
            min_memory_mib: game.min_memory_mib,
            jvm_args,
            // Only Alkaline's own: the global game arguments are Minecraft's, which it does not take.
            game_args: settings
                .game
                .version_settings
                .get(id)
                .and_then(|o| o.game_args.as_deref())
                .map(launch::split_args)
                .unwrap_or_default(),
            server: Some(game.server_address.clone()).filter(|s| !s.is_empty()),
            wrapper: launch::split_args(&game.wrapper_command),
            env: launch::parse_env(&game.env_vars),
        };
        let command = alkaline::command(&client.home(), &options);
        if !game.pre_launch_command.trim().is_empty() {
            pre_launch(&game.pre_launch_command, id, &command.working_dir, &options.java, &options.env).await?;
        }
        let process = GameProcess::spawn(&command)?;
        Ok(Arc::new(GameSession { version_id: id.into(), process }))
    }

    /// Builds Alkaline from source as a task the UI lists, or waits for the
    /// build already running.
    async fn build_alkaline(&self, root: &std::path::Path, java: &std::path::Path) -> Result<()> {
        let (task, started) = self.task(alkaline::VERSION_ID, TaskKind::Build);
        if !started {
            return self.wait_for(&task).await;
        }
        task.monitor.set_stage(InstallStage::Building);
        let java_home = java.parent().and_then(std::path::Path::parent);
        let build = alkaline::build(root, java_home, |line| {
            let line = line.trim();
            if !line.is_empty() {
                *task.detail.lock().unwrap() = Some(line.to_owned());
            }
        });
        let result = tokio::select! {
            result = build => result.map_err(CoreError::from),
            _ = task.cancel.cancelled() => Err(CoreError::Other("the Alkaline build was cancelled".into())),
        };
        *task.outcome.lock().unwrap() = Some(result.as_ref().map(drop).map_err(|e| e.to_string()));
        result
    }

    /// The client jar of vanilla `version`, installed first if it is missing
    /// (as a task the UI lists). `None` if it cannot be had; the client then
    /// runs without vanilla assets.
    async fn vanilla_jar(&self, version: &str) -> Option<std::path::PathBuf> {
        let jar = self.game_dir().version_jar(version);
        if jar.is_file() {
            return Some(jar);
        }
        let installed = async {
            let entry = self.official_manifest().await?.find(version).cloned().ok_or_else(|| CoreError::NotFound(version.into()))?;
            let (task, started) = self.task(version, TaskKind::Game);
            if !started {
                return self.wait_for(&task).await;
            }
            let result = install_job(self, &entry, None, false, &task).await;
            *task.outcome.lock().unwrap() = Some(result.as_ref().map(drop).map_err(|e| e.to_string()));
            result
        };
        match installed.await {
            Ok(()) => Some(jar).filter(|j| j.is_file()),
            Err(e) => {
                tracing::warn!(error = %e, "Alkaline starts without the vanilla {version} jar");
                None
            }
        }
    }

    /// The newest release, from memory unless `refresh`.
    async fn latest_alkaline(&self, refresh: bool) -> Result<Option<alkaline::Release>> {
        if !refresh && let Some(release) = self.alkaline_release.lock().unwrap().clone() {
            return Ok(Some(release));
        }
        let feed = self.settings.read().unwrap().game.alkaline_feed.clone();
        let release =
            alkaline::latest_release(&self.installer(), feed.as_deref(), Platform::current(), &CancellationToken::new()).await?;
        *self.alkaline_release.lock().unwrap() = release.clone();
        Ok(release)
    }

    /// Installs the newest release unless it is the one installed. `required`
    /// fails when there is none for this system; otherwise that is fine.
    async fn download_alkaline(&self, task: &InstallTask, required: bool) -> Result<()> {
        task.monitor.set_stage(InstallStage::Preparing);
        let release = if required {
            self.latest_alkaline(true).await?
        } else {
            tokio::time::timeout(UPDATE_CHECK, self.latest_alkaline(true))
                .await
                .map_err(|_| CoreError::Network("no answer about Alkaline releases".into()))??
        };
        let release = match release {
            Some(r) if r.download.is_some() => r,
            _ if required => {
                return Err(CoreError::NotFound(format!(
                    "Alkaline has no release for {} yet",
                    alkaline::platform_key(Platform::current())
                )));
            }
            _ => return Ok(()),
        };
        if alkaline::installed_release().as_deref() == Some(release.version.as_str()) {
            return Ok(());
        }
        Ok(alkaline::install_release(&self.installer(), &release, &task.monitor, &task.cancel).await?)
    }

    /// [`Self::download_alkaline`] as a task the UI lists, for Play; waits for one already running.
    async fn install_alkaline_now(&self, required: bool) -> Result<()> {
        let (task, started) = self.task(alkaline::VERSION_ID, TaskKind::Game);
        if !started {
            return self.wait_for(&task).await;
        }
        let result = self.download_alkaline(&task, required).await;
        if task.monitor.progress.snapshot().files_total == 0 && (result.is_ok() || !required) {
            // Nothing was downloaded: already the newest, or no answer while offline. No task to show.
            self.installs.lock().unwrap().remove(alkaline::VERSION_ID);
        }
        *task.outcome.lock().unwrap() = Some(result.as_ref().map(drop).map_err(|e| e.to_string()));
        result
    }

    /// The running task keyed `id`, or a new one (`true`).
    fn task(&self, id: &str, kind: TaskKind) -> (Arc<InstallTask>, bool) {
        let mut installs = self.installs.lock().unwrap();
        if let Some(task) = installs.get(id).filter(|t| !t.is_finished()) {
            return (task.clone(), false);
        }
        let task = InstallTask::new(id.to_owned(), kind);
        installs.insert(id.to_owned(), task.clone());
        (task, true)
    }

    async fn wait_for(&self, task: &InstallTask) -> Result<()> {
        while !task.is_finished() {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        match &*task.outcome.lock().unwrap() {
            Some(Err(e)) => Err(CoreError::Other(e.clone())),
            _ => Ok(()),
        }
    }
}
