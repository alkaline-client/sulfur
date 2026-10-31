//! Rift, the Mixin-based loader for Minecraft 1.13.2. The project is archived:
//! its jars live on GitHub releases and carry their own version JSON
//! (`profile.json`), whose Maven repositories are gone, so the libraries are
//! pointed at repositories that still serve them.

use serde_json::json;
use sulfur_download::{CancellationToken, DownloadTask, Downloader};

use super::{Context, LoaderRequest, LoaderVersion, open_zip, read_zip_entry};
use crate::{McError, Result, library, rules::RuleContext, version::VersionJson};

/// The only game version Rift's final builds target.
pub(super) const GAME_VERSION: &str = "1.13.2";

/// Final builds for 1.13.2, newest first. The list will not change.
const BUILDS: [&str; 10] = [
    "1.0.4-106", "1.0.4-105", "1.0.4-87", "1.0.4-86", "1.0.4-77", "1.0.4-74", "1.0.4-72", "1.0.4-66", "1.0.4-65",
    "1.0.4-64",
];

const RELEASES: &str = "https://github.com/DimensionalDevelopment/Rift/releases/download";
const MIXIN_URL: &str = "https://repo.spongepowered.org/repository/maven-public/org/spongepowered/mixin/0.7.11-SNAPSHOT/mixin-0.7.11-20180703.121122-1.jar";

pub(super) async fn versions(
    _downloader: &Downloader,
    game_version: &str,
    _cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    if game_version != GAME_VERSION {
        return Ok(Vec::new());
    }
    Ok(BUILDS
        .iter()
        .enumerate()
        .map(|(i, v)| LoaderVersion {
            version: (*v).to_owned(),
            stable: i == 0,
        })
        .collect())
}

pub(super) async fn install(ctx: &Context<'_>, request: &LoaderRequest) -> Result<()> {
    let build = &request.loader_version;
    if !BUILDS.contains(&build.as_str()) {
        return Err(McError::Invalid(format!("unknown Rift build {build}")));
    }
    let jar_url = format!("{RELEASES}/v{build}/Rift-{build}.jar");
    let jar_path = format!("org/dimdev/rift/{build}/rift-{build}.jar");
    let dir = ctx.installer.dir();
    ctx.installer
        .batch(
            vec![DownloadTask::new(&jar_url, dir.library(&jar_path))],
            &ctx.monitor.progress,
            ctx.cancel,
        )
        .await?;

    let jar = dir.library(&jar_path);
    let profile: serde_json::Value = {
        let mut zip = open_zip(&jar)?;
        let bytes = read_zip_entry(&mut zip, "profile.json", &jar)?;
        serde_json::from_slice(&bytes).map_err(|source| McError::Json { path: jar.clone(), source })?
    };
    if profile.get("inheritsFrom").and_then(|v| v.as_str()) != Some(request.game_version.as_str()) {
        return Err(McError::Invalid(format!("Rift {build} is not made for Minecraft {}", request.game_version)));
    }
    let version = version_json(&request.game_version, build, &jar_url, &jar_path, &profile);
    let parsed: VersionJson = serde_json::from_value(version.clone()).map_err(|source| McError::Json {
        path: jar.clone(),
        source,
    })?;
    let rules = RuleContext::new(ctx.installer.platform());
    let tasks = library::resolve_all(&parsed, &rules)
        .into_iter()
        .map(|lib| DownloadTask::new(&lib.url, dir.library(&lib.path)).size(lib.size))
        .collect();
    ctx.installer.batch(tasks, &ctx.monitor.progress, ctx.cancel).await?;
    ctx.write_version(&request.name, version)
}

/// Rift's own profile with libraries that can still be downloaded.
fn version_json(game: &str, build: &str, jar_url: &str, jar_path: &str, profile: &serde_json::Value) -> serde_json::Value {
    let artifact = |name: &str, path: &str, url: &str| json!({ "name": name, "downloads": { "artifact": { "path": path, "url": url } } });
    let central = "https://repo1.maven.org/maven2/";
    json!({
        "id": format!("{game}-rift-{build}"),
        "inheritsFrom": game,
        "type": "release",
        "mainClass": profile.get("mainClass").cloned().unwrap_or_else(|| "net.minecraft.launchwrapper.Launch".into()),
        "arguments": profile.get("arguments").cloned().unwrap_or_else(|| json!({
            "game": ["--tweakClass", "org.dimdev.riftloader.launch.RiftLoaderClientTweaker"]
        })),
        "libraries": [
            artifact(&format!("org.dimdev:rift:{build}"), jar_path, jar_url),
            artifact(
                "org.spongepowered:mixin:0.7.11-SNAPSHOT",
                "org/spongepowered/mixin/0.7.11-SNAPSHOT/mixin-0.7.11-SNAPSHOT.jar",
                MIXIN_URL,
            ),
            { "name": "org.ow2.asm:asm:6.2", "url": central },
            { "name": "org.ow2.asm:asm-commons:6.2", "url": central },
            { "name": "org.ow2.asm:asm-tree:6.2", "url": central },
            { "name": "net.minecraft:launchwrapper:1.12" },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_dead_repositories() {
        let profile = json!({ "mainClass": "net.minecraft.launchwrapper.Launch" });
        let v = version_json("1.13.2", "1.0.4-106", "https://x/rift.jar", "org/dimdev/rift/1.0.4-106/rift-1.0.4-106.jar", &profile);
        let parsed: VersionJson = serde_json::from_value(v).unwrap();
        let libs = library::resolve_all(&parsed, &RuleContext::new(crate::platform::Platform::current()));
        assert_eq!(libs.len(), 6);
        assert!(libs.iter().all(|l| !l.url.contains("dimdev.org") && !l.url.starts_with("http:")));
        assert!(libs.iter().any(|l| l.url == MIXIN_URL));
    }
}
