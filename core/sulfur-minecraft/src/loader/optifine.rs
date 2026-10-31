//! OptiFine as a standalone version, installed the way its own installer does:
//! `optifine.Patcher` diffs the client jar into a library, and LaunchWrapper
//! loads it through `optifine.OptiFineTweaker`.

use serde::Deserialize;
use sulfur_download::{CancellationToken, DownloadTask, Downloader};

use super::{Context, LoaderRequest, LoaderVersion, fetch_json, open_zip, read_zip_entry, version_order, write_file};
use crate::{McError, Result, launch::load_version, library::maven_path};

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/optifine";
const OFFICIAL: &str = "https://optifine.net/";

pub(super) async fn versions(
    downloader: &Downloader,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    #[derive(Deserialize)]
    struct Entry {
        #[serde(rename = "type")]
        kind: String,
        patch: String,
    }
    let url = format!("{BMCLAPI}/{}", sulfur_download::encode_component(game_version));
    let entries: Vec<Entry> = fetch_json(downloader, &url, cancel).await?;
    let (mut stable, mut previews): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .map(|e| LoaderVersion {
            stable: !e.patch.starts_with("pre"),
            version: format!("{}_{}", e.kind, e.patch),
        })
        .partition(|v| v.stable);
    stable.sort_by_key(|v| std::cmp::Reverse(edition_order(&v.version)));
    previews.sort_by_key(|v| std::cmp::Reverse(version_order(&v.version)));
    stable.extend(previews);
    Ok(stable)
}

/// `HD_U_J1` sorts after `HD_U_I6`.
fn edition_order(version: &str) -> (String, Vec<u64>) {
    let edition = version.rsplit('_').next().unwrap_or_default();
    (edition.chars().filter(char::is_ascii_alphabetic).collect(), version_order(edition))
}

fn file_name(game_version: &str, version: &str) -> String {
    if version.contains("_pre") {
        format!("preview_OptiFine_{game_version}_{version}.jar")
    } else {
        format!("OptiFine_{game_version}_{version}.jar")
    }
}

/// The official download page hands out a short-lived link.
async fn official_url(downloader: &Downloader, file: &str, cancel: &CancellationToken) -> Option<String> {
    let page = downloader
        .fetch(&format!("{OFFICIAL}adloadx?f={}", sulfur_download::encode_component(file)), cancel)
        .await
        .ok()?;
    let page = String::from_utf8_lossy(&page);
    let start = page.find("downloadx?f=")?;
    let link: String = page[start..].chars().take_while(|c| !matches!(c, '\'' | '"' | ' ' | '>')).collect();
    Some(format!("{OFFICIAL}{}", link.replace("&amp;", "&")))
}

pub(super) async fn install(ctx: &Context<'_>, request: &LoaderRequest) -> Result<()> {
    let game = &request.game_version;
    let version = &request.loader_version;
    let file = file_name(game, version);
    let (kind, patch) = version
        .rsplit_once('_')
        .ok_or_else(|| McError::Invalid(format!("unknown OptiFine version {version}")))?;
    let mirror = format!("{BMCLAPI}/{game}/{kind}/{patch}");
    let task = match official_url(ctx.downloader(), &file, ctx.cancel).await {
        Some(url) => DownloadTask::new(url, ctx.work_dir.join("installers").join(&file)).fallback(mirror),
        None => DownloadTask::new(mirror, ctx.work_dir.join("installers").join(&file)),
    };
    let jar = task.dest.clone();
    ctx.installer.batch(vec![task], &ctx.monitor.progress, ctx.cancel).await?;

    let dir = ctx.installer.dir().clone();
    let library = format!("optifine:OptiFine:{game}_{version}");
    let library_file = dir.library(&maven_path(&library).unwrap_or_default());

    // Bundled LaunchWrapper, else Mojang's.
    let (jar_path, lib_dir) = (jar.clone(), dir.clone());
    let (has_patcher, wrapper) = tokio::task::spawn_blocking(move || -> Result<(bool, String)> {
        let mut zip = open_zip(&jar_path)?;
        let has_patcher = zip.by_name("optifine/Patcher.class").is_ok();
        let wrapper = if let Ok(bytes) = read_zip_entry(&mut zip, "launchwrapper-2.0.jar", &jar_path) {
            write_file(&lib_dir.library(&maven_path("optifine:launchwrapper:2.0").unwrap_or_default()), &bytes)?;
            "optifine:launchwrapper:2.0".to_owned()
        } else if let Ok(text) = read_zip_entry(&mut zip, "launchwrapper-of.txt", &jar_path) {
            let v = String::from_utf8_lossy(&text).trim().to_owned();
            let bytes = read_zip_entry(&mut zip, &format!("launchwrapper-of-{v}.jar"), &jar_path)?;
            let coordinate = format!("optifine:launchwrapper-of:{v}");
            write_file(&lib_dir.library(&maven_path(&coordinate).unwrap_or_default()), &bytes)?;
            coordinate
        } else {
            "net.minecraft:launchwrapper:1.12".to_owned()
        };
        Ok((has_patcher, wrapper))
    })
    .await
    .map_err(|e| McError::Invalid(format!("installer worker failed: {e}")))??;

    if let Some(parent) = library_file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| McError::io(parent, e))?;
    }
    if has_patcher {
        let args = [dir.version_jar(game), jar.clone(), library_file.clone()]
            .map(|p| p.to_string_lossy().into_owned());
        ctx.run_java(std::slice::from_ref(&jar), "optifine.Patcher", &args, "OptiFine patcher").await?;
    } else {
        std::fs::copy(&jar, &library_file).map_err(|e| McError::io(&library_file, e))?;
    }
    if wrapper == "net.minecraft:launchwrapper:1.12" {
        let path = maven_path(&wrapper).unwrap_or_default();
        let task = DownloadTask::new(format!("{}{path}", crate::library::LIBRARIES_URL), dir.library(&path));
        ctx.installer.batch(vec![task], &ctx.monitor.progress, ctx.cancel).await?;
    }

    let parent = load_version(&dir, game)?;
    let tweak = "--tweakClass optifine.OptiFineTweaker";
    let mut json = serde_json::json!({
        "inheritsFrom": game,
        "type": "release",
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "libraries": [{ "name": library }, { "name": wrapper }],
    });
    if parent.arguments.as_ref().is_some_and(|a| !a.game.is_empty()) {
        json["arguments"] = serde_json::json!({ "game": ["--tweakClass", "optifine.OptiFineTweaker"] });
    } else {
        let base = parent.minecraft_arguments.unwrap_or_default();
        json["minecraftArguments"] = format!("{base} {tweak}").trim().into();
    }
    ctx.write_version(&request.name, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_and_orders_editions() {
        assert_eq!(file_name("1.21.1", "HD_U_J1"), "OptiFine_1.21.1_HD_U_J1.jar");
        assert_eq!(file_name("1.21.1", "HD_U_J1_pre15"), "preview_OptiFine_1.21.1_HD_U_J1_pre15.jar");
        let mut v = vec!["HD_U_I6", "HD_U_J1", "HD_U_E3"];
        v.sort_by_key(|x| std::cmp::Reverse(edition_order(x)));
        assert_eq!(v, ["HD_U_J1", "HD_U_I6", "HD_U_E3"]);
    }
}
