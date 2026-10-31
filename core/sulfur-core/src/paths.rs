use std::{io, path::PathBuf};

/// Platform-specific directories Sulfur uses.
///
/// | | Linux | macOS | Windows |
/// |---|---|---|---|
/// | config | `~/.config/sulfur` | `~/Library/Application Support/dev.Sulfur.Sulfur` | `%APPDATA%\Sulfur\Sulfur\config` |
/// | data | `~/.local/share/sulfur` | `~/Library/Application Support/dev.Sulfur.Sulfur` | `%APPDATA%\Sulfur\Sulfur\data` |
#[derive(Clone, Debug)]
pub struct AppDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
}

impl AppDirs {
    /// Resolves the standard directories for the current user.
    pub fn standard() -> io::Result<Self> {
        let dirs = directories::ProjectDirs::from("dev", "Sulfur", "Sulfur")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
        Ok(Self::rooted(
            dirs.config_dir().to_owned(),
            dirs.data_dir().to_owned(),
            dirs.cache_dir().to_owned(),
        ))
    }

    /// All directories under one root; used for portable installs and tests.
    pub fn portable(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self::rooted(root.join("config"), root.join("data"), root.join("cache"))
    }

    fn rooted(config: PathBuf, data: PathBuf, cache: PathBuf) -> Self {
        let logs = data.join("logs");
        Self {
            config,
            data,
            cache,
            logs,
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.toml")
    }

    /// Game directory used when the user has not picked one.
    pub fn default_game_dir(&self) -> PathBuf {
        self.data.join("minecraft")
    }

    /// Java runtimes Sulfur downloaded.
    pub fn java_dir(&self) -> PathBuf {
        self.data.join("java")
    }

    pub fn create_all(&self) -> io::Result<()> {
        for dir in [&self.config, &self.data, &self.cache, &self.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
