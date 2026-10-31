//! Records and enums exchanged with Kotlin, and their mapping to core types.
//! Core crates stay free of UniFFI; this module is the only translation layer.

use std::path::PathBuf;

use sulfur_core as core;
use sulfur_minecraft::{groups, loader, manifest, omniarchive};

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum DownloadSource {
    /// Official servers first, BMCLAPI mirror as fallback.
    Auto,
    Official,
    /// BMCLAPI mirror first.
    Mirror,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum NavigationPosition {
    Top,
    Side,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Isolation {
    Isolated,
    Shared,
}

/// User settings, flattened for the UI. Per-version isolation overrides are
/// managed separately through `SulfurCore::set_version_isolation`.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Settings {
    pub theme: ThemeMode,
    pub seed_color: u32,
    pub high_contrast: bool,
    pub navigation: NavigationPosition,
    /// Picture behind the pages; `None` for none.
    pub background_image: Option<String>,
    /// `0.0..=1.0`.
    pub background_opacity: f32,
    /// Blur radius in dp.
    pub background_blur: f32,
    /// UI language: empty follows the system, else `en`, `zh-CN` or `zh-TW`.
    pub language: String,
    pub download_source: DownloadSource,
    pub max_connections: u32,
    pub segments_per_file: u32,
    pub game_dir: Option<String>,
    pub isolation: Isolation,
    pub reuse_official_dir: bool,
    pub reuse_dirs: Vec<String>,
    pub java_path: Option<String>,
    pub max_memory_mib: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub jvm_args: String,
    pub selected_version: Option<String>,
    /// Download a matching Java when none is installed.
    pub auto_java: bool,
    /// Patch Log4Shell in versions with a vulnerable Log4j.
    pub log4j_fix: bool,
    pub offline_name: String,
    /// The account to play with: a Microsoft account's id, or empty for offline.
    pub selected_account: String,
    /// Official CurseForge API key; empty uses the MCIM mirror.
    pub curseforge_api_key: String,
    /// Download Cleanroom without its download page.
    pub cleanroom_direct: bool,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    Other,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct RemoteVersion {
    pub id: String,
    pub kind: VersionKind,
    /// RFC 3339 timestamp.
    pub release_time: String,
    pub installed: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct VersionList {
    pub latest_release: String,
    pub latest_snapshot: String,
    pub versions: Vec<RemoteVersion>,
    /// The same versions grouped into major updates, newest first.
    pub groups: Vec<VersionGroup>,
    /// True when the list came from the on-disk cache because the network failed.
    pub from_cache: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum GroupKind {
    /// Snapshots newer than the latest release.
    Upcoming,
    Release,
    Beta,
    Alpha,
    /// Mojang's April Fools versions.
    AprilFools,
}

/// A major update (`1.21`, `26.3`) with its releases and previews.
#[derive(Clone, Debug, uniffi::Record)]
pub struct VersionGroup {
    pub key: String,
    pub kind: GroupKind,
    /// Official name, e.g. "Tricky Trials", where known.
    pub name: Option<String>,
    /// Newest first.
    pub releases: Vec<RemoteVersion>,
    /// Snapshots, pre-releases and release candidates, newest first.
    pub previews: Vec<RemoteVersion>,
}

/// Artwork for a [`VersionGroup`].
#[derive(Clone, Debug, uniffi::Record)]
pub struct Banner {
    pub key: String,
    pub image_url: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum LoaderKind {
    Fabric,
    Forge,
    NeoForge,
    OptiFine,
    LegacyFabric,
    Ornithe,
    Quilt,
    Rift,
    Cleanroom,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct LoaderVersion {
    pub version: String,
    /// Recommended or stable build.
    pub stable: bool,
}

/// What to install: a vanilla version, optionally with a loader on top.
#[derive(Clone, Debug, uniffi::Record)]
pub struct InstallRequest {
    pub game_version: String,
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<String>,
    /// Id of the new version; defaults to `<game>-<loader>-<loader version>`.
    pub name: Option<String>,
    /// Also put Fabric API into the version's mods folder (Fabric only).
    pub fabric_api: bool,
    /// Cleanroom: the installer link the user picked on its download page.
    pub installer_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ModSource {
    Modrinth,
    CurseForge,
}

/// What the Mods section browses, and the folder it installs into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ProjectKind {
    Mod,
    ResourcePack,
    /// Installed into one world's `datapacks` folder.
    DataPack,
    Shader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ModSort {
    Relevance,
    Downloads,
    Updated,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ModQuery {
    /// One source, or several whose results are merged.
    pub sources: Vec<ModSource>,
    pub kind: ProjectKind,
    pub text: String,
    /// Installed version whose game version and loader filter the results.
    pub version_id: Option<String>,
    pub sort: ModSort,
    pub offset: u32,
}

/// A project on one source.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ModRef {
    pub source: ModSource,
    pub id: String,
    pub page_url: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ModHit {
    pub source: ModSource,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub page_url: String,
    /// The same project on other sources.
    pub also: Vec<ModRef>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ModPage {
    pub hits: Vec<ModHit>,
    pub total: u64,
}

/// One release of a Modrinth modpack.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ModpackVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    /// Modrinth loader names, e.g. `fabric`, `neoforge`.
    pub loaders: Vec<String>,
    /// RFC 3339 date.
    pub published: String,
    pub stable: bool,
    pub file_name: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// What a `.mrpack` file on disk contains.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ModpackInfo {
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub game_version: String,
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<String>,
    /// Files to download for the client.
    pub files: u32,
}

/// A jar in a version's mods folder.
#[derive(Clone, Debug, uniffi::Record)]
pub struct InstalledMod {
    pub file_name: String,
    /// False for `.jar.disabled` files.
    pub enabled: bool,
    pub size: u64,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct InstalledVersion {
    pub id: String,
    pub kind: String,
    pub java_major: u32,
    pub isolation: Isolation,
    /// Whether `isolation` comes from a per-version override.
    pub isolation_overridden: bool,
    /// Where saves, mods and options of this version live.
    pub run_dir: String,
    pub loader: Option<LoaderKind>,
    /// The vanilla version this one is based on (its own id for vanilla).
    pub game_version: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct JavaRuntime {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub arch: String,
    pub vendor: String,
    /// Downloaded by Sulfur.
    pub managed: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CorePaths {
    pub config_dir: String,
    pub data_dir: String,
    pub logs_dir: String,
    pub game_dir: String,
    pub settings_file: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum InstallStage {
    Preparing,
    Downloading,
    Extracting,
    /// Installing a mod loader.
    Loader,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct InstallSnapshot {
    pub version_id: String,
    pub stage: InstallStage,
    pub files_total: u64,
    pub files_done: u64,
    pub files_skipped: u64,
    pub files_reused: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
    pub bytes_per_second: f64,
    /// Completion in `0.0..=1.0`.
    pub fraction: f64,
    pub error: Option<String>,
}

/// Whether offline accounts may be used here. They are allowed in mainland
/// China, where Java Edition is not sold; elsewhere they need a signed-in
/// Microsoft account that owns the game.
#[derive(Clone, Debug, uniffi::Record)]
pub struct AccountPolicy {
    pub offline_allowed: bool,
    pub mainland_china: bool,
    /// This build can sign in to Microsoft accounts (it has an application ID).
    pub microsoft_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AccountKind {
    Offline,
    Microsoft,
}

/// An account the game can be played with.
#[derive(Clone, Debug, uniffi::Record)]
pub struct AccountInfo {
    /// The Minecraft UUID for Microsoft accounts; empty for the offline account.
    pub id: String,
    pub kind: AccountKind,
    pub name: String,
    /// The player's skin texture (64×64 or 64×32 PNG), if any.
    pub skin_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SignInStage {
    /// Asking Microsoft for a code.
    Starting,
    /// The player has to enter `user_code` at `verification_uri`.
    WaitingForPlayer,
    Xbox,
    Minecraft,
    Profile,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SignInSnapshot {
    pub stage: SignInStage,
    pub user_code: Option<String>,
    pub verification_uri: Option<String>,
    /// The signed-in account once `stage` is `Done`.
    pub account: Option<AccountInfo>,
    pub error: Option<String>,
}

/// A client jar preserved by Omniarchive that Mojang no longer offers.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ArchivedVersion {
    /// The jar name without `.jar`, used as the version id.
    pub id: String,
    pub file_name: String,
    /// Archive folder: `preclassic`, `classic`, `indev`, `infdev`, `alpha`,
    /// `beta`, `release` or `april-fools`.
    pub category: String,
    /// As the archive shows it, e.g. `1.4 MiB`.
    pub size: String,
    pub installed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ForeignLauncher {
    Hmcl,
    PclCe,
    Pcl2,
}

/// Settings found in another launcher's folder; `None` where it had none.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ForeignConfig {
    pub launcher: ForeignLauncher,
    /// The folder its settings were found in.
    pub location: String,
    /// Its game folders, the selected one first.
    pub game_dirs: Vec<String>,
    pub selected_version: Option<String>,
    pub max_memory_mib: Option<u32>,
    pub jvm_args: Option<String>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub java_path: Option<String>,
    pub download_source: Option<DownloadSource>,
    pub max_connections: Option<u32>,
    pub isolation: Option<Isolation>,
    pub offline_name: Option<String>,
    pub background_image: Option<String>,
    pub language: Option<String>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct LogChunk {
    /// Line number to pass to the next call.
    pub next: u64,
    pub lines: Vec<String>,
}

pub(crate) fn path_string(path: impl Into<PathBuf>) -> String {
    path.into().to_string_lossy().into_owned()
}

fn non_empty(s: Option<String>) -> Option<PathBuf> {
    s.filter(|s| !s.trim().is_empty()).map(PathBuf::from)
}

impl From<core::Isolation> for Isolation {
    fn from(i: core::Isolation) -> Self {
        match i {
            core::Isolation::Isolated => Self::Isolated,
            core::Isolation::Shared => Self::Shared,
        }
    }
}

impl From<Isolation> for core::Isolation {
    fn from(i: Isolation) -> Self {
        match i {
            Isolation::Isolated => Self::Isolated,
            Isolation::Shared => Self::Shared,
        }
    }
}

impl From<manifest::VersionKind> for VersionKind {
    fn from(k: manifest::VersionKind) -> Self {
        match k {
            manifest::VersionKind::Release => Self::Release,
            manifest::VersionKind::Snapshot => Self::Snapshot,
            manifest::VersionKind::OldBeta => Self::OldBeta,
            manifest::VersionKind::OldAlpha => Self::OldAlpha,
            manifest::VersionKind::Other => Self::Other,
        }
    }
}

impl From<&core::Settings> for Settings {
    fn from(s: &core::Settings) -> Self {
        Self {
            theme: match s.appearance.theme {
                core::ThemeMode::System => ThemeMode::System,
                core::ThemeMode::Light => ThemeMode::Light,
                core::ThemeMode::Dark => ThemeMode::Dark,
            },
            seed_color: s.appearance.seed_color,
            high_contrast: s.appearance.high_contrast,
            navigation: match s.appearance.navigation {
                core::NavigationPosition::Top => NavigationPosition::Top,
                core::NavigationPosition::Side => NavigationPosition::Side,
            },
            background_image: s.appearance.background_image.clone().map(path_string),
            background_opacity: s.appearance.background_opacity,
            background_blur: s.appearance.background_blur,
            language: s.appearance.language.clone(),
            download_source: download_source_out(s.download.source),
            max_connections: s.download.max_connections,
            segments_per_file: s.download.segments_per_file,
            game_dir: s.game.game_dir.clone().map(path_string),
            isolation: s.game.isolation.into(),
            reuse_official_dir: s.game.reuse_official_dir,
            reuse_dirs: s.game.reuse_dirs.iter().cloned().map(path_string).collect(),
            java_path: s.game.java_path.clone().map(path_string),
            max_memory_mib: s.game.max_memory_mib,
            window_width: s.game.window_width,
            window_height: s.game.window_height,
            fullscreen: s.game.fullscreen,
            jvm_args: s.game.jvm_args.clone(),
            selected_version: s.game.selected_version.clone(),
            auto_java: s.game.auto_java,
            log4j_fix: s.game.log4j_fix,
            offline_name: s.account.offline_name.clone(),
            selected_account: s.account.selected.clone(),
            curseforge_api_key: s.mods.curseforge_api_key.clone(),
            cleanroom_direct: s.mods.cleanroom_direct,
        }
    }
}

impl Settings {
    /// Writes these values onto `target`, keeping fields the UI does not see.
    pub(crate) fn apply_to(self, target: &mut core::Settings) {
        target.appearance.theme = match self.theme {
            ThemeMode::System => core::ThemeMode::System,
            ThemeMode::Light => core::ThemeMode::Light,
            ThemeMode::Dark => core::ThemeMode::Dark,
        };
        target.appearance.seed_color = self.seed_color & 0xFF_FFFF;
        target.appearance.high_contrast = self.high_contrast;
        target.appearance.navigation = match self.navigation {
            NavigationPosition::Top => core::NavigationPosition::Top,
            NavigationPosition::Side => core::NavigationPosition::Side,
        };
        target.appearance.background_image = non_empty(self.background_image);
        target.appearance.background_opacity = self.background_opacity.clamp(0.0, 1.0);
        target.appearance.background_blur = self.background_blur.clamp(0.0, 64.0);
        target.appearance.language = match self.language.as_str() {
            "en" | "zh-CN" | "zh-TW" => self.language,
            _ => String::new(),
        };
        target.download.source = download_source_in(self.download_source);
        target.download.max_connections = self.max_connections.clamp(1, 512);
        target.download.segments_per_file = self.segments_per_file.clamp(1, 32);
        target.game.game_dir = non_empty(self.game_dir);
        target.game.isolation = self.isolation.into();
        target.game.reuse_official_dir = self.reuse_official_dir;
        target.game.reuse_dirs = self.reuse_dirs.into_iter().filter_map(|d| non_empty(Some(d))).collect();
        target.game.java_path = non_empty(self.java_path);
        target.game.max_memory_mib = self.max_memory_mib.max(512);
        target.game.window_width = self.window_width.max(320);
        target.game.window_height = self.window_height.max(240);
        target.game.fullscreen = self.fullscreen;
        target.game.jvm_args = self.jvm_args;
        target.game.selected_version = self.selected_version.filter(|v| !v.is_empty());
        target.game.auto_java = self.auto_java;
        target.game.log4j_fix = self.log4j_fix;
        let name = self.offline_name.trim();
        target.account.offline_name = if name.is_empty() { "Player".into() } else { name.to_owned() };
        target.account.selected = self.selected_account;
        target.mods.curseforge_api_key = self.curseforge_api_key.trim().to_owned();
        target.mods.cleanroom_direct = self.cleanroom_direct;
    }
}

impl From<groups::GroupKind> for GroupKind {
    fn from(k: groups::GroupKind) -> Self {
        match k {
            groups::GroupKind::Upcoming => Self::Upcoming,
            groups::GroupKind::Release => Self::Release,
            groups::GroupKind::Beta => Self::Beta,
            groups::GroupKind::Alpha => Self::Alpha,
            groups::GroupKind::AprilFools => Self::AprilFools,
        }
    }
}

impl From<loader::LoaderKind> for LoaderKind {
    fn from(k: loader::LoaderKind) -> Self {
        match k {
            loader::LoaderKind::Fabric => Self::Fabric,
            loader::LoaderKind::Forge => Self::Forge,
            loader::LoaderKind::NeoForge => Self::NeoForge,
            loader::LoaderKind::OptiFine => Self::OptiFine,
            loader::LoaderKind::LegacyFabric => Self::LegacyFabric,
            loader::LoaderKind::Ornithe => Self::Ornithe,
            loader::LoaderKind::Quilt => Self::Quilt,
            loader::LoaderKind::Rift => Self::Rift,
            loader::LoaderKind::Cleanroom => Self::Cleanroom,
        }
    }
}

impl From<LoaderKind> for loader::LoaderKind {
    fn from(k: LoaderKind) -> Self {
        match k {
            LoaderKind::Fabric => Self::Fabric,
            LoaderKind::Forge => Self::Forge,
            LoaderKind::NeoForge => Self::NeoForge,
            LoaderKind::OptiFine => Self::OptiFine,
            LoaderKind::LegacyFabric => Self::LegacyFabric,
            LoaderKind::Ornithe => Self::Ornithe,
            LoaderKind::Quilt => Self::Quilt,
            LoaderKind::Rift => Self::Rift,
            LoaderKind::Cleanroom => Self::Cleanroom,
        }
    }
}

impl LoaderKind {
    /// The loader mods are made for; OptiFine is not one.
    pub(crate) fn mod_loader(self) -> Option<sulfur_mods::Loader> {
        match self {
            Self::Fabric => Some(sulfur_mods::Loader::Fabric),
            Self::Forge => Some(sulfur_mods::Loader::Forge),
            Self::NeoForge => Some(sulfur_mods::Loader::NeoForge),
            // Cleanroom runs Forge 1.12.2 mods.
            Self::Cleanroom => Some(sulfur_mods::Loader::Forge),
            Self::LegacyFabric => Some(sulfur_mods::Loader::LegacyFabric),
            Self::Ornithe => Some(sulfur_mods::Loader::Ornithe),
            Self::Quilt => Some(sulfur_mods::Loader::Quilt),
            Self::Rift => Some(sulfur_mods::Loader::Rift),
            Self::OptiFine => None,
        }
    }
}

impl From<ModSource> for sulfur_mods::Source {
    fn from(s: ModSource) -> Self {
        match s {
            ModSource::Modrinth => Self::Modrinth,
            ModSource::CurseForge => Self::CurseForge,
        }
    }
}

impl From<sulfur_mods::Source> for ModSource {
    fn from(s: sulfur_mods::Source) -> Self {
        match s {
            sulfur_mods::Source::Modrinth => Self::Modrinth,
            sulfur_mods::Source::CurseForge => Self::CurseForge,
        }
    }
}

impl From<ProjectKind> for sulfur_mods::ProjectKind {
    fn from(k: ProjectKind) -> Self {
        match k {
            ProjectKind::Mod => Self::Mod,
            ProjectKind::ResourcePack => Self::ResourcePack,
            ProjectKind::DataPack => Self::DataPack,
            ProjectKind::Shader => Self::Shader,
        }
    }
}

impl From<sulfur_mods::ProjectRef> for ModRef {
    fn from(r: sulfur_mods::ProjectRef) -> Self {
        Self {
            source: r.source.into(),
            id: r.id,
            page_url: r.page_url,
        }
    }
}

impl From<ModRef> for sulfur_mods::ProjectRef {
    fn from(r: ModRef) -> Self {
        Self {
            source: r.source.into(),
            id: r.id,
            page_url: r.page_url,
        }
    }
}

impl From<sulfur_mods::ModHit> for ModHit {
    fn from(h: sulfur_mods::ModHit) -> Self {
        Self {
            source: h.source.into(),
            id: h.id,
            slug: h.slug,
            title: h.title,
            author: h.author,
            description: h.description,
            icon_url: h.icon_url,
            downloads: h.downloads,
            categories: h.categories,
            page_url: h.page_url,
            also: h.also.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ModHit> for sulfur_mods::ModHit {
    fn from(h: ModHit) -> Self {
        Self {
            source: h.source.into(),
            id: h.id,
            slug: h.slug,
            title: h.title,
            author: h.author,
            description: h.description,
            icon_url: h.icon_url,
            downloads: h.downloads,
            categories: h.categories,
            page_url: h.page_url,
            also: h.also.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<sulfur_mods::PackVersion> for ModpackVersion {
    fn from(v: sulfur_mods::PackVersion) -> Self {
        Self {
            id: v.id,
            name: v.name,
            version_number: v.version_number,
            game_versions: v.game_versions,
            loaders: v.loaders,
            published: v.published,
            stable: v.stable,
            file_name: v.file.file_name,
            url: v.file.url,
            sha1: v.file.sha1,
            size: v.file.size,
        }
    }
}

impl From<sulfur_mods::mrpack::PackLoader> for LoaderKind {
    fn from(l: sulfur_mods::mrpack::PackLoader) -> Self {
        use sulfur_mods::mrpack::PackLoader;
        match l {
            PackLoader::Forge => Self::Forge,
            PackLoader::NeoForge => Self::NeoForge,
            PackLoader::Fabric => Self::Fabric,
            PackLoader::Quilt => Self::Quilt,
        }
    }
}

impl From<ModSort> for sulfur_mods::Sort {
    fn from(s: ModSort) -> Self {
        match s {
            ModSort::Relevance => Self::Relevance,
            ModSort::Downloads => Self::Downloads,
            ModSort::Updated => Self::Updated,
        }
    }
}

impl From<&omniarchive::ArchivedJar> for ArchivedVersion {
    fn from(j: &omniarchive::ArchivedJar) -> Self {
        Self {
            id: j.id.clone(),
            file_name: j.file_name.clone(),
            category: j.category.clone(),
            size: j.size.clone(),
            installed: false,
        }
    }
}

fn download_source_out(s: core::DownloadSource) -> DownloadSource {
    match s {
        core::DownloadSource::Auto => DownloadSource::Auto,
        core::DownloadSource::Official => DownloadSource::Official,
        core::DownloadSource::Mirror => DownloadSource::Mirror,
    }
}

fn download_source_in(s: DownloadSource) -> core::DownloadSource {
    match s {
        DownloadSource::Auto => core::DownloadSource::Auto,
        DownloadSource::Official => core::DownloadSource::Official,
        DownloadSource::Mirror => core::DownloadSource::Mirror,
    }
}

impl From<core::import::ForeignConfig> for ForeignConfig {
    fn from(c: core::import::ForeignConfig) -> Self {
        Self {
            launcher: match c.launcher {
                core::import::ForeignLauncher::Hmcl => ForeignLauncher::Hmcl,
                core::import::ForeignLauncher::PclCe => ForeignLauncher::PclCe,
                core::import::ForeignLauncher::Pcl2 => ForeignLauncher::Pcl2,
            },
            location: path_string(c.location),
            game_dirs: c.game_dirs.into_iter().map(path_string).collect(),
            selected_version: c.selected_version,
            max_memory_mib: c.max_memory_mib,
            jvm_args: c.jvm_args,
            window_width: c.window.map(|w| w.0),
            window_height: c.window.map(|w| w.1),
            fullscreen: c.fullscreen,
            java_path: c.java_path.map(path_string),
            download_source: c.download_source.map(download_source_out),
            max_connections: c.max_connections,
            isolation: c.isolation.map(Into::into),
            offline_name: c.offline_name,
            background_image: c.background_image.map(path_string),
            language: c.language,
        }
    }
}

impl From<ForeignConfig> for core::import::ForeignConfig {
    fn from(c: ForeignConfig) -> Self {
        Self {
            launcher: match c.launcher {
                ForeignLauncher::Hmcl => core::import::ForeignLauncher::Hmcl,
                ForeignLauncher::PclCe => core::import::ForeignLauncher::PclCe,
                ForeignLauncher::Pcl2 => core::import::ForeignLauncher::Pcl2,
            },
            location: c.location.into(),
            game_dirs: c.game_dirs.into_iter().map(PathBuf::from).collect(),
            selected_version: c.selected_version,
            max_memory_mib: c.max_memory_mib,
            jvm_args: c.jvm_args,
            window: c.window_width.zip(c.window_height),
            fullscreen: c.fullscreen,
            java_path: c.java_path.map(PathBuf::from),
            download_source: c.download_source.map(download_source_in),
            max_connections: c.max_connections,
            isolation: c.isolation.map(Into::into),
            offline_name: c.offline_name,
            background_image: c.background_image.map(PathBuf::from),
            language: c.language,
        }
    }
}
