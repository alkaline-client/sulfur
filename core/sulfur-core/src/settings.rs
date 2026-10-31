use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot access settings file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("settings file {path} is invalid: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialize settings: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Everything the user can configure. Unknown or missing keys fall back to
/// defaults, so older and newer settings files stay loadable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: AppearanceSettings,
    pub download: DownloadSettings,
    pub game: GameSettings,
    pub account: AccountSettings,
    pub mods: ModSettings,
    pub launch_page: LaunchPageSettings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeMode,
    /// Seed for the dynamic Material color scheme, `0xRRGGBB`.
    pub seed_color: u32,
    /// Where the seed comes from: `seed_color`, or a color sampled from a picture.
    pub color_source: ColorSource,
    /// Use the high-contrast variant of the generated scheme.
    pub high_contrast: bool,
    /// Where the page entries sit.
    pub navigation: NavigationPosition,
    /// A picture shown behind the pages.
    pub background_image: Option<PathBuf>,
    /// Opacity of the background picture, `0.0..=1.0`.
    pub background_opacity: f32,
    /// Blur radius of the background picture in dp.
    pub background_blur: f32,
    /// UI language: empty follows the system, else `en`, `zh-CN` or `zh-TW`.
    pub language: String,
}

/// Where the theme's seed color comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSource {
    /// `seed_color`, picked in Settings.
    #[default]
    Seed,
    /// Sampled from the desktop wallpaper.
    Wallpaper,
    /// Sampled from the background picture.
    Background,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationPosition {
    /// A bar above the page.
    #[default]
    Top,
    /// A navigation rail on the left.
    Side,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            seed_color: 0xE8B53B, // sulfur yellow
            color_source: ColorSource::Seed,
            high_contrast: false,
            navigation: NavigationPosition::Top,
            background_image: None,
            background_opacity: 0.6,
            background_blur: 12.0,
            language: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSource {
    /// Official servers first, mirror as fallback.
    #[default]
    Auto,
    /// Official servers only.
    Official,
    /// BMCLAPI mirror first, official servers as fallback.
    Mirror,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadSettings {
    pub source: DownloadSource,
    /// Concurrent connections across all files.
    pub max_connections: u32,
    /// Parallel ranges for one large file.
    pub segments_per_file: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            source: DownloadSource::Auto,
            max_connections: 256,
            segments_per_file: 8,
        }
    }
}

/// Where a version keeps saves, mods, options and other per-game state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    /// `versions/<id>/` is the game directory, so versions never share saves or mods.
    #[default]
    Isolated,
    /// All versions share the root game directory, like the official launcher.
    Shared,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    /// Overrides the default game directory. Libraries and assets always live
    /// here and are shared by all versions.
    pub game_dir: Option<PathBuf>,
    /// Default isolation for versions without an override.
    pub isolation: Isolation,
    /// Per-version isolation overrides, keyed by version id.
    pub version_isolation: BTreeMap<String, Isolation>,
    /// Other game directories (e.g. another launcher's) whose verified files
    /// are reused instead of downloaded.
    pub reuse_dirs: Vec<PathBuf>,
    /// Also reuse files from the official launcher's directory if it exists.
    pub reuse_official_dir: bool,
    /// Overrides automatic Java selection.
    pub java_path: Option<PathBuf>,
    pub max_memory_mib: u32,
    /// Initial heap (`-Xms`); 0 leaves it to the JVM.
    pub min_memory_mib: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    /// Extra JVM arguments, whitespace separated; quotes group words.
    pub jvm_args: String,
    /// Extra game arguments, after the version's own.
    pub game_args: String,
    /// Garbage collector for the game.
    pub gc: GarbageCollector,
    /// A program the game runs through, e.g. `gamemoderun` or `prime-run`.
    pub wrapper_command: String,
    /// A shell command run before the game starts; launching waits for it.
    pub pre_launch_command: String,
    /// Environment variables for the game, one `NAME=value` per line.
    pub env_vars: String,
    /// The version selected on the Launch page.
    pub selected_version: Option<String>,
    /// Download a matching Java runtime when none is installed.
    pub auto_java: bool,
    /// Patch Log4Shell (CVE-2021-44228) in versions that ship a vulnerable Log4j.
    pub log4j_fix: bool,
    /// Per-version overrides of the settings above, keyed by version id.
    pub version_settings: BTreeMap<String, VersionSettings>,
}

/// Garbage collector for the game's JVM. Collectors the chosen Java does not
/// have fall back to the JVM's default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GarbageCollector {
    /// The JVM's default (G1 on current Java).
    #[default]
    Auto,
    /// G1 tuned like the official launcher: short pauses.
    G1,
    /// Generational ZGC: sub-millisecond pauses, needs Java 17+ and more memory.
    Zgc,
    /// Shenandoah: low pauses, OpenJDK builds of Java 12+ only.
    Shenandoah,
    /// Parallel: highest throughput, longer pauses.
    Parallel,
}

/// One version's overrides; `None` (or an empty string for text) follows
/// the global [`GameSettings`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionSettings {
    pub java_path: Option<PathBuf>,
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
    /// Join this server when the game starts (`host` or `host:port`).
    pub server_address: Option<String>,
}

impl VersionSettings {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The settings one launch uses: a version's overrides over the global ones.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchSettings {
    pub java_path: Option<PathBuf>,
    pub max_memory_mib: u32,
    pub min_memory_mib: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub jvm_args: String,
    pub game_args: String,
    pub gc: GarbageCollector,
    pub wrapper_command: String,
    pub pre_launch_command: String,
    pub env_vars: String,
    pub log4j_fix: bool,
    pub server_address: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            game_dir: None,
            isolation: Isolation::Isolated,
            version_isolation: BTreeMap::new(),
            reuse_dirs: Vec::new(),
            reuse_official_dir: true,
            java_path: None,
            max_memory_mib: 4096,
            min_memory_mib: 0,
            window_width: 1280,
            window_height: 720,
            fullscreen: false,
            jvm_args: String::new(),
            game_args: String::new(),
            gc: GarbageCollector::Auto,
            wrapper_command: String::new(),
            pre_launch_command: String::new(),
            env_vars: String::new(),
            selected_version: None,
            auto_java: true,
            log4j_fix: true,
            version_settings: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountSettings {
    /// Player name for offline play.
    pub offline_name: String,
    /// The account to play with: a Microsoft account's UUID, or empty for offline.
    pub selected: String,
}

impl Default for AccountSettings {
    fn default() -> Self {
        Self {
            offline_name: "Player".into(),
            selected: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModSettings {
    /// Key for the official CurseForge API; empty uses a public mirror.
    pub curseforge_api_key: String,
    /// Download Cleanroom straight from its repository instead of through
    /// its download page. Off by default: the Cleanroom team asks launchers
    /// to send users to the page.
    pub cleanroom_direct: bool,
}

/// What the Launch page shows next to the Play column.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchPageSettings {
    /// Which side the account card and Play button sit on.
    pub column: LaunchColumn,
    /// Cards on the rest of the page, top to bottom.
    pub widgets: Vec<LaunchWidget>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchColumn {
    #[default]
    Left,
    Right,
}

/// A card on the Launch page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchWidget {
    pub kind: WidgetKind,
    /// Heading; empty uses the kind's own.
    pub title: String,
    /// Note text; links as `label | url` lines; a Markdown file path or https URL.
    pub content: String,
}

impl Default for LaunchWidget {
    fn default() -> Self {
        Self { kind: WidgetKind::Note, title: String::new(), content: String::new() }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    /// Minecraft: Java Edition news from the official launcher's feed.
    News,
    /// The latest release and snapshot.
    Versions,
    /// The player's own text.
    #[default]
    Note,
    /// A list of links.
    Links,
    /// A Markdown document from a file or a web address.
    Markdown,
}

impl GameSettings {
    pub fn isolation_for(&self, version: &str) -> Isolation {
        self.version_isolation.get(version).copied().unwrap_or(self.isolation)
    }

    /// The settings `version` launches with: its overrides, else the global values.
    pub fn launch_settings(&self, version: &str) -> LaunchSettings {
        let o = self.version_settings.get(version).cloned().unwrap_or_default();
        let text = |own: Option<String>, global: &str| own.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| global.to_owned());
        LaunchSettings {
            java_path: o.java_path.or_else(|| self.java_path.clone()),
            max_memory_mib: o.max_memory_mib.unwrap_or(self.max_memory_mib),
            min_memory_mib: o.min_memory_mib.unwrap_or(self.min_memory_mib),
            window_width: o.window_width.unwrap_or(self.window_width),
            window_height: o.window_height.unwrap_or(self.window_height),
            fullscreen: o.fullscreen.unwrap_or(self.fullscreen),
            jvm_args: text(o.jvm_args, &self.jvm_args),
            game_args: text(o.game_args, &self.game_args),
            gc: o.gc.unwrap_or(self.gc),
            wrapper_command: text(o.wrapper_command, &self.wrapper_command),
            pre_launch_command: text(o.pre_launch_command, &self.pre_launch_command),
            env_vars: text(o.env_vars, &self.env_vars),
            log4j_fix: o.log4j_fix.unwrap_or(self.log4j_fix),
            server_address: o.server_address.unwrap_or_default().trim().to_owned(),
        }
    }
}

impl Settings {
    /// Loads settings, returning defaults if the file does not exist yet.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| SettingsError::Parse {
                path: path.to_owned(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SettingsError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes settings atomically (temp file + rename).
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let io = |source| SettingsError::Io {
            path: path.to_owned(),
            source,
        };
        let text = toml::to_string_pretty(self)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_partial_files() {
        let dir = std::env::temp_dir().join(format!("sulfur-settings-{}", std::process::id()));
        let path = dir.join("settings.toml");
        let mut s = Settings::default();
        s.account.offline_name = "Steve".into();
        s.download.source = DownloadSource::Mirror;
        s.game.version_isolation.insert("1.8.9".into(), Isolation::Shared);
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);

        std::fs::write(&path, "[game]\nmax_memory_mib = 2048\n").unwrap();
        let partial = Settings::load(&path).unwrap();
        assert_eq!(partial.game.max_memory_mib, 2048);
        assert_eq!(partial.account.offline_name, "Player");
        assert_eq!(partial.game.isolation_for("1.8.9"), Isolation::Isolated);
        assert_eq!(s.game.isolation_for("1.8.9"), Isolation::Shared);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn version_settings_override_global_ones() {
        let mut game = GameSettings { jvm_args: "-Dglobal".into(), gc: GarbageCollector::G1, ..Default::default() };
        game.version_settings.insert(
            "1.20.1-forge".into(),
            VersionSettings {
                max_memory_mib: Some(8192),
                jvm_args: Some("  ".into()),
                gc: Some(GarbageCollector::Zgc),
                server_address: Some(" mc.example.com:25566 ".into()),
                ..Default::default()
            },
        );
        let forge = game.launch_settings("1.20.1-forge");
        assert_eq!(forge.max_memory_mib, 8192);
        assert_eq!(forge.jvm_args, "-Dglobal", "blank text follows the global value");
        assert_eq!(forge.gc, GarbageCollector::Zgc);
        assert_eq!(forge.server_address, "mc.example.com:25566");
        let vanilla = game.launch_settings("1.21");
        assert_eq!(vanilla.max_memory_mib, 4096);
        assert_eq!(vanilla.gc, GarbageCollector::G1);
        assert!(vanilla.server_address.is_empty());

        let text = toml::to_string(&Settings { game, ..Default::default() }).unwrap();
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(back.game.version_settings["1.20.1-forge"].gc, Some(GarbageCollector::Zgc));
    }
}
