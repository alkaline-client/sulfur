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

/// How round the UI's corners are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CornerStyle {
    Square,
    Small,
    Default,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Isolation {
    Isolated,
    Shared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ColorSource {
    Seed,
    Wallpaper,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum GarbageCollector {
    Auto,
    G1,
    Zgc,
    Shenandoah,
    Parallel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum LaunchColumn {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum WidgetKind {
    News,
    Versions,
    Note,
    Links,
    Markdown,
}

/// A card on the Launch page.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LaunchWidget {
    pub kind: WidgetKind,
    pub title: String,
    pub content: String,
}

/// User settings, flattened for the UI. Per-version overrides are managed
/// separately through `SulfurCore::set_version_isolation` and
/// `SulfurCore::set_version_settings`.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Settings {
    pub theme: ThemeMode,
    pub seed_color: u32,
    /// Whether the seed is `seed_color` or sampled from a picture.
    pub color_source: ColorSource,
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
    /// One roundness for every corner, the window's included.
    pub corners: CornerStyle,
    /// Small touches on special days.
    pub seasonal: bool,
    pub download_source: DownloadSource,
    pub max_connections: u32,
    pub segments_per_file: u32,
    pub game_dir: Option<String>,
    pub isolation: Isolation,
    pub reuse_official_dir: bool,
    pub reuse_dirs: Vec<String>,
    pub java_path: Option<String>,
    pub max_memory_mib: u32,
    /// `-Xms`; 0 leaves it to the JVM.
    pub min_memory_mib: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub jvm_args: String,
    pub game_args: String,
    pub gc: GarbageCollector,
    /// A program the game runs through, e.g. `gamemoderun`.
    pub wrapper_command: String,
    /// A shell command run before the game starts.
    pub pre_launch_command: String,
    /// `NAME=value` lines.
    pub env_vars: String,
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
    /// Alkaline's client folder or source checkout; `None` looks in the usual places.
    pub alkaline_dir: Option<String>,
    /// The address of Alkaline's release feed; `None` uses its GitHub releases.
    pub alkaline_feed: Option<String>,
    /// Side of the Launch page the account and Play button sit on.
    pub launch_column: LaunchColumn,
    pub launch_widgets: Vec<LaunchWidget>,
}

/// One version's overrides; `None` follows the global setting.
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct VersionSettings {
    pub java_path: Option<String>,
    pub max_memory_mib: Option<u32>,
    pub min_memory_mib: Option<u32>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub jvm_args: Option<String>,
    pub game_args: Option<String>,
    pub gc: Option<GarbageCollector>,
    pub wrapper_command: Option<String>,
    pub pre_launch_command: Option<String>,
    pub env_vars: Option<String>,
    pub log4j_fix: Option<bool>,
    /// Join this server when the game starts.
    pub server_address: Option<String>,
}

/// The desktop's wallpaper: a picture the UI can decode, or a plain color.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct DesktopWallpaper {
    pub path: Option<String>,
    /// `0xRRGGBB`.
    pub color: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct NewsEntry {
    pub id: String,
    pub title: String,
    pub text: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub image_url: Option<String>,
    pub link: String,
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
    /// A small version of the image for the version grid, when there is one.
    pub preview_url: Option<String>,
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
    /// The Alkaline client rather than a version of the game.
    pub alkaline: bool,
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
    /// Building a client from source.
    Building,
    Done,
    Failed,
    Cancelled,
}

/// The newest release of Alkaline and what is installed.
#[derive(Clone, Debug, uniffi::Record)]
pub struct AlkalineRelease {
    pub version: String,
    /// Release notes, in Markdown.
    pub notes: String,
    /// `YYYY-MM-DD`, or empty.
    pub published: String,
    pub page_url: String,
    /// Whether the release has a download for this system.
    pub available: bool,
}

/// What a task installs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TaskKind {
    /// A game version, with its mod loader if any.
    Game,
    /// A modpack, installed as a version of its own.
    Modpack,
    /// A Java runtime; the task is keyed `Java <major>`.
    Java,
    /// A client built from source (Alkaline).
    Build,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct InstallSnapshot {
    pub version_id: String,
    pub kind: TaskKind,
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
    /// What the task is doing now, beyond its stage (a build's output).
    pub detail: Option<String>,
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
            color_source: match s.appearance.color_source {
                core::ColorSource::Seed => ColorSource::Seed,
                core::ColorSource::Wallpaper => ColorSource::Wallpaper,
                core::ColorSource::Background => ColorSource::Background,
            },
            high_contrast: s.appearance.high_contrast,
            navigation: match s.appearance.navigation {
                core::NavigationPosition::Top => NavigationPosition::Top,
                core::NavigationPosition::Side => NavigationPosition::Side,
            },
            background_image: s.appearance.background_image.clone().map(path_string),
            background_opacity: s.appearance.background_opacity,
            background_blur: s.appearance.background_blur,
            language: s.appearance.language.clone(),
            corners: match s.appearance.corners {
                core::CornerStyle::Square => CornerStyle::Square,
                core::CornerStyle::Small => CornerStyle::Small,
                core::CornerStyle::Default => CornerStyle::Default,
                core::CornerStyle::Round => CornerStyle::Round,
            },
            seasonal: s.appearance.seasonal,
            download_source: download_source_out(s.download.source),
            max_connections: s.download.max_connections,
            segments_per_file: s.download.segments_per_file,
            game_dir: s.game.game_dir.clone().map(path_string),
            isolation: s.game.isolation.into(),
            reuse_official_dir: s.game.reuse_official_dir,
            reuse_dirs: s.game.reuse_dirs.iter().cloned().map(path_string).collect(),
            java_path: s.game.java_path.clone().map(path_string),
            max_memory_mib: s.game.max_memory_mib,
            min_memory_mib: s.game.min_memory_mib,
            window_width: s.game.window_width,
            window_height: s.game.window_height,
            fullscreen: s.game.fullscreen,
            jvm_args: s.game.jvm_args.clone(),
            game_args: s.game.game_args.clone(),
            gc: s.game.gc.into(),
            wrapper_command: s.game.wrapper_command.clone(),
            pre_launch_command: s.game.pre_launch_command.clone(),
            env_vars: s.game.env_vars.clone(),
            selected_version: s.game.selected_version.clone(),
            auto_java: s.game.auto_java,
            log4j_fix: s.game.log4j_fix,
            offline_name: s.account.offline_name.clone(),
            selected_account: s.account.selected.clone(),
            curseforge_api_key: s.mods.curseforge_api_key.clone(),
            cleanroom_direct: s.mods.cleanroom_direct,
            alkaline_dir: s.game.alkaline_dir.clone().map(path_string),
            alkaline_feed: s.game.alkaline_feed.clone(),
            launch_column: match s.launch_page.column {
                core::LaunchColumn::Left => LaunchColumn::Left,
                core::LaunchColumn::Right => LaunchColumn::Right,
            },
            launch_widgets: s.launch_page.widgets.iter().map(Into::into).collect(),
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
        target.appearance.color_source = match self.color_source {
            ColorSource::Seed => core::ColorSource::Seed,
            ColorSource::Wallpaper => core::ColorSource::Wallpaper,
            ColorSource::Background => core::ColorSource::Background,
        };
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
        target.appearance.corners = match self.corners {
            CornerStyle::Square => core::CornerStyle::Square,
            CornerStyle::Small => core::CornerStyle::Small,
            CornerStyle::Default => core::CornerStyle::Default,
            CornerStyle::Round => core::CornerStyle::Round,
        };
        target.appearance.seasonal = self.seasonal;
        target.download.source = download_source_in(self.download_source);
        target.download.max_connections = self.max_connections.clamp(1, 512);
        target.download.segments_per_file = self.segments_per_file.clamp(1, 32);
        target.game.game_dir = non_empty(self.game_dir);
        target.game.isolation = self.isolation.into();
        target.game.reuse_official_dir = self.reuse_official_dir;
        target.game.reuse_dirs = self.reuse_dirs.into_iter().filter_map(|d| non_empty(Some(d))).collect();
        target.game.java_path = non_empty(self.java_path);
        target.game.max_memory_mib = self.max_memory_mib.max(512);
        target.game.min_memory_mib = self.min_memory_mib;
        target.game.window_width = self.window_width.max(320);
        target.game.window_height = self.window_height.max(240);
        target.game.fullscreen = self.fullscreen;
        target.game.jvm_args = self.jvm_args;
        target.game.game_args = self.game_args;
        target.game.gc = self.gc.into();
        target.game.wrapper_command = self.wrapper_command.trim().to_owned();
        target.game.pre_launch_command = self.pre_launch_command;
        target.game.env_vars = self.env_vars;
        target.game.selected_version = self.selected_version.filter(|v| !v.is_empty());
        target.game.auto_java = self.auto_java;
        target.game.log4j_fix = self.log4j_fix;
        let name = self.offline_name.trim();
        target.account.offline_name = if name.is_empty() { "Player".into() } else { name.to_owned() };
        target.account.selected = self.selected_account;
        target.mods.curseforge_api_key = self.curseforge_api_key.trim().to_owned();
        target.mods.cleanroom_direct = self.cleanroom_direct;
        target.game.alkaline_dir = non_empty(self.alkaline_dir);
        target.game.alkaline_feed = self.alkaline_feed.map(|f| f.trim().to_owned()).filter(|f| !f.is_empty());
        target.launch_page.column = match self.launch_column {
            LaunchColumn::Left => core::LaunchColumn::Left,
            LaunchColumn::Right => core::LaunchColumn::Right,
        };
        target.launch_page.widgets = self.launch_widgets.into_iter().map(Into::into).collect();
    }
}

impl From<core::GarbageCollector> for GarbageCollector {
    fn from(gc: core::GarbageCollector) -> Self {
        match gc {
            core::GarbageCollector::Auto => Self::Auto,
            core::GarbageCollector::G1 => Self::G1,
            core::GarbageCollector::Zgc => Self::Zgc,
            core::GarbageCollector::Shenandoah => Self::Shenandoah,
            core::GarbageCollector::Parallel => Self::Parallel,
        }
    }
}

impl From<GarbageCollector> for core::GarbageCollector {
    fn from(gc: GarbageCollector) -> Self {
        match gc {
            GarbageCollector::Auto => Self::Auto,
            GarbageCollector::G1 => Self::G1,
            GarbageCollector::Zgc => Self::Zgc,
            GarbageCollector::Shenandoah => Self::Shenandoah,
            GarbageCollector::Parallel => Self::Parallel,
        }
    }
}

impl From<&core::LaunchWidget> for LaunchWidget {
    fn from(w: &core::LaunchWidget) -> Self {
        Self {
            kind: match w.kind {
                core::WidgetKind::News => WidgetKind::News,
                core::WidgetKind::Versions => WidgetKind::Versions,
                core::WidgetKind::Note => WidgetKind::Note,
                core::WidgetKind::Links => WidgetKind::Links,
                core::WidgetKind::Markdown => WidgetKind::Markdown,
            },
            title: w.title.clone(),
            content: w.content.clone(),
        }
    }
}

impl From<LaunchWidget> for core::LaunchWidget {
    fn from(w: LaunchWidget) -> Self {
        Self {
            kind: match w.kind {
                WidgetKind::News => core::WidgetKind::News,
                WidgetKind::Versions => core::WidgetKind::Versions,
                WidgetKind::Note => core::WidgetKind::Note,
                WidgetKind::Links => core::WidgetKind::Links,
                WidgetKind::Markdown => core::WidgetKind::Markdown,
            },
            title: w.title,
            content: w.content,
        }
    }
}

impl From<&core::VersionSettings> for VersionSettings {
    fn from(v: &core::VersionSettings) -> Self {
        Self {
            java_path: v.java_path.clone().map(path_string),
            max_memory_mib: v.max_memory_mib,
            min_memory_mib: v.min_memory_mib,
            window_width: v.window_width,
            window_height: v.window_height,
            fullscreen: v.fullscreen,
            jvm_args: v.jvm_args.clone(),
            game_args: v.game_args.clone(),
            gc: v.gc.map(Into::into),
            wrapper_command: v.wrapper_command.clone(),
            pre_launch_command: v.pre_launch_command.clone(),
            env_vars: v.env_vars.clone(),
            log4j_fix: v.log4j_fix,
            server_address: v.server_address.clone(),
        }
    }
}

impl From<VersionSettings> for core::VersionSettings {
    /// Blank text counts as "follow the global setting".
    fn from(v: VersionSettings) -> Self {
        let text = |t: Option<String>| t.filter(|t| !t.trim().is_empty());
        Self {
            java_path: non_empty(v.java_path),
            max_memory_mib: v.max_memory_mib.map(|m| m.max(512)),
            min_memory_mib: v.min_memory_mib,
            window_width: v.window_width.map(|w| w.max(320)),
            window_height: v.window_height.map(|h| h.max(240)),
            fullscreen: v.fullscreen,
            jvm_args: text(v.jvm_args),
            game_args: text(v.game_args),
            gc: v.gc.map(Into::into),
            wrapper_command: text(v.wrapper_command).map(|t| t.trim().to_owned()),
            pre_launch_command: text(v.pre_launch_command),
            env_vars: text(v.env_vars),
            log4j_fix: v.log4j_fix,
            server_address: text(v.server_address).map(|t| t.trim().to_owned()),
        }
    }
}

impl From<sulfur_minecraft::news::NewsEntry> for NewsEntry {
    fn from(n: sulfur_minecraft::news::NewsEntry) -> Self {
        Self { id: n.id, title: n.title, text: n.text, date: n.date, image_url: n.image_url, link: n.link }
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
