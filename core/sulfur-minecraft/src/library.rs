//! Resolving library entries into concrete files for this platform.

use crate::{
    platform::{Arch, Os},
    rules::{self, RuleContext},
    version::{Artifact, Library, VersionJson},
};

pub const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
pub const MAVEN_CENTRAL_URL: &str = "https://repo1.maven.org/maven2/";

/// A library file to download and use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLibrary {
    pub name: String,
    /// Path below `libraries/`, `/`-separated.
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Legacy native archive to extract into the natives directory instead of
    /// putting it on the classpath.
    pub extract: bool,
    pub extract_exclude: Vec<String>,
}

/// Maven coordinate `group:artifact:version[:classifier][@ext]` to a repository path.
pub fn maven_path(coordinate: &str) -> Option<String> {
    let (coordinate, ext) = coordinate.split_once('@').unwrap_or((coordinate, "jar"));
    let mut parts = coordinate.split(':');
    let (group, artifact, version) = (parts.next()?, parts.next()?, parts.next()?);
    let classifier = parts.next().map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}",
        group.replace('.', "/")
    ))
}

/// All libraries of a version that apply to `ctx`, in classpath order and
/// without duplicate paths.
pub fn resolve_all(version: &VersionJson, ctx: &RuleContext) -> Vec<ResolvedLibrary> {
    let mut seen = std::collections::HashSet::new();
    version
        .libraries
        .iter()
        .flat_map(|lib| resolve(lib, ctx))
        .filter(|lib| seen.insert(lib.path.clone()))
        .collect()
}

pub fn resolve(lib: &Library, ctx: &RuleContext) -> Vec<ResolvedLibrary> {
    if !rules::allowed(&lib.rules, ctx) {
        return Vec::new();
    }
    let native_classifier = lib
        .natives
        .as_ref()
        .and_then(|n| n.get(ctx.platform.os.mojang_name()))
        .map(|c| c.replace("${arch}", ctx.platform.arch.bits()));
    let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();

    let mut out = Vec::new();
    match &lib.downloads {
        Some(downloads) => {
            if let Some(artifact) = &downloads.artifact {
                out.extend(from_artifact(&lib.name, artifact, false, Vec::new()));
            }
            if let Some(classifier) = &native_classifier
                && let Some(artifact) = downloads.classifiers.get(classifier)
            {
                // Without its own path, a classifier artifact lives at `name:classifier`.
                out.extend(from_artifact(&format!("{}:{classifier}", lib.name), artifact, true, exclude));
            }
        }
        None => {
            let base = lib.url.as_deref().unwrap_or(LIBRARIES_URL);
            let base = if base.ends_with('/') { base.to_owned() } else { format!("{base}/") };
            let coordinate = match &native_classifier {
                Some(classifier) => format!("{}:{classifier}", lib.name),
                None => lib.name.clone(),
            };
            if let Some(path) = maven_path(&coordinate) {
                out.push(ResolvedLibrary {
                    name: lib.name.clone(),
                    url: format!("{base}{path}"),
                    path,
                    sha1: native_classifier.is_none().then(|| lib.sha1.clone()).flatten(),
                    size: native_classifier.is_none().then_some(lib.size).flatten(),
                    extract: native_classifier.is_some(),
                    extract_exclude: exclude,
                });
            }
        }
    }
    out.into_iter().map(|l| patch_for_platform(l, ctx)).collect()
}

fn from_artifact(name: &str, artifact: &Artifact, extract: bool, exclude: Vec<String>) -> Option<ResolvedLibrary> {
    let path = artifact.path.clone().or_else(|| maven_path(name))?;
    Some(ResolvedLibrary {
        name: name.to_owned(),
        path,
        url: artifact.url.clone(),
        sha1: artifact.sha1.clone(),
        size: artifact.size,
        extract,
        extract_exclude: exclude,
    })
}

/// Mojang ships x86-64 LWJGL natives only for Linux. On Linux arm64, swap in
/// LWJGL's official arm64 builds from Maven Central (LWJGL 3.3+ publishes them).
fn patch_for_platform(lib: ResolvedLibrary, ctx: &RuleContext) -> ResolvedLibrary {
    if ctx.platform.os != Os::Linux || ctx.platform.arch != Arch::Arm64 {
        return lib;
    }
    let is_lwjgl3_linux_natives = lib.name.starts_with("org.lwjgl:")
        && lib.name.ends_with(":natives-linux")
        && !lib.extract;
    if !is_lwjgl3_linux_natives {
        return lib;
    }
    let name = format!("{}-arm64", lib.name);
    let Some(path) = maven_path(&name) else {
        return lib;
    };
    ResolvedLibrary {
        url: format!("{MAVEN_CENTRAL_URL}{path}"),
        name,
        path,
        sha1: None,
        size: None,
        ..lib
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Platform;

    #[test]
    fn natives_without_a_path_keep_their_classifier() {
        // Cleanroom's metadata: the same coordinate once per platform, natives without `path`.
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"x","libraries":[
              {"name":"org.lwjgl:lwjgl:3.4.3","downloads":{"artifact":{"path":"org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3.jar","url":"u"}}},
              {"name":"org.lwjgl:lwjgl:3.4.3","natives":{"linux":"natives-linux-arm64"},"extract":{"exclude":["META-INF/"]},
               "rules":[{"action":"allow","os":{"arch":"arm64","name":"linux"}}],
               "downloads":{"classifiers":{"natives-linux-arm64":{"url":"n"}}}}]}"#,
        )
        .unwrap();
        let arm = RuleContext::new(Platform { os: Os::Linux, arch: Arch::Arm64 });
        let libs = resolve_all(&version, &arm);
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[1].path, "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux-arm64.jar");
        assert!(libs[1].extract);
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.4.3:natives-linux").unwrap(),
            "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux.jar"
        );
        assert_eq!(maven_path("a.b:c:1@zip").unwrap(), "a/b/c/1/c-1.zip");
        assert!(maven_path("broken").is_none());
    }

    #[test]
    fn swaps_lwjgl_natives_on_linux_arm64() {
        let lib: Library = serde_json::from_str(
            r#"{"name":"org.lwjgl:lwjgl:3.4.3:natives-linux",
                "downloads":{"artifact":{"path":"org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux.jar",
                "url":"https://libraries.minecraft.net/x.jar","sha1":"00","size":1}},
                "rules":[{"action":"allow","os":{"name":"linux"}}]}"#,
        )
        .unwrap();
        let arm = RuleContext::new(Platform { os: Os::Linux, arch: Arch::Arm64 });
        let r = &resolve(&lib, &arm)[0];
        assert_eq!(r.path, "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux-arm64.jar");
        assert!(r.url.starts_with(MAVEN_CENTRAL_URL));
        let x64 = RuleContext::new(Platform { os: Os::Linux, arch: Arch::X86_64 });
        assert_eq!(resolve(&lib, &x64)[0].sha1.as_deref(), Some("00"));
        let win = RuleContext::new(Platform { os: Os::Windows, arch: Arch::X86_64 });
        assert!(resolve(&lib, &win).is_empty());
    }

    #[test]
    fn legacy_natives_are_extracted() {
        let lib: Library = serde_json::from_str(
            r#"{"name":"org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
                "natives":{"linux":"natives-linux","windows":"natives-windows-${arch}"},
                "extract":{"exclude":["META-INF/"]},
                "downloads":{"classifiers":{
                  "natives-linux":{"path":"l.jar","url":"https://x/l.jar"},
                  "natives-windows-64":{"path":"w.jar","url":"https://x/w.jar"}}}}"#,
        )
        .unwrap();
        let win = RuleContext::new(Platform { os: Os::Windows, arch: Arch::X86_64 });
        let r = resolve(&lib, &win);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "w.jar");
        assert!(r[0].extract);
        assert_eq!(r[0].extract_exclude, ["META-INF/"]);
    }
}
