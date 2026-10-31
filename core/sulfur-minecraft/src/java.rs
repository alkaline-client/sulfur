//! Finding Java runtimes installed on this machine.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::platform::{Arch, Os, Platform};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaInstall {
    /// The `java` executable.
    pub path: PathBuf,
    /// Full version, e.g. `21.0.4` or `1.8.0_412`.
    pub version: String,
    pub major: u32,
    /// `os.arch` as reported by the runtime, e.g. `amd64` or `aarch64`.
    pub arch: String,
    pub vendor: String,
}

impl JavaInstall {
    /// Whether this runtime runs natively on `platform` (no emulation).
    pub fn is_native_for(&self, platform: Platform) -> bool {
        match platform.arch {
            Arch::X86_64 => matches!(self.arch.as_str(), "amd64" | "x86_64"),
            Arch::Arm64 => self.arch == "aarch64",
            Arch::X86 => matches!(self.arch.as_str(), "x86" | "i386"),
            Arch::Other => true,
        }
    }
}

/// Probes all candidate locations concurrently.
pub async fn discover() -> Vec<JavaInstall> {
    discover_in(Vec::new()).await
}

/// Like [`discover`], also probing `extra` executables (e.g. downloaded runtimes).
pub async fn discover_in(extra: Vec<PathBuf>) -> Vec<JavaInstall> {
    let mut paths = extra;
    paths.extend(candidates());
    let probes = paths.into_iter().map(|path| async move { probe(&path).await });
    let mut seen = HashSet::new();
    let mut found: Vec<_> = futures_util::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .filter(|j| seen.insert(std::fs::canonicalize(&j.path).unwrap_or_else(|_| j.path.clone())))
        .collect();
    found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.path.cmp(&b.path)));
    found
}

/// Picks the best runtime for a game needing Java `required`: the exact major
/// version if available, otherwise the closest newer one; native builds first.
pub fn pick(installs: &[JavaInstall], required: u32, platform: Platform) -> Option<&JavaInstall> {
    installs
        .iter()
        .filter(|j| j.major >= required)
        .min_by_key(|j| (j.major != required, !j.is_native_for(platform), j.major))
}

/// The installed runtime that fits a game needing Java `required` without
/// compromise: the exact major version, or for Java 16+ games (which run fine
/// on newer Java) any newer one. Older games, and especially their mod
/// loaders, often break on newer Java, so they only take an exact match.
pub fn pick_compatible(installs: &[JavaInstall], required: u32, platform: Platform) -> Option<&JavaInstall> {
    pick(installs, required, platform).filter(|j| j.major == required || required >= 16)
}

/// Runs `java -XshowSettings:properties -version` and parses the output.
pub async fn probe(path: &Path) -> Option<JavaInstall> {
    let mut cmd = tokio::process::Command::new(path);
    cmd.args(["-XshowSettings:properties", "-version"])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let output = tokio::time::timeout(Duration::from_secs(10), cmd.output()).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    let property = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.trim().split_once(" = ")?;
            (k == key).then(|| v.trim().to_owned())
        })
    };
    let version = property("java.version")?;
    Some(JavaInstall {
        path: path.to_owned(),
        major: major_version(&version)?,
        arch: property("os.arch").unwrap_or_default(),
        vendor: property("java.vendor").unwrap_or_default(),
        version,
    })
}

/// `1.8.0_412` → 8, `21.0.4` → 21, `25` → 25.
pub fn major_version(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    match parts.next()?.parse().ok()? {
        1 => parts.next()?.parse().ok(),
        major => Some(major),
    }
}

fn candidates() -> Vec<PathBuf> {
    let exe = if Os::current() == Os::Windows { "java.exe" } else { "java" };
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        out.push(PathBuf::from(home).join("bin").join(exe));
    }
    if let Some(path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&path).map(|dir| dir.join(exe)));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut roots: Vec<(PathBuf, &str)> = Vec::new();
    match Os::current() {
        Os::Linux => {
            roots.push(("/usr/lib/jvm".into(), "bin"));
            roots.push(("/usr/lib64/jvm".into(), "bin"));
            roots.push(("/opt".into(), "bin"));
        }
        Os::MacOs => {
            roots.push(("/Library/Java/JavaVirtualMachines".into(), "Contents/Home/bin"));
            if let Some(home) = &home {
                roots.push((home.join("Library/Java/JavaVirtualMachines"), "Contents/Home/bin"));
            }
        }
        Os::Windows => {
            for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Some(pf) = std::env::var_os(var).map(PathBuf::from) {
                    for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto"] {
                        roots.push((pf.join(vendor), "bin"));
                    }
                }
            }
        }
    }
    if let Some(home) = &home {
        roots.push((home.join(".jdks"), "bin"));
        roots.push((home.join(".sdkman/candidates/java"), "bin"));
    }
    for (root, bin) in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let mut java = entry.path();
            java.extend(bin.split('/'));
            out.push(java.join(exe));
        }
    }
    let mut seen = HashSet::new();
    out.retain(|p| p.is_file() && seen.insert(p.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_major_versions() {
        assert_eq!(major_version("1.8.0_412"), Some(8));
        assert_eq!(major_version("21.0.4"), Some(21));
        assert_eq!(major_version("25"), Some(25));
        assert_eq!(major_version("25-ea"), Some(25));
    }

    #[test]
    fn prefers_exact_then_newer() {
        let p = Platform { os: Os::Linux, arch: Arch::X86_64 };
        let j = |major: u32, arch: &str| JavaInstall {
            path: format!("/j{major}{arch}").into(),
            version: major.to_string(),
            major,
            arch: arch.into(),
            vendor: String::new(),
        };
        let all = [j(25, "amd64"), j(17, "amd64"), j(21, "aarch64"), j(21, "amd64")];
        assert_eq!(pick(&all, 21, p).unwrap(), &all[3]);
        assert_eq!(pick(&all, 18, p).unwrap(), &all[3]);
        assert_eq!(pick(&all, 8, p).unwrap(), &all[1]);
        assert!(pick(&all, 26, p).is_none());
        assert_eq!(pick_compatible(&all, 18, p).unwrap(), &all[3]);
        assert!(pick_compatible(&all, 8, p).is_none());
        assert_eq!(pick_compatible(&all, 17, p).unwrap(), &all[1]);
    }
}
