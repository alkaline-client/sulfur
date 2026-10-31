//! Mod loaders and OptiFine: listing their versions and installing them as
//! versions that inherit from a vanilla one.
//!
//! * Fabric, Legacy Fabric, Ornithe and Quilt publish ready-made version JSON ([`fabric`]).
//! * Forge, NeoForge and Cleanroom ship installers whose processors patch the
//!   game locally ([`forge`]).
//! * OptiFine patches the client jar with its own tool ([`optifine`]).
//! * Rift jars carry their own version JSON ([`rift`]).

mod fabric;
mod forge;
mod optifine;
mod rift;

pub use forge::{CLEANROOM_GAME_VERSION, cleanroom_installer_version, is_cleanroom_installer_url};

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use sulfur_download::{CancellationToken, DownloadError, Downloader};

use crate::{
    McError, Result,
    install::{InstallMonitor, InstallStage, Installer},
    version::VersionJson,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoaderKind {
    Fabric,
    Forge,
    NeoForge,
    OptiFine,
    /// Fabric for Minecraft 1.3–1.13.2.
    LegacyFabric,
    /// Fabric for old versions with Ornithe's intermediary mappings.
    Ornithe,
    Quilt,
    /// Rift, 1.13.2 only.
    Rift,
    /// Cleanroom, a Forge fork for 1.12.2 on modern Java.
    Cleanroom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderVersion {
    pub version: String,
    /// Recommended or stable build (not a beta, preview or plain "latest").
    pub stable: bool,
}

/// A loader to install on top of an installed vanilla version.
#[derive(Clone, Debug)]
pub struct LoaderRequest {
    pub kind: LoaderKind,
    /// The vanilla version, which must already be installed.
    pub game_version: String,
    pub loader_version: String,
    /// Id of the new version.
    pub name: String,
    /// Where to get the installer (Cleanroom): a link the user picked on the
    /// project's download page. Without it, the installer is built from the
    /// project's repository.
    pub installer_url: Option<String>,
}

/// Versions of `kind` available for `game_version`, newest first; empty
/// when the loader does not support that game version.
pub async fn versions(
    downloader: &Downloader,
    kind: LoaderKind,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    let found = match kind {
        LoaderKind::Fabric | LoaderKind::LegacyFabric | LoaderKind::Ornithe | LoaderKind::Quilt => {
            fabric::versions(downloader, kind, game_version, cancel).await
        }
        LoaderKind::Forge => forge::forge_versions(downloader, game_version, cancel).await,
        LoaderKind::NeoForge => forge::neoforge_versions(downloader, game_version, cancel).await,
        LoaderKind::Cleanroom => forge::cleanroom_versions(downloader, game_version, cancel).await,
        LoaderKind::OptiFine => optifine::versions(downloader, game_version, cancel).await,
        LoaderKind::Rift => rift::versions(downloader, game_version, cancel).await,
    };
    match found {
        // Fabric's, Quilt's and Legacy Fabric's meta servers answer 400 or 404
        // for game versions they do not know.
        Err(McError::Download(DownloadError::Status { status: 400 | 404, .. })) => Ok(Vec::new()),
        other => other,
    }
}

/// Which loader an installed (resolved) version uses.
pub fn detect(version: &VersionJson) -> Option<LoaderKind> {
    let has = |prefix: &str| version.libraries.iter().any(|l| l.name.starts_with(prefix));
    if has("com.cleanroommc:cleanroom:") {
        Some(LoaderKind::Cleanroom)
    } else if has("org.quiltmc:quilt-loader:") {
        Some(LoaderKind::Quilt)
    } else if has("net.fabricmc:fabric-loader:") {
        if has("net.legacyfabric") {
            Some(LoaderKind::LegacyFabric)
        } else if has("net.ornithemc:") {
            Some(LoaderKind::Ornithe)
        } else {
            Some(LoaderKind::Fabric)
        }
    } else if has("org.dimdev:rift:") {
        Some(LoaderKind::Rift)
    } else if has("net.neoforged") {
        Some(LoaderKind::NeoForge)
    } else if has("net.minecraftforge:forge:") || has("net.minecraftforge:fmlloader:") {
        Some(LoaderKind::Forge)
    } else if has("optifine:OptiFine:") {
        Some(LoaderKind::OptiFine)
    } else {
        None
    }
}

impl Installer {
    /// Installs a loader as version `request.name`. `java` runs installer
    /// tools; `work_dir` keeps downloaded installers.
    pub async fn install_loader(
        &self,
        request: &LoaderRequest,
        java: &Path,
        work_dir: &Path,
        monitor: &InstallMonitor,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if !self.dir().version_json(&request.game_version).is_file() {
            return Err(McError::NotInstalled(request.game_version.clone()));
        }
        if request.name.trim().is_empty() || request.name.contains(['/', '\\', ':']) || request.name == ".." {
            return Err(McError::Invalid(format!("{:?} is not a valid version name", request.name)));
        }
        monitor.set_stage(InstallStage::Loader);
        let ctx = Context {
            installer: self,
            java,
            work_dir,
            monitor,
            cancel,
        };
        match request.kind {
            LoaderKind::Fabric | LoaderKind::LegacyFabric | LoaderKind::Ornithe | LoaderKind::Quilt => {
                fabric::install(&ctx, request).await
            }
            LoaderKind::Forge | LoaderKind::NeoForge | LoaderKind::Cleanroom => forge::install(&ctx, request).await,
            LoaderKind::OptiFine => optifine::install(&ctx, request).await,
            LoaderKind::Rift => rift::install(&ctx, request).await,
        }?;
        Ok(())
    }
}

/// What every loader installer needs.
struct Context<'a> {
    installer: &'a Installer,
    java: &'a Path,
    work_dir: &'a Path,
    monitor: &'a InstallMonitor,
    cancel: &'a CancellationToken,
}

impl Context<'_> {
    fn downloader(&self) -> &Downloader {
        self.installer.downloader()
    }

    /// Writes the final version JSON with `id` set to the requested name.
    /// Done last, so an interrupted install never shows up as installed.
    fn write_version(&self, name: &str, mut json: serde_json::Value) -> Result<()> {
        json["id"] = serde_json::Value::String(name.to_owned());
        let path = self.installer.dir().version_json(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| McError::io(parent, e))?;
        }
        let text = serde_json::to_vec_pretty(&json).map_err(|source| McError::Json { path: path.clone(), source })?;
        std::fs::write(&path, text).map_err(|e| McError::io(&path, e))
    }

    /// Runs `java -cp <classpath> <main> <args>` and fails with its last output lines.
    async fn run_java(&self, classpath: &[PathBuf], main: &str, args: &[String], what: &str) -> Result<()> {
        let sep = self.installer.platform().classpath_separator();
        let cp = classpath.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join(sep);
        let mut command = tokio::process::Command::new(self.java);
        command.arg("-cp").arg(cp).arg(main).args(args).current_dir(self.work_dir);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        tracing::info!(main, "running {what}");
        let output = tokio::select! {
            out = command.kill_on_drop(true).output() => out.map_err(McError::Spawn)?,
            _ = self.cancel.cancelled() => return Err(sulfur_download::DownloadError::Cancelled.into()),
        };
        if output.status.success() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&output.stderr).into_owned() + &String::from_utf8_lossy(&output.stdout);
        let tail: Vec<&str> = text.lines().rev().take(6).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        Err(McError::Invalid(format!("{what} failed ({}): {}", output.status, tail.join(" | "))))
    }
}

pub(crate) fn read_zip_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, path: &Path) -> Result<Vec<u8>> {
    let mut entry = zip
        .by_name(name.trim_start_matches('/'))
        .map_err(|_| McError::Invalid(format!("{} has no {name}", path.display())))?;
    let mut out = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut out).map_err(|e| McError::io(path, e))?;
    Ok(out)
}

pub(crate) fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|e| McError::io(path, e))?;
    zip::ZipArchive::new(file).map_err(|e| McError::io(path, e.into()))
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| McError::io(parent, e))?;
    }
    std::fs::write(path, bytes).map_err(|e| McError::io(path, e))
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    downloader: &Downloader,
    url: &str,
    cancel: &CancellationToken,
) -> Result<T> {
    let bytes = downloader.fetch(url, cancel).await?;
    serde_json::from_slice(&bytes).map_err(|source| McError::Json { path: url.into(), source })
}

/// `<version>` values of a Maven `maven-metadata.xml`.
fn maven_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split_once("</version>").map(|(v, _)| v.trim().to_owned()))
        .collect()
}

/// Orders versions like `1.21-51.0.33` or `21.1.65-beta` by their numbers.
fn version_order(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven_metadata() {
        let xml = "<metadata><versioning><versions><version>1.0</version>\n<version> 2.0 </version></versions></versioning></metadata>";
        assert_eq!(maven_versions(xml), ["1.0", "2.0"]);
        let mut v = vec!["1.21-51.0.9", "1.21-51.0.33", "1.21-51.0.10"];
        v.sort_by_key(|x| std::cmp::Reverse(version_order(x)));
        assert_eq!(v, ["1.21-51.0.33", "1.21-51.0.10", "1.21-51.0.9"]);
    }

    #[test]
    fn detects_loaders() {
        let json = |lib: &str| -> VersionJson {
            serde_json::from_str(&format!(r#"{{"id":"x","libraries":[{{"name":"{lib}"}}]}}"#)).unwrap()
        };
        assert_eq!(detect(&json("net.fabricmc:fabric-loader:0.17.3")), Some(LoaderKind::Fabric));
        assert_eq!(detect(&json("net.neoforged.fancymodloader:loader:4.0.1")), Some(LoaderKind::NeoForge));
        assert_eq!(detect(&json("net.minecraftforge:fmlloader:1.20.1-47.3.0")), Some(LoaderKind::Forge));
        assert_eq!(detect(&json("optifine:OptiFine:1.21.1_HD_U_J1")), Some(LoaderKind::OptiFine));
        assert_eq!(detect(&json("org.quiltmc:quilt-loader:0.26.0")), Some(LoaderKind::Quilt));
        assert_eq!(detect(&json("com.cleanroommc:cleanroom:0.7.14-alpha")), Some(LoaderKind::Cleanroom));
        assert_eq!(detect(&json("org.dimdev:rift:1.0.4-106")), Some(LoaderKind::Rift));
        let two = |a: &str, b: &str| -> VersionJson {
            serde_json::from_str(&format!(r#"{{"id":"x","libraries":[{{"name":"{a}"}},{{"name":"{b}"}}]}}"#)).unwrap()
        };
        let fabric = "net.fabricmc:fabric-loader:0.17.3";
        assert_eq!(detect(&two(fabric, "net.legacyfabric:intermediary:1.8.9")), Some(LoaderKind::LegacyFabric));
        assert_eq!(detect(&two(fabric, "net.ornithemc:calamus-intermediary-gen2:1.8.9")), Some(LoaderKind::Ornithe));
        assert_eq!(detect(&json("org.lwjgl:lwjgl:3.3.3")), None);
    }
}
