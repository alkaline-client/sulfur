//! Modrinth modpacks (`.mrpack`): a zip with `modrinth.index.json`, which
//! lists the game version, the mod loader and files to download, plus
//! `overrides/` (and `client-overrides/`) copied into the game folder as is.

use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

use crate::{ModsError, Result};

/// Hosts the format allows pack files to come from.
const ALLOWED_HOSTS: &[&str] = &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

/// The loader a pack runs on, with the `dependencies` key naming it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackLoader {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

const LOADER_KEYS: &[(&str, PackLoader)] = &[
    ("forge", PackLoader::Forge),
    ("neoforge", PackLoader::NeoForge),
    ("fabric-loader", PackLoader::Fabric),
    ("quilt-loader", PackLoader::Quilt),
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub files: Vec<PackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    /// Destination relative to the game folder.
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<PackEnv>,
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackEnv {
    pub client: String,
}

impl PackIndex {
    /// Reads the index of the pack at `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let mut zip = open(path)?;
        let entry = zip
            .by_name("modrinth.index.json")
            .map_err(|_| ModsError::Pack(format!("{} is not a Modrinth modpack", path.display())))?;
        let index: Self = serde_json::from_reader(entry).map_err(|source| ModsError::Json {
            url: path.display().to_string(),
            source,
        })?;
        if index.format_version != 1 || index.game != "minecraft" {
            return Err(ModsError::Pack(format!(
                "unsupported modpack format {} for {}",
                index.format_version, index.game
            )));
        }
        Ok(index)
    }

    pub fn minecraft(&self) -> Result<&str> {
        self.dependencies
            .get("minecraft")
            .map(String::as_str)
            .ok_or_else(|| ModsError::Pack("the modpack names no Minecraft version".into()))
    }

    /// The pack's mod loader and its version, if any.
    pub fn loader(&self) -> Result<Option<(PackLoader, &str)>> {
        let mut found = LOADER_KEYS
            .iter()
            .filter_map(|(key, loader)| self.dependencies.get(*key).map(|v| (*loader, v.as_str())));
        let first = found.next();
        if found.next().is_some() {
            return Err(ModsError::Pack("the modpack names more than one mod loader".into()));
        }
        Ok(first)
    }

    /// Files the client needs (optional ones included), checked: each has a
    /// safe relative path, a SHA-1 and only download links on allowed hosts.
    pub fn client_files(&self) -> Result<Vec<(PathBuf, &PackFile)>> {
        self.files
            .iter()
            .filter(|f| f.env.as_ref().is_none_or(|e| e.client != "unsupported"))
            .map(|f| {
                let path = safe_relative(&f.path).ok_or_else(|| ModsError::Pack(format!("unsafe file path {:?}", f.path)))?;
                if !f.hashes.contains_key("sha1") {
                    return Err(ModsError::Pack(format!("{} has no SHA-1", f.path)));
                }
                if f.downloads.is_empty() || !f.downloads.iter().all(|u| allowed_url(u)) {
                    return Err(ModsError::Pack(format!("{} downloads from a host modpacks may not use", f.path)));
                }
                Ok((path, f))
            })
            .collect()
    }
}

/// Copies `overrides/` and then `client-overrides/` into `dest`; returns the
/// number of files written. Entries leaving `dest` are refused.
pub fn extract_overrides(pack: &Path, dest: &Path) -> Result<usize> {
    let mut zip = open(pack)?;
    let mut written = 0;
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| ModsError::Pack(e.to_string()))?;
            let Some(name) = entry.enclosed_name() else { continue };
            let Ok(relative) = name.strip_prefix(prefix) else { continue };
            if relative.as_os_str().is_empty() || entry.is_dir() {
                continue;
            }
            let Some(relative) = relative.to_str().and_then(safe_relative) else { continue };
            let target = dest.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            let mut out = File::create(&target).map_err(|e| io(&target, e))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| io(&target, e))?;
            written += 1;
        }
    }
    Ok(written)
}

fn open(path: &Path) -> Result<zip::ZipArchive<BufReader<File>>> {
    let file = File::open(path).map_err(|e| io(path, e))?;
    zip::ZipArchive::new(BufReader::new(file))
        .map_err(|_| ModsError::Pack(format!("{} is not a Modrinth modpack", path.display())))
}

fn io(path: &Path, e: std::io::Error) -> ModsError {
    ModsError::Pack(format!("{}: {e}", path.display()))
}

/// `path` as a relative path that stays inside its base folder.
fn safe_relative(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.contains('\\') || path.contains(':') {
        return None;
    }
    let path = Path::new(path);
    path.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| path.to_path_buf())
}

fn allowed_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    ALLOWED_HOSTS.contains(&host)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn pack(dir: &Path, index: &str, extra: &[(&str, &str)]) -> PathBuf {
        let path = dir.join("pack.mrpack");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        zip.write_all(index.as_bytes()).unwrap();
        for (name, body) in extra {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn reads_index_and_overrides() {
        let dir = std::env::temp_dir().join(format!("sulfur-mrpack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let index = r#"{
            "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Test Pack",
            "files": [
                {"path": "mods/a.jar", "hashes": {"sha1": "aa", "sha512": "bb"},
                 "downloads": ["https://cdn.modrinth.com/data/x/a.jar"], "fileSize": 3},
                {"path": "mods/server.jar", "hashes": {"sha1": "cc"}, "env": {"client": "unsupported", "server": "required"},
                 "downloads": ["https://cdn.modrinth.com/data/x/s.jar"]}
            ],
            "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.5"}
        }"#;
        let path = pack(
            &dir,
            index,
            &[("overrides/config/a.txt", "one"), ("client-overrides/options.txt", "two"), ("other/x", "no")],
        );
        let index = PackIndex::read(&path).unwrap();
        assert_eq!(index.minecraft().unwrap(), "1.21.1");
        assert_eq!(index.loader().unwrap(), Some((PackLoader::Fabric, "0.16.5")));
        let files = index.client_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, PathBuf::from("mods/a.jar"));

        let out = dir.join("game");
        assert_eq!(extract_overrides(&path, &out).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(out.join("config/a.txt")).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(out.join("options.txt")).unwrap(), "two");
        assert!(!out.join("x").exists() && !out.join("other").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_unsafe_files() {
        assert_eq!(safe_relative("mods/a.jar"), Some(PathBuf::from("mods/a.jar")));
        for bad in ["../a.jar", "/etc/passwd", "mods/../../a", "C:/a", "mods\\a.jar", ""] {
            assert_eq!(safe_relative(bad), None, "{bad}");
        }
        assert!(allowed_url("https://cdn.modrinth.com/data/x/a.jar"));
        assert!(allowed_url("https://github.com/o/r/releases/download/v/a.jar"));
        assert!(!allowed_url("http://cdn.modrinth.com/a.jar"));
        assert!(!allowed_url("https://cdn.modrinth.com.evil.example/a.jar"));
        assert!(!allowed_url("https://example.com/a.jar"));
    }
}
