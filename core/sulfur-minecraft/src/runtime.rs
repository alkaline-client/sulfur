//! Java runtimes that Sulfur downloads for the user.
//!
//! Builds come from Eclipse Temurin (Adoptium API), falling back to Azul Zulu
//! where Temurin has none (e.g. Java 8 on Apple silicon). Each runtime is
//! unpacked into `<java dir>/<major>-<arch>/` and found again by [`find`].

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sulfur_download::{CancellationToken, Checksum, DownloadTask};

use crate::{
    McError, Result,
    install::{InstallMonitor, InstallStage, Installer},
    java::{self, JavaInstall},
    platform::{Arch, Os, Platform},
};

/// Major versions offered for download.
pub const OFFERED: [u32; 5] = [8, 11, 17, 21, 25];

/// A runtime archive to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePackage {
    pub url: String,
    pub sha256: Option<String>,
    pub size: Option<u64>,
    pub zip: bool,
}

/// Directory of the runtime for `major` on `platform`.
pub fn runtime_dir(root: &Path, major: u32, platform: Platform) -> PathBuf {
    root.join(format!("{major}-{}", arch_name(platform.arch)))
}

/// `java` executables of every downloaded runtime, for [`java::discover_in`].
pub fn executables(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().ends_with(".partial"))
        .filter_map(|e| java_in(&e.path()))
        .collect()
}

/// The `java` executable inside an unpacked runtime.
fn java_in(dir: &Path) -> Option<PathBuf> {
    let exe = if Os::current() == Os::Windows { "java.exe" } else { "java" };
    [dir.join("bin").join(exe), dir.join("Contents/Home/bin").join(exe)]
        .into_iter()
        .find(|p| p.is_file())
}

fn arch_name(arch: Arch) -> &'static str {
    match arch {
        Arch::X86 => "x86",
        Arch::Arm64 => "aarch64",
        _ => "x64",
    }
}

/// Finds a download for `major` on `platform`.
pub async fn find(installer: &Installer, major: u32, platform: Platform, cancel: &CancellationToken) -> Result<RuntimePackage> {
    let downloader = installer.downloader();
    let (os, arch) = (adoptium_os(platform.os), arch_name(platform.arch));
    for image in ["jre", "jdk"] {
        let url = format!(
            "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type={image}&os={os}&vendor=eclipse"
        );
        match downloader.fetch(&url, cancel).await {
            Ok(bytes) => {
                if let Some(found) = parse_adoptium(&bytes) {
                    return Ok(found);
                }
            }
            Err(e @ sulfur_download::DownloadError::Cancelled) => return Err(e.into()),
            Err(e) => tracing::warn!(error = %e, "Adoptium lookup failed"),
        }
    }
    let archive = if platform.os == Os::Windows { "zip" } else { "tar.gz" };
    let zulu_os = match platform.os {
        Os::Windows => "windows",
        Os::MacOs => "macos",
        Os::Linux => "linux",
    };
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={zulu_os}&arch={arch}&archive_type={archive}&java_package_type=jre&latest=true&release_status=ga&availability_types=CA"
    );
    let bytes = downloader.fetch(&url, cancel).await?;
    parse_zulu(&bytes).ok_or(McError::NoJava(major))
}

fn adoptium_os(os: Os) -> &'static str {
    match os {
        Os::Windows => "windows",
        Os::MacOs => "mac",
        Os::Linux => "linux",
    }
}

fn parse_adoptium(bytes: &[u8]) -> Option<RuntimePackage> {
    #[derive(Deserialize)]
    struct Asset {
        binary: Binary,
    }
    #[derive(Deserialize)]
    struct Binary {
        package: Package,
    }
    #[derive(Deserialize)]
    struct Package {
        link: String,
        checksum: Option<String>,
        size: Option<u64>,
    }
    let assets: Vec<Asset> = serde_json::from_slice(bytes).ok()?;
    let package = assets.into_iter().next()?.binary.package;
    Some(RuntimePackage {
        zip: package.link.ends_with(".zip"),
        url: package.link,
        sha256: package.checksum,
        size: package.size,
    })
}

fn parse_zulu(bytes: &[u8]) -> Option<RuntimePackage> {
    #[derive(Deserialize)]
    struct Package {
        download_url: String,
        name: String,
    }
    let packages: Vec<Package> = serde_json::from_slice(bytes).ok()?;
    // Builds bundling JavaFX ("-fx-") are larger and not needed.
    let package = packages.into_iter().find(|p| !p.name.contains("-fx-"))?;
    Some(RuntimePackage {
        zip: package.download_url.ends_with(".zip"),
        url: package.download_url,
        sha256: None,
        size: None,
    })
}

/// Downloads and unpacks Java `major` into `root`, replacing an older copy.
pub async fn install(
    installer: &Installer,
    root: &Path,
    major: u32,
    monitor: &InstallMonitor,
    cancel: &CancellationToken,
) -> Result<JavaInstall> {
    let platform = installer.platform();
    monitor.set_stage(InstallStage::Preparing);
    let package = find(installer, major, platform, cancel).await?;
    let archive = root.join(format!("{major}-{}.{}", arch_name(platform.arch), if package.zip { "zip" } else { "tar.gz" }));
    let task = DownloadTask::new(&package.url, &archive)
        .size(package.size)
        .checksum(package.sha256.as_deref().and_then(Checksum::sha256_hex));
    monitor.set_stage(InstallStage::Downloading);
    installer.batch(vec![task], &monitor.progress, cancel).await?;

    monitor.set_stage(InstallStage::Extracting);
    let target = runtime_dir(root, major, platform);
    let (archive_path, target_dir, zip) = (archive.clone(), target.clone(), package.zip);
    tokio::task::spawn_blocking(move || unpack(&archive_path, &target_dir, zip))
        .await
        .map_err(|e| McError::Invalid(format!("unpack worker failed: {e}")))??;
    let _ = std::fs::remove_file(&archive);

    let exe = java_in(&target).ok_or_else(|| McError::Invalid(format!("no java executable in {}", target.display())))?;
    java::probe(&exe)
        .await
        .ok_or_else(|| McError::Invalid(format!("the downloaded Java {major} does not run")))
}

/// Unpacks into `<target>.partial`, drops the archive's top folder, then
/// swaps the result into place so a broken unpack never looks installed.
fn unpack(archive: &Path, target: &Path, zip: bool) -> Result<()> {
    let partial = target.with_extension("partial");
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).map_err(|e| McError::io(&partial, e))?;
    if zip {
        let file = File::open(archive).map_err(|e| McError::io(archive, e))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| McError::io(archive, e.into()))?;
        zip.extract(&partial).map_err(|e| McError::io(&partial, e.into()))?;
    } else {
        let file = File::open(archive).map_err(|e| McError::io(archive, e))?;
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        tar.set_preserve_permissions(true);
        // `unpack` refuses entries that would land outside `partial`.
        tar.unpack(&partial).map_err(|e| McError::io(&partial, e))?;
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&partial)
        .map_err(|e| McError::io(&partial, e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    let content = if entries.len() == 1 && entries[0].is_dir() { entries.remove(0) } else { partial.clone() };
    let _ = std::fs::remove_dir_all(target);
    std::fs::rename(&content, target).map_err(|e| McError::io(target, e))?;
    let _ = std::fs::remove_dir_all(&partial);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_responses() {
        let adoptium = br#"[{"binary":{"package":{"link":"https://x/OpenJDK8U-jre.tar.gz","checksum":"ab","size":40}}}]"#;
        assert_eq!(
            parse_adoptium(adoptium),
            Some(RuntimePackage { url: "https://x/OpenJDK8U-jre.tar.gz".into(), sha256: Some("ab".into()), size: Some(40), zip: false })
        );
        assert_eq!(parse_adoptium(b"[]"), None);
        let zulu = br#"[{"download_url":"https://c/zulu-fx-jre.zip","name":"zulu8-ca-fx-jre8"},{"download_url":"https://c/zulu-jre.zip","name":"zulu8-ca-jre8"}]"#;
        assert_eq!(parse_zulu(zulu).unwrap().url, "https://c/zulu-jre.zip");
        assert!(parse_zulu(zulu).unwrap().zip);
    }

    #[test]
    fn unpacks_and_strips_the_top_folder() {
        let root = std::env::temp_dir().join(format!("sulfur-runtime-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("a.tar.gz");
        {
            let gz = flate2::write::GzEncoder::new(File::create(&archive).unwrap(), flate2::Compression::fast());
            let mut tar = tar::Builder::new(gz);
            let mut header = tar::Header::new_gnu();
            header.set_size(2);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "jdk-21/bin/java", &b"hi"[..]).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let target = root.join("21-x64");
        unpack(&archive, &target, false).unwrap();
        assert_eq!(std::fs::read(target.join("bin/java")).unwrap(), b"hi");
        assert!(!target.with_extension("partial").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
