//! Forge and NeoForge, installed from their official installer jars.
//!
//! Modern installers (Forge 1.13+, every NeoForge) carry an `install_profile.json`
//! whose *processors* deobfuscate and patch the client on the user's machine;
//! we run them with the game's Java, like the official installer does. Legacy
//! installers (Forge 1.12.2 and older) just contain a version JSON and the
//! universal jar.
//!
//! Cleanroom (a Forge fork for 1.12.2) ships a modern installer without
//! processors. Its team asks launchers not to download it on their own, so by
//! default the installer comes from a link the user picked on its download page.

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sulfur_download::{CancellationToken, Checksum, DownloadTask, Downloader, verify_file};

use super::{
    Context, LoaderKind, LoaderRequest, LoaderVersion, fetch_json, maven_versions, open_zip, read_zip_entry,
    version_order, write_file,
};
use crate::{
    McError, Result,
    library::{self, MAVEN_CENTRAL_URL, maven_path},
    rules::RuleContext,
    version::Library,
};

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/";
const FORGE_PROMOTIONS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const CLEANROOM_MAVEN: &str = "https://repo.cleanroommc.com/releases/";

/// Records the Java version Cleanroom needs in its version JSON, which does
/// not say. Builds move to newer Java over time (0.3 needs 21, 0.7 needs 25),
/// so it is read from the class file of the main class.
fn set_cleanroom_java(ctx: &Context<'_>, name: &str) -> Result<()> {
    let dir = ctx.installer.dir();
    let path = dir.version_json(name);
    let mut json: serde_json::Value = crate::error::read_json(&path)?;
    let version: crate::version::VersionJson =
        serde_json::from_value(json.clone()).map_err(|source| McError::Json { path: path.clone(), source })?;
    let class = version.main_class.as_deref().map(|c| format!("{}.class", c.replace('.', "/")));
    let rules = RuleContext::new(ctx.installer.platform());
    let found = class.and_then(|class| {
        library::resolve_all(&version, &rules)
            .iter()
            .find_map(|lib| class_file_java(&dir.library(&lib.path), &class))
    });
    let java = found.unwrap_or(0).max(CLEANROOM_JAVA);
    json["javaVersion"] = serde_json::json!({ "component": "java-runtime-delta", "majorVersion": java });
    let text = serde_json::to_vec_pretty(&json).map_err(|source| McError::Json { path: path.clone(), source })?;
    write_file(&path, &text)
}

/// The Java version `class` inside `jar` was compiled for (class file major
/// version 65 is Java 21, 69 is Java 25), if the jar has it.
fn class_file_java(jar: &Path, class: &str) -> Option<u32> {
    let mut zip = open_zip(jar).ok()?;
    let mut header = [0u8; 8];
    zip.by_name(class).ok()?.read_exact(&mut header).ok()?;
    (header[..4] == [0xCA, 0xFE, 0xBA, 0xBE]).then(|| u32::from(u16::from_be_bytes([header[6], header[7]])).saturating_sub(44))
}

/// The game version Cleanroom runs on.
pub const CLEANROOM_GAME_VERSION: &str = "1.12.2";
/// Cleanroom needs at least Java 21, although its metadata does not say so.
const CLEANROOM_JAVA: u32 = 21;

/// Whether `url` is a Cleanroom installer on one of the project's own hosts.
pub fn is_cleanroom_installer_url(url: &str) -> bool {
    let trusted = ["https://github.com/CleanroomMC/Cleanroom/releases/download/", CLEANROOM_MAVEN];
    trusted.iter().any(|p| url.starts_with(p)) && cleanroom_installer_version(url).is_some()
}

/// `…/cleanroom-0.7.14-alpha-installer.jar` (a URL or a file path) → `0.7.14-alpha`.
pub fn cleanroom_installer_version(url: &str) -> Option<String> {
    let file = url.split(['?', '#']).next()?.rsplit(['/', '\\']).next()?;
    let version = file.strip_prefix("cleanroom-")?.strip_suffix("-installer.jar")?;
    (!version.is_empty() && version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')))
        .then(|| version.to_owned())
}

pub(super) async fn cleanroom_versions(
    downloader: &Downloader,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    if game_version != CLEANROOM_GAME_VERSION {
        return Ok(Vec::new());
    }
    let xml = downloader
        .fetch(&format!("{CLEANROOM_MAVEN}com/cleanroommc/cleanroom/maven-metadata.xml"), cancel)
        .await?;
    let mut versions: Vec<String> = maven_versions(&String::from_utf8_lossy(&xml));
    versions.sort_by_key(|v| std::cmp::Reverse(version_order(v)));
    // Every build is labelled alpha; the newest is the one the project recommends.
    Ok(versions
        .into_iter()
        .enumerate()
        .map(|(i, version)| LoaderVersion { version, stable: i == 0 })
        .collect())
}

pub(super) async fn forge_versions(
    downloader: &Downloader,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    #[derive(Deserialize)]
    struct Promotions {
        promos: HashMap<String, String>,
    }
    let xml = downloader
        .fetch(&format!("{FORGE_MAVEN}net/minecraftforge/forge/maven-metadata.xml"), cancel)
        .await?;
    let recommended = fetch_json::<Promotions>(downloader, FORGE_PROMOTIONS, cancel)
        .await
        .ok()
        .and_then(|p| p.promos.get(&format!("{game_version}-recommended")).cloned());
    let prefix = format!("{game_version}-");
    let mut versions: Vec<LoaderVersion> = maven_versions(&String::from_utf8_lossy(&xml))
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| LoaderVersion {
            stable: recommended.as_deref().is_some_and(|r| v[prefix.len()..].starts_with(r)),
            version: v,
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(version_order(&v.version)));
    Ok(versions)
}

pub(super) async fn neoforge_versions(
    downloader: &Downloader,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    let xml = downloader
        .fetch(&format!("{NEOFORGE_MAVEN}net/neoforged/neoforge/maven-metadata.xml"), cancel)
        .await?;
    let mut versions: Vec<LoaderVersion> = maven_versions(&String::from_utf8_lossy(&xml))
        .into_iter()
        .filter(|v| neoforge_game_version(v).as_deref() == Some(game_version))
        .map(|v| LoaderVersion {
            stable: !v.contains("beta") && !v.contains("alpha"),
            version: v,
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(version_order(&v.version)));
    Ok(versions)
}

/// The Minecraft version a NeoForge version targets: `21.1.65` → `1.21.1`,
/// `21.0.10-beta` → `1.21`, `26.1.2.124` → `26.1.2`, `26.3.0.99` → `26.3`.
fn neoforge_game_version(neoforge: &str) -> Option<String> {
    let numbers: Vec<u32> = neoforge
        .split(['-', '+'])
        .next()?
        .split('.')
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [major, minor, patch, _build] if *major >= 26 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        [major, minor, _] if *major < 26 => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct InstallProfile {
    /// Path of the version JSON inside the installer (modern).
    #[serde(default)]
    json: Option<String>,
    #[serde(default)]
    data: HashMap<String, DataEntry>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
    /// The whole version JSON (legacy).
    #[serde(rename = "versionInfo", default)]
    version_info: Option<serde_json::Value>,
    /// Where the universal jar goes (legacy).
    #[serde(default)]
    install: Option<LegacyInstall>,
}

#[derive(Deserialize)]
struct DataEntry {
    client: String,
}

#[derive(Deserialize)]
struct Processor {
    #[serde(default)]
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

#[derive(Deserialize)]
struct LegacyInstall {
    path: String,
    #[serde(rename = "filePath")]
    file_path: String,
}

pub(super) async fn install(ctx: &Context<'_>, request: &LoaderRequest) -> Result<()> {
    let (maven, artifact) = match request.kind {
        LoaderKind::NeoForge => (NEOFORGE_MAVEN, "net.neoforged:neoforge"),
        LoaderKind::Cleanroom => (CLEANROOM_MAVEN, "com.cleanroommc:cleanroom"),
        _ => (FORGE_MAVEN, "net.minecraftforge:forge"),
    };
    let coordinate = format!("{artifact}:{}:installer", request.loader_version);
    let relative = maven_path(&coordinate).ok_or_else(|| McError::Invalid(format!("bad version {coordinate}")))?;
    let installer = ctx.work_dir.join("installers").join(relative.rsplit('/').next().unwrap_or("installer.jar"));
    let url = match &request.installer_url {
        Some(source) if request.kind == LoaderKind::Cleanroom => {
            let local = Path::new(source);
            if local.is_absolute() && local.is_file() && cleanroom_installer_version(source).is_some() {
                // An installer the user downloaded with their browser.
                write_file(&installer, &std::fs::read(local).map_err(|e| McError::io(local, e))?)?;
                None
            } else if is_cleanroom_installer_url(source) {
                Some(source.clone())
            } else {
                return Err(McError::Invalid(format!("{source} is not a Cleanroom installer")));
            }
        }
        _ => Some(format!("{maven}{relative}")),
    };
    if let Some(url) = url {
        ctx.installer
            .batch(vec![DownloadTask::new(url, &installer)], &ctx.monitor.progress, ctx.cancel)
            .await?;
    }

    let dir = ctx.installer.dir().clone();
    let data_dir = ctx.work_dir.join("installers").join(format!("{}-data", request.name));
    let (installer_path, data_path) = (installer.clone(), data_dir.clone());
    let unpacked = tokio::task::spawn_blocking(move || unpack(&installer_path, &dir, &data_path))
        .await
        .map_err(|e| McError::Invalid(format!("installer worker failed: {e}")))??;

    match unpacked {
        Unpacked::Legacy { version } => install_legacy(ctx, request, version).await,
        Unpacked::Modern {
            profile,
            version,
            extracted,
        } => {
            let mut version = version;
            // Standalone metadata (Cleanroom 0.7 replaces vanilla's libraries
            // instead of inheriting them) still runs the vanilla jar.
            if version.get("inheritsFrom").is_none() && version.get("jar").is_none() {
                version["jar"] = request.game_version.clone().into();
            }
            install_modern(ctx, request, &installer, profile, version, extracted).await
        }
    }?;
    if request.kind == LoaderKind::Cleanroom {
        set_cleanroom_java(ctx, &request.name)?;
    }
    let _ = std::fs::remove_dir_all(&data_dir);
    Ok(())
}

enum Unpacked {
    Legacy {
        version: serde_json::Value,
    },
    Modern {
        profile: Box<InstallProfile>,
        version: serde_json::Value,
        /// Installer paths (`/data/client.lzma`) extracted to disk.
        extracted: HashMap<String, PathBuf>,
    },
}

/// Reads the installer and extracts what the install needs onto disk.
fn unpack(installer: &Path, dir: &crate::layout::GameDir, data_dir: &Path) -> Result<Unpacked> {
    let mut zip = open_zip(installer)?;
    let profile_bytes = read_zip_entry(&mut zip, "install_profile.json", installer)?;
    let profile: InstallProfile = serde_json::from_slice(&profile_bytes).map_err(|source| McError::Json {
        path: installer.join("install_profile.json"),
        source,
    })?;

    if let (Some(version), Some(install)) = (&profile.version_info, &profile.install) {
        let universal = read_zip_entry(&mut zip, &install.file_path, installer)?;
        let path = maven_path(&install.path).ok_or_else(|| McError::Invalid(format!("bad path {}", install.path)))?;
        write_file(&dir.library(&path), &universal)?;
        return Ok(Unpacked::Legacy {
            version: version.clone(),
        });
    }

    let json_name = profile.json.clone().unwrap_or_else(|| "version.json".into());
    let version_bytes = read_zip_entry(&mut zip, &json_name, installer)?;
    let version: serde_json::Value = serde_json::from_slice(&version_bytes).map_err(|source| McError::Json {
        path: installer.join(&json_name),
        source,
    })?;

    // Libraries shipped inside the installer.
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| McError::io(installer, e.into()))?;
        let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
        let Some(relative) = name.strip_prefix("maven/") else { continue };
        if entry.is_dir() || relative.is_empty() || relative.split('/').any(|p| p == "..") {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|e| McError::io(installer, e))?;
        write_file(&dir.library(relative), &bytes)?;
    }

    // Data files referenced by path, e.g. the binary patches.
    let mut extracted = HashMap::new();
    for entry in profile.data.values() {
        let value = &entry.client;
        if value.starts_with('/') && !extracted.contains_key(value) {
            let bytes = read_zip_entry(&mut zip, value, installer)?;
            let relative = value.trim_start_matches('/');
            if relative.split('/').any(|p| p == "..") {
                continue;
            }
            let out = data_dir.join(relative);
            write_file(&out, &bytes)?;
            extracted.insert(value.clone(), out);
        }
    }
    Ok(Unpacked::Modern {
        profile: Box::new(profile),
        version,
        extracted,
    })
}

async fn install_legacy(ctx: &Context<'_>, request: &LoaderRequest, mut version: serde_json::Value) -> Result<()> {
    // Old metadata points at the retired HTTP maven and lists server-only libraries.
    if let Some(libraries) = version.get_mut("libraries").and_then(|l| l.as_array_mut()) {
        libraries.retain(|lib| lib.get("clientreq").and_then(|c| c.as_bool()) != Some(false));
        for lib in libraries.iter_mut() {
            if let Some(url) = lib.get("url").and_then(|u| u.as_str())
                && url.contains("files.minecraftforge.net")
            {
                lib["url"] = FORGE_MAVEN.into();
            }
        }
    }
    if version.get("inheritsFrom").is_none() {
        version["inheritsFrom"] = request.game_version.clone().into();
        version["jar"] = request.game_version.clone().into();
    }
    let libraries: Vec<Library> = version
        .get("libraries")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|source| McError::Json {
            path: "versionInfo".into(),
            source,
        })?
        .unwrap_or_default();
    download_libraries(ctx, &libraries).await?;
    ctx.write_version(&request.name, version)
}

async fn install_modern(
    ctx: &Context<'_>,
    request: &LoaderRequest,
    installer: &Path,
    profile: Box<InstallProfile>,
    version: serde_json::Value,
    extracted: HashMap<String, PathBuf>,
) -> Result<()> {
    let profile = *profile;
    let version_libraries: Vec<Library> = version
        .get("libraries")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|source| McError::Json {
            path: installer.join("version.json"),
            source,
        })?
        .unwrap_or_default();
    let mut libraries = profile.libraries;
    libraries.extend(version_libraries);
    download_libraries(ctx, &libraries).await?;

    let dir = ctx.installer.dir();
    let path_string = |p: PathBuf| p.to_string_lossy().into_owned();
    let mut data: HashMap<String, String> = HashMap::from([
        ("SIDE".into(), "client".into()),
        ("MINECRAFT_JAR".into(), path_string(dir.version_jar(&request.game_version))),
        ("MINECRAFT_VERSION".into(), request.game_version.clone()),
        ("ROOT".into(), path_string(dir.root().to_owned())),
        ("INSTALLER".into(), path_string(installer.to_owned())),
        ("LIBRARY_DIR".into(), path_string(dir.libraries())),
    ]);
    for (key, entry) in &profile.data {
        let value = &entry.client;
        let resolved = if let Some(coordinate) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            library_path(dir, coordinate)?
        } else if let Some(literal) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            literal.to_owned()
        } else if let Some(path) = extracted.get(value) {
            path_string(path.clone())
        } else {
            value.clone()
        };
        data.insert(key.clone(), resolved);
    }

    let client_processors: Vec<&Processor> = profile
        .processors
        .iter()
        .filter(|p| p.sides.as_ref().is_none_or(|s| s.iter().any(|s| s == "client")))
        .collect();
    for (index, processor) in client_processors.iter().enumerate() {
        run_processor(ctx, processor, &data, index + 1, client_processors.len()).await?;
    }
    ctx.write_version(&request.name, version)
}

async fn download_libraries(ctx: &Context<'_>, libraries: &[Library]) -> Result<()> {
    let rules = RuleContext::new(ctx.installer.platform());
    let dir = ctx.installer.dir();
    let tasks: Vec<DownloadTask> = libraries
        .iter()
        .flat_map(|lib| library::resolve(lib, &rules))
        // Empty URLs mark files the installer ships or its processors create.
        .filter(|lib| !lib.url.is_empty())
        .map(|lib| {
            let task = DownloadTask::new(&lib.url, dir.library(&lib.path))
                .size(lib.size)
                .checksum(lib.sha1.as_deref().and_then(Checksum::sha1_hex));
            if lib.url.starts_with(MAVEN_CENTRAL_URL) {
                task
            } else {
                task.fallback(format!("{MAVEN_CENTRAL_URL}{}", lib.path))
            }
        })
        .collect();
    ctx.installer.batch(tasks, &ctx.monitor.progress, ctx.cancel).await
}

async fn run_processor(
    ctx: &Context<'_>,
    processor: &Processor,
    data: &HashMap<String, String>,
    number: usize,
    count: usize,
) -> Result<()> {
    let dir = ctx.installer.dir();
    let outputs: Vec<(PathBuf, String)> = processor
        .outputs
        .iter()
        .map(|(file, sha1)| Ok((PathBuf::from(substitute(file, data, dir)?), substitute(sha1, data, dir)?)))
        .collect::<Result<_>>()?;
    let up_to_date = !outputs.is_empty()
        && outputs.iter().all(|(file, sha1)| {
            Checksum::sha1_hex(sha1.trim_matches('\'')).is_some_and(|c| verify_file(file, &c).unwrap_or(false))
        });
    if up_to_date {
        return Ok(());
    }

    let jar = PathBuf::from(library_path(dir, &processor.jar)?);
    let main = main_class(&jar)?;
    let mut classpath = vec![jar];
    for entry in &processor.classpath {
        classpath.push(PathBuf::from(library_path(dir, entry)?));
    }
    let args = processor
        .args
        .iter()
        .map(|a| substitute(a, data, dir))
        .collect::<Result<Vec<_>>>()?;
    ctx.run_java(&classpath, &main, &args, &format!("installer step {number} of {count}"))
        .await?;

    for (file, sha1) in &outputs {
        let ok = Checksum::sha1_hex(sha1.trim_matches('\'')).is_some_and(|c| verify_file(file, &c).unwrap_or(false));
        if !ok {
            return Err(McError::Invalid(format!("installer step produced a bad {}", file.display())));
        }
    }
    Ok(())
}

/// Resolves `[maven:coordinate]` and `{DATA_KEY}` references in a processor argument.
fn substitute(arg: &str, data: &HashMap<String, String>, dir: &crate::layout::GameDir) -> Result<String> {
    if let Some(coordinate) = arg.strip_prefix('[').and_then(|a| a.strip_suffix(']')) {
        return library_path(dir, coordinate);
    }
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        match rest[start + 1..].find('}') {
            Some(end) => {
                let key = &rest[start + 1..start + 1 + end];
                match data.get(key) {
                    Some(value) => out.push_str(value),
                    None => return Err(McError::Invalid(format!("installer references unknown {{{key}}}"))),
                }
                rest = &rest[start + end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn library_path(dir: &crate::layout::GameDir, coordinate: &str) -> Result<String> {
    let path = maven_path(coordinate).ok_or_else(|| McError::Invalid(format!("bad library {coordinate}")))?;
    Ok(dir.library(&path).to_string_lossy().into_owned())
}

/// `Main-Class` from a jar's manifest.
fn main_class(jar: &Path) -> Result<String> {
    let mut zip = open_zip(jar)?;
    let manifest = read_zip_entry(&mut zip, "META-INF/MANIFEST.MF", jar)?;
    String::from_utf8_lossy(&manifest)
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|c| c.trim().to_owned()))
        .ok_or_else(|| McError::Invalid(format!("{} has no Main-Class", jar.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_cleanroom_installers() {
        let gh = "https://github.com/CleanroomMC/Cleanroom/releases/download/0.7.14-alpha/cleanroom-0.7.14-alpha-installer.jar";
        assert!(is_cleanroom_installer_url(gh));
        assert_eq!(cleanroom_installer_version(gh).as_deref(), Some("0.7.14-alpha"));
        let repo = "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/0.3.16-alpha/cleanroom-0.3.16-alpha-installer.jar";
        assert!(is_cleanroom_installer_url(repo));
        assert!(!is_cleanroom_installer_url("https://github.com/CleanroomMC/Cleanroom/releases/download/0.7.14-alpha/cleanroom-0.7.14-alpha.zip"));
        assert!(!is_cleanroom_installer_url("https://evil.example/cleanroom-1-installer.jar"));
    }

    #[test]
    fn maps_neoforge_to_game_versions() {
        assert_eq!(neoforge_game_version("21.1.65").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_game_version("21.0.10-beta").as_deref(), Some("1.21"));
        assert_eq!(neoforge_game_version("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_game_version("26.1.2.124").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_game_version("26.3.0.99").as_deref(), Some("26.3"));
        assert_eq!(neoforge_game_version("garbage"), None);
    }

    #[test]
    fn substitutes_processor_arguments() {
        let dir = crate::layout::GameDir::new(std::env::temp_dir().join("forge-test"));
        let data = HashMap::from([("SIDE".to_owned(), "client".to_owned())]);
        assert_eq!(substitute("--side={SIDE}", &data, &dir).unwrap(), "--side=client");
        let lib = substitute("[a.b:c:1:d@txt]", &data, &dir).unwrap();
        assert!(lib.ends_with(&format!("c{}1{}c-1-d.txt", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR)));
        assert!(substitute("{MISSING}", &data, &dir).is_err());
    }
}
