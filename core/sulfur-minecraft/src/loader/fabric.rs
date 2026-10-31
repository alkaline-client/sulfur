//! Fabric and its relatives (Legacy Fabric, Ornithe, Quilt): each runs a meta
//! server that serves a complete version JSON per loader version.

use serde::Deserialize;
use sulfur_download::{CancellationToken, DownloadTask, Downloader, encode_component};

use super::{Context, LoaderKind, LoaderRequest, LoaderVersion, fetch_json, version_order};
use crate::{
    Result,
    library::{self, MAVEN_CENTRAL_URL},
    rules::RuleContext,
    version::VersionJson,
};

/// Base URL of the loader endpoints; `/{game}` lists versions and
/// `/{game}/{loader}/profile/json` serves the version JSON.
fn meta(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::LegacyFabric => "https://meta.legacyfabric.net/v2/versions/loader",
        // Ornithe's second-generation intermediary is the one its mods target.
        LoaderKind::Ornithe => "https://meta.ornithemc.net/v3/versions/gen2/fabric-loader",
        LoaderKind::Quilt => "https://meta.quiltmc.org/v3/versions/loader",
        _ => "https://meta.fabricmc.net/v2/versions/loader",
    }
}

pub(super) async fn versions(
    downloader: &Downloader,
    kind: LoaderKind,
    game_version: &str,
    cancel: &CancellationToken,
) -> Result<Vec<LoaderVersion>> {
    #[derive(Deserialize)]
    struct Entry {
        loader: Loader,
    }
    #[derive(Deserialize)]
    struct Loader {
        version: String,
        /// Missing on Quilt, whose unstable builds carry a `-beta` suffix instead.
        stable: Option<bool>,
    }
    let url = format!("{}/{}", meta(kind), encode_component(game_version));
    let entries: Vec<Entry> = fetch_json(downloader, &url, cancel).await?;
    let mut versions: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| LoaderVersion {
            stable: e.loader.stable.unwrap_or(!e.loader.version.contains('-')),
            version: e.loader.version,
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(version_order(&v.version)));
    versions.dedup_by(|a, b| a.version == b.version);
    Ok(versions)
}

pub(super) async fn install(ctx: &Context<'_>, request: &LoaderRequest) -> Result<()> {
    let url = format!(
        "{}/{}/{}/profile/json",
        meta(request.kind),
        encode_component(&request.game_version),
        encode_component(&request.loader_version)
    );
    let mut json: serde_json::Value = fetch_json(ctx.downloader(), &url, ctx.cancel).await?;
    // Ornithe names its parent `<game>-vanilla`; ours is the plain vanilla id.
    json["inheritsFrom"] = request.game_version.clone().into();
    let profile: VersionJson = serde_json::from_value(json.clone()).map_err(|source| crate::McError::Json {
        path: url.clone().into(),
        source,
    })?;

    let rules = RuleContext::new(ctx.installer.platform());
    let dir = ctx.installer.dir();
    let tasks = library::resolve_all(&profile, &rules)
        .into_iter()
        .map(|lib| {
            let fallback = format!("{MAVEN_CENTRAL_URL}{}", lib.path);
            DownloadTask::new(&lib.url, dir.library(&lib.path))
                .size(lib.size)
                .checksum(lib.sha1.as_deref().and_then(sulfur_download::Checksum::sha1_hex))
                .fallback(fallback)
        })
        .collect();
    ctx.installer.batch(tasks, &ctx.monitor.progress, ctx.cancel).await?;
    ctx.write_version(&request.name, json)
}
