//! Modrinth API v2 (no key needed).

use serde::Deserialize;
use sulfur_download::CancellationToken;

use crate::{Loader, ModFile, ModHit, ModsClient, PackVersion, ProjectKind, Query, Result, SearchPage, Sort, Source, encode};

const API: &str = "https://api.modrinth.com/v2";

/// Loaders whose mods run on `loader`, its own first. Quilt also runs Fabric mods.
fn loader_names(loader: Loader) -> &'static [&'static str] {
    match loader {
        Loader::Fabric => &["fabric"],
        Loader::Forge => &["forge"],
        Loader::NeoForge => &["neoforge"],
        Loader::Quilt => &["quilt", "fabric"],
        Loader::LegacyFabric => &["legacy-fabric"],
        Loader::Ornithe => &["ornithe"],
        Loader::Rift => &["rift"],
    }
}

fn project_type(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Mod => "mod",
        ProjectKind::ResourcePack => "resourcepack",
        ProjectKind::DataPack => "datapack",
        ProjectKind::Shader => "shader",
        ProjectKind::Modpack => "modpack",
    }
}

/// `["a","b"]` as JSON.
fn json_list(items: &[&str]) -> String {
    format!("[{}]", items.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(","))
}

pub(crate) fn search_url(query: &Query) -> String {
    let mut facets = vec![format!(r#"["project_type:{}"]"#, project_type(query.kind))];
    if let Some(v) = &query.game_version {
        facets.push(format!(r#"["versions:{v}"]"#));
    }
    if let Some(l) = query.loader.filter(|_| query.kind == ProjectKind::Mod) {
        let any: Vec<String> = loader_names(l).iter().map(|n| format!("\"categories:{n}\"")).collect();
        facets.push(format!("[{}]", any.join(",")));
    }
    if let Some(c) = &query.category {
        facets.push(format!(r#"["categories:{c}"]"#));
    }
    let index = match query.sort {
        Sort::Relevance => "relevance",
        Sort::Downloads => "downloads",
        Sort::Updated => "updated",
    };
    format!(
        "{API}/search?query={}&facets={}&index={index}&offset={}&limit={}",
        encode(query.text.trim()),
        encode(&format!("[{}]", facets.join(","))),
        query.offset,
        query.limit.clamp(1, 100),
    )
}

pub(crate) async fn search(client: &ModsClient, query: &Query, cancel: &CancellationToken) -> Result<SearchPage> {
    #[derive(Deserialize)]
    struct Response {
        hits: Vec<Hit>,
        total_hits: u64,
    }
    #[derive(Deserialize)]
    struct Hit {
        project_id: String,
        slug: String,
        title: String,
        #[serde(default)]
        author: String,
        #[serde(default)]
        description: String,
        icon_url: Option<String>,
        #[serde(default)]
        downloads: u64,
        #[serde(default)]
        display_categories: Vec<String>,
    }
    let response: Response = client.get_json(&search_url(query), &[], cancel).await?;
    let page = project_type(query.kind);
    Ok(SearchPage {
        total: response.total_hits,
        hits: response
            .hits
            .into_iter()
            .map(|h| ModHit {
                source: Source::Modrinth,
                page_url: format!("https://modrinth.com/{page}/{}", h.slug),
                id: h.project_id,
                slug: h.slug,
                title: h.title,
                author: h.author,
                description: h.description,
                icon_url: h.icon_url.filter(|u| !u.is_empty()),
                downloads: h.downloads,
                categories: h.display_categories,
                also: Vec::new(),
            })
            .collect(),
    })
}

pub(crate) async fn best_file(
    client: &ModsClient,
    kind: ProjectKind,
    id: &str,
    game_version: &str,
    loader: Option<Loader>,
    cancel: &CancellationToken,
) -> Result<Option<ModFile>> {
    #[derive(Deserialize)]
    struct Version {
        version_number: String,
        version_type: String,
        files: Vec<File>,
        #[serde(default)]
        dependencies: Vec<Dependency>,
    }
    #[derive(Deserialize)]
    struct File {
        url: String,
        filename: String,
        #[serde(default)]
        primary: bool,
        size: Option<u64>,
        #[serde(default)]
        hashes: std::collections::HashMap<String, String>,
    }
    #[derive(Deserialize)]
    struct Dependency {
        project_id: Option<String>,
        dependency_type: String,
    }

    let mut url = format!(
        "{API}/project/{}/version?game_versions={}",
        encode(id),
        encode(&format!(r#"["{game_version}"]"#))
    );
    let loaders = match (kind, loader) {
        (ProjectKind::Mod, Some(l)) => Some(loader_names(l)),
        (ProjectKind::DataPack, _) => Some(&["datapack"][..]),
        _ => None,
    };
    if let Some(loaders) = loaders {
        url.push_str(&format!("&loaders={}", encode(&json_list(loaders))));
    }
    // Newest first.
    let versions: Vec<Version> = client.get_json(&url, &[], cancel).await?;
    let chosen = versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first());
    Ok(chosen.and_then(|v| {
        let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first())?;
        Some(ModFile {
            file_name: file.filename.clone(),
            url: file.url.clone(),
            fallbacks: Vec::new(),
            sha1: file.hashes.get("sha1").cloned(),
            size: file.size,
            version_name: v.version_number.clone(),
            required: v
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "required")
                .filter_map(|d| d.project_id.clone())
                .collect(),
        })
    }))
}

pub(crate) async fn pack_versions(client: &ModsClient, id: &str, cancel: &CancellationToken) -> Result<Vec<PackVersion>> {
    #[derive(Deserialize)]
    struct Version {
        id: String,
        name: String,
        version_number: String,
        version_type: String,
        #[serde(default)]
        game_versions: Vec<String>,
        #[serde(default)]
        loaders: Vec<String>,
        date_published: String,
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        url: String,
        filename: String,
        #[serde(default)]
        primary: bool,
        size: Option<u64>,
        #[serde(default)]
        hashes: std::collections::HashMap<String, String>,
    }
    let versions: Vec<Version> = client.get_json(&format!("{API}/project/{}/version", encode(id)), &[], cancel).await?;
    Ok(versions
        .into_iter()
        .filter_map(|v| {
            let file = v
                .files
                .iter()
                .find(|f| f.primary && f.filename.ends_with(".mrpack"))
                .or_else(|| v.files.iter().find(|f| f.filename.ends_with(".mrpack")))?;
            Some(PackVersion {
                file: ModFile {
                    file_name: file.filename.clone(),
                    url: file.url.clone(),
                    fallbacks: Vec::new(),
                    sha1: file.hashes.get("sha1").cloned(),
                    size: file.size,
                    version_name: v.version_number.clone(),
                    required: Vec::new(),
                },
                id: v.id,
                name: v.name,
                version_number: v.version_number,
                game_versions: v.game_versions,
                loaders: v.loaders,
                published: v.date_published,
                stable: v.version_type == "release",
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_urls() {
        let url = search_url(&Query {
            text: "sodium extra".into(),
            game_version: Some("1.21.1".into()),
            loader: Some(Loader::Fabric),
            limit: 20,
            ..Default::default()
        });
        assert!(url.starts_with("https://api.modrinth.com/v2/search?query=sodium%20extra&facets="));
        assert!(url.contains("%22versions%3A1.21.1%22"));
        assert!(url.contains("%22categories%3Afabric%22"));
        assert!(url.contains("project_type%3Amod"));
        let quilt = search_url(&Query { loader: Some(Loader::Quilt), ..Default::default() });
        assert!(quilt.contains(&encode(r#"["categories:quilt","categories:fabric"]"#)));
        let shaders = search_url(&Query { kind: ProjectKind::Shader, loader: Some(Loader::Fabric), ..Default::default() });
        assert!(shaders.contains("project_type%3Ashader") && !shaders.contains("categories"));
        assert!(url.ends_with("&index=relevance&offset=0&limit=20"));
        let packs = search_url(&Query { kind: ProjectKind::ResourcePack, category: Some("16x".into()), ..Default::default() });
        assert!(packs.contains(&encode(r#"["categories:16x"]"#)));
    }
}
