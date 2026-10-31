//! Reading settings from other launchers: HMCL, PCL Community Edition and
//! the original PCL2.
//!
//! Both are portable apps that keep their settings next to their executable,
//! so the user points Sulfur at the folder they live in; a few user-wide
//! locations are checked as well. Only settings with a Sulfur equivalent are
//! read. Accounts stay behind: PCL encrypts them per machine, and Sulfur's own
//! account rules apply anyway.
//!
//! Formats, from the launchers' sources:
//! * HMCL 3.16+: `.hmcl/config/{game-directories,game-settings,launcher-settings,accounts}.json`
//!   in its folder; before that `hmcl.json` (`.hmcl.json` on Linux).
//! * PCL CE: `PCL/config.v1.yml` in its folder (flat YAML, formerly
//!   `PCL/setup.ini`) and `%APPDATA%\PCLCE\config.v1.json`.
//! * PCL2: `PCL/Setup.ini` (`key:value` lines) and `HKCU\Software\PCL`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{DownloadSource, Isolation, Settings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeignLauncher {
    Hmcl,
    PclCe,
    Pcl2,
}

/// Settings found in another launcher's folder. `None` means not set there.
#[derive(Clone, Debug, PartialEq)]
pub struct ForeignConfig {
    pub launcher: ForeignLauncher,
    /// The folder the launcher's settings were found in.
    pub location: PathBuf,
    /// Its game folders (`.minecraft`), the selected one first.
    pub game_dirs: Vec<PathBuf>,
    pub selected_version: Option<String>,
    pub max_memory_mib: Option<u32>,
    pub jvm_args: Option<String>,
    pub window: Option<(u32, u32)>,
    pub fullscreen: Option<bool>,
    pub java_path: Option<PathBuf>,
    pub download_source: Option<DownloadSource>,
    pub max_connections: Option<u32>,
    pub isolation: Option<Isolation>,
    pub offline_name: Option<String>,
    pub background_image: Option<PathBuf>,
    /// `en`, `zh-CN` or `zh-TW`.
    pub language: Option<String>,
}

impl ForeignConfig {
    fn new(launcher: ForeignLauncher, location: &Path) -> Self {
        Self {
            launcher,
            location: location.to_owned(),
            game_dirs: Vec::new(),
            selected_version: None,
            max_memory_mib: None,
            jvm_args: None,
            window: None,
            fullscreen: None,
            java_path: None,
            download_source: None,
            max_connections: None,
            isolation: None,
            offline_name: None,
            background_image: None,
            language: None,
        }
    }

    /// Copies what was found into `settings`. With `use_game_dir`, the
    /// launcher's selected game folder becomes Sulfur's; otherwise (and for
    /// its other folders) files there are reused instead of downloaded.
    pub fn apply(&self, settings: &mut Settings, use_game_dir: bool) {
        let mut dirs = self.game_dirs.iter();
        if use_game_dir && let Some(first) = dirs.next() {
            settings.game.game_dir = Some(first.clone());
            settings.game.selected_version = self.selected_version.clone();
        }
        for dir in dirs {
            if Some(dir) != settings.game.game_dir.as_ref() && !settings.game.reuse_dirs.contains(dir) {
                settings.game.reuse_dirs.push(dir.clone());
            }
        }
        let game = &mut settings.game;
        if let Some(v) = self.max_memory_mib {
            game.max_memory_mib = v.clamp(512, 65536);
        }
        if let Some(v) = &self.jvm_args {
            game.jvm_args = v.clone();
        }
        if let Some((w, h)) = self.window {
            (game.window_width, game.window_height) = (w.max(320), h.max(240));
        }
        if let Some(v) = self.fullscreen {
            game.fullscreen = v;
        }
        if let Some(v) = &self.java_path {
            game.java_path = Some(v.clone());
        }
        if let Some(v) = self.isolation {
            game.isolation = v;
        }
        if let Some(v) = self.download_source {
            settings.download.source = v;
        }
        if let Some(v) = self.max_connections {
            settings.download.max_connections = v.clamp(8, 256);
        }
        if let Some(v) = &self.offline_name {
            settings.account.offline_name = v.clone();
        }
        if let Some(v) = &self.background_image {
            settings.appearance.background_image = Some(v.clone());
        }
        if let Some(v) = &self.language {
            settings.appearance.language = v.clone();
        }
    }

    fn is_empty(&self) -> bool {
        *self == Self::new(self.launcher, &self.location)
    }
}

/// Looks for other launchers' settings in `folders` (folders the user chose:
/// where HMCL or PCL is, or their `.hmcl`/`PCL` subfolder) and in user-wide
/// locations.
pub fn scan(folders: &[PathBuf]) -> Vec<ForeignConfig> {
    let mut found: Vec<ForeignConfig> = Vec::new();
    let mut add = |config: Option<ForeignConfig>| {
        if let Some(c) = config.filter(|c| !c.is_empty())
            && !found.iter().any(|f| f.launcher == c.launcher && f.location == c.location)
        {
            found.push(c);
        }
    };
    for folder in folders {
        // Accept the settings subfolder itself, too.
        let base = match folder.file_name().and_then(|n| n.to_str()) {
            Some(".hmcl" | "PCL") => folder.parent().unwrap_or(folder),
            _ => folder.as_path(),
        };
        add(hmcl(base));
        add(pcl(base));
    }
    if let Some(home) = hmcl_user_home() {
        add(hmcl_user(&home));
    }
    if let Some(shared) = pcl_ce_shared_file().filter(|p| p.is_file()) {
        let mut c = ForeignConfig::new(ForeignLauncher::PclCe, shared.parent().unwrap_or(&shared));
        read_pcl(&mut c, &read_json_map(&shared), None);
        add(Some(c));
    }
    found
}

// ---------------------------------------------------------------- HMCL

fn hmcl(dir: &Path) -> Option<ForeignConfig> {
    let config = dir.join(".hmcl").join("config");
    if config.join("game-directories.json").is_file() || config.join("game-settings.json").is_file() {
        return Some(hmcl_current(dir, &config));
    }
    let legacy = [dir.join("hmcl.json"), dir.join(".hmcl.json")].into_iter().find(|p| p.is_file())?;
    Some(hmcl_legacy(dir, &read_json(&legacy)?))
}

/// HMCL 3.16+ split settings.
fn hmcl_current(dir: &Path, config: &Path) -> ForeignConfig {
    let mut c = ForeignConfig::new(ForeignLauncher::Hmcl, dir);
    if let Some(dirs) = read_json(&config.join("game-directories.json")) {
        for entry in dirs.get("directories").and_then(Value::as_array).into_iter().flatten() {
            if let Some(path) = entry.get("path").and_then(Value::as_str) {
                push_dir(&mut c.game_dirs, resolve(dir, path));
            }
        }
    }
    if let Some(presets) = read_json(&config.join("game-settings.json")) {
        // The first preset is the default one new directories start from.
        if let Some(p) = presets.get("presets").and_then(Value::as_array).and_then(|a| a.first()) {
            read_hmcl_game(&mut c, p);
        }
    }
    if let Some(launcher) = read_json(&config.join("launcher-settings.json")) {
        read_hmcl_launcher(&mut c, &launcher);
    }
    if let Some(accounts) = read_json(&config.join("accounts.json")) {
        c.offline_name = offline_name(&accounts);
    }
    c
}

/// The user-wide HMCL folder only holds settings shared by all copies.
fn hmcl_user(home: &Path) -> Option<ForeignConfig> {
    let mut c = ForeignConfig::new(ForeignLauncher::Hmcl, home);
    let config = home.join("config");
    if let Some(dirs) = read_json(&config.join("user-game-directories.json")) {
        for entry in dirs.get("directories").and_then(Value::as_array).into_iter().flatten() {
            if let Some(path) = entry.get("path").and_then(Value::as_str) {
                push_dir(&mut c.game_dirs, resolve(home, path));
            }
        }
    }
    if let Some(accounts) = read_json(&config.join("user-accounts.json")) {
        c.offline_name = offline_name(&accounts);
    }
    Some(c)
}

fn read_hmcl_game(c: &mut ForeignConfig, p: &Value) {
    if !get_bool(p, "autoMemory").unwrap_or(true) {
        c.max_memory_mib = get_u32(p, "maxMemory");
    }
    c.jvm_args = get_str(p, "jvmOptions").or_else(|| get_str(p, "javaArgs")).filter(|s| !s.trim().is_empty());
    if let (Some(w), Some(h)) = (get_u32(p, "width"), get_u32(p, "height"))
        && w > 0
        && h > 0
    {
        c.window = Some((w, h));
    }
    c.fullscreen = match get_str(p, "windowType").as_deref() {
        Some("FULLSCREEN") => Some(true),
        Some(_) => Some(false),
        None => get_bool(p, "fullscreen"),
    };
    c.java_path = get_str(p, "customJavaPath")
        .filter(|_| get_str(p, "javaType").as_deref() == Some("CUSTOM"))
        .map(PathBuf::from);
    c.isolation = match get_str(p, "defaultIsolationType").as_deref() {
        Some("NEVER") => Some(Isolation::Shared),
        Some("ALWAYS" | "MODDED") => Some(Isolation::Isolated),
        _ => None,
    };
}

fn read_hmcl_launcher(c: &mut ForeignConfig, l: &Value) {
    c.download_source = match get_str(l, "fileDownloadSource").or_else(|| get_str(l, "downloadType")).as_deref() {
        Some("OFFICIAL" | "mojang" | "official") => Some(DownloadSource::Official),
        Some("MIRROR" | "bmclapi" | "mirror") => Some(DownloadSource::Mirror),
        Some(_) => Some(DownloadSource::Auto),
        None => None,
    };
    if get_bool(l, "autoDownloadThreads") == Some(false) {
        c.max_connections = get_u32(l, "downloadThreads");
    }
    let custom = matches!(get_str(l, "backgroundType").as_deref(), Some("CUSTOM"));
    c.background_image = get_str(l, "customBackgroundImagePath")
        .or_else(|| get_str(l, "bgpath"))
        .filter(|p| custom && !p.is_empty())
        .map(PathBuf::from);
    c.language = get_str(l, "language").or_else(|| get_str(l, "localization")).and_then(|l| language(&l));
}

/// Settings of HMCL before 3.16, all in one file.
fn hmcl_legacy(dir: &Path, json: &Value) -> ForeignConfig {
    let mut c = ForeignConfig::new(ForeignLauncher::Hmcl, dir);
    let profiles = json.get("configurations").and_then(Value::as_object);
    let last = get_str(json, "last").unwrap_or_else(|| "Default".into());
    if let Some(profiles) = profiles {
        let mut ordered: Vec<(&String, &Value)> = profiles.iter().collect();
        ordered.sort_by_key(|(name, _)| **name != last);
        for (_, profile) in &ordered {
            if let Some(path) = get_str(profile, "gameDir") {
                push_dir(&mut c.game_dirs, resolve(dir, &path));
            }
        }
        if let Some((_, profile)) = ordered.first() {
            c.selected_version = get_str(profile, "selectedMinecraftVersion").filter(|s| !s.is_empty());
            if let Some(global) = profile.get("global") {
                read_hmcl_game(&mut c, global);
                let java = get_str(global, "java").unwrap_or_default();
                if !matches!(java.as_str(), "" | "Auto" | "Default") {
                    c.java_path = Some(PathBuf::from(java));
                }
                c.isolation = match global.get("gameDirType") {
                    Some(Value::Number(n)) if n.as_u64() == Some(0) => Some(Isolation::Shared),
                    Some(Value::Number(n)) if n.as_u64() == Some(1) => Some(Isolation::Isolated),
                    Some(Value::String(s)) if s == "ROOT_FOLDER" => Some(Isolation::Shared),
                    Some(Value::String(s)) if s == "VERSION_FOLDER" => Some(Isolation::Isolated),
                    _ => None,
                };
            }
        }
    }
    read_hmcl_launcher(&mut c, json);
    if json.get("backgroundType").is_none() && get_str(json, "bgpath").is_some_and(|p| !p.is_empty()) {
        c.background_image = get_str(json, "bgpath").map(PathBuf::from);
    }
    c.offline_name = offline_name(json);
    c
}

/// The first offline account's name anywhere in an accounts structure.
fn offline_name(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("offline") {
                return map.get("username").or_else(|| map.get("name")).and_then(Value::as_str).map(String::from);
            }
            map.values().find_map(offline_name)
        }
        Value::Array(items) => items.iter().find_map(offline_name),
        _ => None,
    }
}

fn hmcl_user_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HMCL_USER_HOME") {
        return Some(home.into());
    }
    let home = directories::BaseDirs::new()?;
    // %APPDATA%\.hmcl, ~/Library/Application Support/hmcl, $XDG_DATA_HOME/hmcl
    Some(if cfg!(windows) { home.config_dir().join(".hmcl") } else { home.data_dir().join("hmcl") })
}

// ---------------------------------------------------------------- PCL

fn pcl(dir: &Path) -> Option<ForeignConfig> {
    let data = dir.join("PCL");
    let yaml = data.join("config.v1.yml");
    let ini = ["setup.ini", "Setup.ini"].iter().map(|n| data.join(n)).find(|p| p.is_file());
    let (launcher, local) = if yaml.is_file() {
        (ForeignLauncher::PclCe, read_flat_yaml(&std::fs::read_to_string(&yaml).ok()?))
    } else {
        // PCL CE migrates setup.ini on first start, so a bare one is PCL2's.
        (ForeignLauncher::Pcl2, read_cat_ini(&std::fs::read_to_string(ini?).ok()?))
    };
    let mut shared = match launcher {
        ForeignLauncher::PclCe => pcl_ce_shared_file().map(|p| read_json_map(&p)).unwrap_or_default(),
        _ => pcl2_registry(),
    };
    // Local values win over shared ones with the same key.
    shared.extend(local);
    let mut c = ForeignConfig::new(launcher, dir);
    read_pcl(&mut c, &shared, Some(dir));
    Some(c)
}

/// Reads PCL keys. `base` replaces the `$` prefix PCL uses for paths
/// relative to its own folder.
fn read_pcl(c: &mut ForeignConfig, keys: &HashMap<String, String>, base: Option<&Path>) {
    let get = |k: &str| keys.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let int = |k: &str| get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let path = |p: &str| -> Option<PathBuf> {
        match (p.strip_prefix('$'), base) {
            (Some(rest), Some(base)) => Some(base.join(rest.replace('\\', "/"))),
            (Some(_), None) => None,
            (None, _) => Some(PathBuf::from(p)),
        }
    };
    if let Some(selected) = get("LaunchFolderSelect").and_then(path) {
        push_dir(&mut c.game_dirs, selected);
    }
    // `name>path|name>path`
    for entry in get("LaunchFolders").unwrap_or_default().split('|') {
        if let Some(dir) = entry.split_once('>').and_then(|(_, p)| path(p)) {
            push_dir(&mut c.game_dirs, dir);
        }
    }
    c.selected_version = get("LaunchInstanceSelect").or_else(|| get("LaunchVersionSelect")).map(String::from);
    if int("LaunchRamType") == Some(1) {
        c.max_memory_mib = int("LaunchRamCustom").map(|v| pcl_memory_mib(v as u32));
    }
    c.jvm_args = get("LaunchAdvanceJvm").map(String::from);
    c.fullscreen = int("LaunchArgumentWindowType").map(|t| t == 0);
    if int("LaunchArgumentWindowType") == Some(3)
        && let (Some(w), Some(h)) = (int("LaunchArgumentWindowWidth"), int("LaunchArgumentWindowHeight"))
    {
        c.window = Some((w.max(100) as u32, h.max(100) as u32));
    }
    c.isolation = int("LaunchArgumentIndieV2").or_else(|| int("LaunchArgumentIndie")).map(|v| match v {
        0 => Isolation::Shared,
        _ => Isolation::Isolated,
    });
    c.download_source = int("ToolDownloadSource").map(|v| match v {
        0 => DownloadSource::Mirror,
        2 => DownloadSource::Official,
        _ => DownloadSource::Auto,
    });
    c.max_connections = int("ToolDownloadThread").map(|v| (v + 1).clamp(1, 1024) as u32);
    c.java_path = get("LaunchArgumentJavaSelect").and_then(pcl_java);
}

/// PCL's memory slider position in MiB (its `RamGive` curve).
fn pcl_memory_mib(value: u32) -> u32 {
    let gib = match value {
        0..=12 => value as f64 * 0.1 + 0.3,
        13..=25 => (value - 12) as f64 * 0.5 + 1.5,
        26..=33 => (value - 25) as f64 + 8.0,
        _ => (value - 33) as f64 * 2.0 + 16.0,
    };
    (gib * 1024.0).round() as u32
}

/// The selected Java: `{"Path": "<folder>", …}` (PCL2) or a path.
fn pcl_java(value: &str) -> Option<PathBuf> {
    let path = if value.starts_with('{') {
        serde_json::from_str::<Value>(value).ok()?.get("Path")?.as_str()?.to_owned()
    } else {
        value.to_owned()
    };
    let path = PathBuf::from(path);
    if path.is_dir() {
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        return [path.join(exe), path.join("bin").join(exe)].into_iter().find(|p| p.is_file());
    }
    Some(path)
}

fn pcl_ce_shared_file() -> Option<PathBuf> {
    let appdata = directories::BaseDirs::new()?.config_dir().to_owned();
    let current = appdata.join("PCLCE").join("config.v1.json");
    if current.is_file() {
        return Some(current);
    }
    Some(appdata.join(".PCLCE").join("Config.json"))
}

/// PCL2's shared settings in `HKCU\Software\PCL`. Values PCL encrypts
/// (accounts) come back unreadable and are not used.
fn pcl2_registry() -> HashMap<String, String> {
    if !cfg!(windows) {
        return HashMap::new();
    }
    let Ok(out) = std::process::Command::new("reg").args(["query", r"HKCU\Software\PCL"]).output() else {
        return HashMap::new();
    };
    // `    Name    REG_SZ    Value`
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.trim().splitn(3, "    ");
            let (name, kind, value) = (parts.next()?, parts.next()?, parts.next().unwrap_or_default());
            kind.starts_with("REG_").then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

// ---------------------------------------------------------------- formats

/// LTCat-style ini: one `key:value` per line.
fn read_cat_ini(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.to_owned()))
        .collect()
}

/// A flat YAML mapping of scalars, as PCL CE writes it: plain, `'single'`
/// (with `''` escapes) or `"double"` quoted values. Nested or multi-line
/// values are skipped.
fn read_flat_yaml(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t', '#', '-']) || line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        let value = if let Some(v) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            v.replace("''", "'")
        } else if let Some(v) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            serde_json::from_str::<String>(&format!("\"{v}\"")).unwrap_or_else(|_| v.to_owned())
        } else if matches!(value, "" | "|" | ">" | "~" | "null") {
            continue;
        } else {
            value.to_owned()
        };
        out.insert(key.trim().trim_matches(['\'', '"']).to_owned(), value);
    }
    out
}

/// A flat JSON object with scalar values, as strings.
fn read_json_map(path: &Path) -> HashMap<String, String> {
    let Some(Value::Object(map)) = read_json(path) else { return HashMap::new() };
    map.into_iter()
        .filter_map(|(k, v)| match v {
            Value::String(s) => Some((k, s)),
            Value::Number(n) => Some((k, n.to_string())),
            Value::Bool(b) => Some((k, b.to_string())),
            _ => None,
        })
        .collect()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn get_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

fn get_bool(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

fn get_u32(v: &Value, key: &str) -> Option<u32> {
    let n = v.get(key)?;
    n.as_f64().or_else(|| n.as_str()?.parse().ok()).filter(|n| *n >= 0.0).map(|n| n as u32)
}

/// A path as another launcher stored it, relative to its folder if not absolute.
fn resolve(base: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path.replace('\\', std::path::MAIN_SEPARATOR_STR));
    if p.is_absolute() || path.starts_with(['/', '\\']) || path.get(1..2) == Some(":") {
        PathBuf::from(path)
    } else {
        base.join(p)
    }
}

fn push_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

/// Sulfur's language code for another launcher's (`zh_CN`, `zh-Hant`, `en`).
fn language(code: &str) -> Option<String> {
    let code = code.replace('_', "-").to_ascii_lowercase();
    Some(match code.as_str() {
        "zh-cn" | "zh-hans" | "zh" | "zh-sg" => "zh-CN",
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "zh-TW",
        c if c.starts_with("en") => "en",
        _ => return None,
    }
    .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sulfur-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_pcl2_setup_ini() {
        let dir = temp_dir("pcl2");
        std::fs::create_dir_all(dir.join("PCL")).unwrap();
        std::fs::write(
            dir.join("PCL/Setup.ini"),
            "LaunchFolderSelect:$.minecraft\\\nLaunchRamType:1\nLaunchRamCustom:20\nLaunchArgumentWindowType:3\n\
             LaunchArgumentWindowWidth:1600\nLaunchArgumentWindowHeight:900\nLaunchArgumentIndieV2:0\n\
             LaunchAdvanceJvm:-XX:+UseG1GC -Dx=y\n",
        )
        .unwrap();
        let c = pcl(&dir).unwrap();
        assert_eq!(c.launcher, ForeignLauncher::Pcl2);
        assert_eq!(c.game_dirs, [dir.join(".minecraft/")]);
        assert_eq!(c.max_memory_mib, Some(5632)); // 5.5 GiB
        assert_eq!(c.window, Some((1600, 900)));
        assert_eq!(c.fullscreen, Some(false));
        assert_eq!(c.isolation, Some(Isolation::Shared));
        assert_eq!(c.jvm_args.as_deref(), Some("-XX:+UseG1GC -Dx=y"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_pcl_ce_yaml() {
        let yaml = "LocalFileVersion: 1\nLaunchFolderSelect: '$.minecraft\\'\nLaunchAdvanceJvm: \"-Da=\\\"b\\\"\"\n\
                    ToolDownloadSource: 0\nLaunchArgumentWindowType: 0\nNested:\n  key: 1\n";
        let keys = read_flat_yaml(yaml);
        assert_eq!(keys["LaunchFolderSelect"], "$.minecraft\\");
        assert_eq!(keys["LaunchAdvanceJvm"], "-Da=\"b\"");
        assert!(!keys.contains_key("Nested") && !keys.contains_key("key"));
        let mut c = ForeignConfig::new(ForeignLauncher::PclCe, Path::new("/pcl"));
        read_pcl(&mut c, &keys, Some(Path::new("/pcl")));
        assert_eq!(c.download_source, Some(DownloadSource::Mirror));
        assert_eq!(c.fullscreen, Some(true));
        assert_eq!(c.game_dirs, [PathBuf::from("/pcl/.minecraft/")]);
    }

    #[test]
    fn reads_legacy_hmcl_json() {
        let dir = temp_dir("hmcl");
        std::fs::write(
            dir.join("hmcl.json"),
            r#"{"last":"Main","configurations":{
                "Default":{"gameDir":".minecraft","global":{}},
                "Main":{"gameDir":"/games/mc","selectedMinecraftVersion":"1.20.1",
                        "global":{"autoMemory":false,"maxMemory":6144,"javaArgs":"-Xss2m","width":1280,"height":720,
                                  "fullscreen":false,"gameDirType":1,"java":"Auto"}}},
               "accounts":[{"type":"microsoft","displayName":"x"},{"type":"offline","username":"Steve"}],
               "downloadType":"bmclapi","localization":"zh_CN"}"#,
        )
        .unwrap();
        let c = hmcl(&dir).unwrap();
        assert_eq!(c.game_dirs, [PathBuf::from("/games/mc"), dir.join(".minecraft")]);
        assert_eq!(c.selected_version.as_deref(), Some("1.20.1"));
        assert_eq!(c.max_memory_mib, Some(6144));
        assert_eq!(c.isolation, Some(Isolation::Isolated));
        assert_eq!(c.java_path, None);
        assert_eq!(c.download_source, Some(DownloadSource::Mirror));
        assert_eq!(c.offline_name.as_deref(), Some("Steve"));
        assert_eq!(c.language.as_deref(), Some("zh-CN"));

        let mut settings = Settings::default();
        c.apply(&mut settings, true);
        assert_eq!(settings.game.game_dir, Some(PathBuf::from("/games/mc")));
        assert_eq!(settings.game.reuse_dirs, [dir.join(".minecraft")]);
        assert_eq!(settings.game.window_width, 1280);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_current_hmcl_files() {
        let dir = temp_dir("hmcl3");
        let config = dir.join(".hmcl/config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("game-directories.json"), r#"{"directories":[{"id":"x","path":".minecraft"}]}"#).unwrap();
        std::fs::write(
            config.join("game-settings.json"),
            r#"{"presets":[{"id":"d","autoMemory":false,"maxMemory":4096,"windowType":"FULLSCREEN","defaultIsolationType":"NEVER"}]}"#,
        )
        .unwrap();
        std::fs::write(config.join("launcher-settings.json"), r#"{"fileDownloadSource":"OFFICIAL","language":"zh-Hant"}"#).unwrap();
        let found = scan(std::slice::from_ref(&dir.join(".hmcl")));
        let c = found.iter().find(|c| c.location == dir).unwrap();
        assert_eq!(c.game_dirs, [dir.join(".minecraft")]);
        assert_eq!(c.max_memory_mib, Some(4096));
        assert_eq!(c.fullscreen, Some(true));
        assert_eq!(c.isolation, Some(Isolation::Shared));
        assert_eq!(c.download_source, Some(DownloadSource::Official));
        assert_eq!(c.language.as_deref(), Some("zh-TW"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
