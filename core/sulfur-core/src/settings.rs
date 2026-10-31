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
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    /// Extra JVM arguments, whitespace separated.
    pub jvm_args: String,
    /// The version selected on the Launch page.
    pub selected_version: Option<String>,
    /// Download a matching Java runtime when none is installed.
    pub auto_java: bool,
    /// Patch Log4Shell (CVE-2021-44228) in versions that ship a vulnerable Log4j.
    pub log4j_fix: bool,
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
            window_width: 1280,
            window_height: 720,
            fullscreen: false,
            jvm_args: String::new(),
            selected_version: None,
            auto_java: true,
            log4j_fix: true,
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

impl GameSettings {
    pub fn isolation_for(&self, version: &str) -> Isolation {
        self.version_isolation.get(version).copied().unwrap_or(self.isolation)
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
}
