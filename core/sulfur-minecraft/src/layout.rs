use std::path::{Path, PathBuf};

/// The official launcher's game directory, if it exists.
pub fn official_game_dir() -> Option<GameDir> {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let dir = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support/minecraft"))
    } else {
        home().map(|h| h.join(".minecraft"))
    }?;
    dir.is_dir().then(|| GameDir::new(dir))
}

/// The on-disk layout of a game directory, compatible with the official launcher.
#[derive(Clone, Debug)]
pub struct GameDir {
    root: PathBuf,
}

impl GameDir {
    /// `root` is made absolute: the game runs from a different working
    /// directory, so every path handed to it must be absolute.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            root: std::path::absolute(&root).unwrap_or(root),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.version_dir(id).join("natives")
    }

    pub fn manifest_cache(&self) -> PathBuf {
        self.versions().join("version_manifest_v2.json")
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// `relative` uses `/` separators, as in Maven paths.
    pub fn library(&self, relative: &str) -> PathBuf {
        let mut path = self.libraries();
        path.extend(relative.split('/'));
        path
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets().join("objects").join(&hash[..2.min(hash.len())]).join(hash)
    }

    /// A Log4j configuration from a version's `logging` section.
    pub fn log_config(&self, id: &str) -> PathBuf {
        self.assets().join("log_configs").join(id)
    }

    pub fn virtual_assets(&self, index_id: &str) -> PathBuf {
        self.assets().join("virtual").join(index_id)
    }

    /// The game's working directory (`${game_directory}`): saves, mods,
    /// options and logs live here.
    pub fn run_dir(&self, id: &str, isolated: bool) -> PathBuf {
        if isolated { self.version_dir(id) } else { self.root.clone() }
    }

    /// Equivalent of `path` (inside this directory) under another root.
    pub fn rebase(&self, path: &Path, other: &GameDir) -> Option<PathBuf> {
        path.strip_prefix(&self.root).ok().map(|rel| other.root.join(rel))
    }

    /// Ids of versions whose metadata and jar are present.
    pub fn installed_versions(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.versions()) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|id| self.version_json(id).is_file())
            .collect();
        ids.sort();
        ids
    }
}
