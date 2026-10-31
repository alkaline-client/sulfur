//! CurseForge API v1, either official (with a key) or through the MCIM mirror.

use serde::Deserialize;
use sulfur_download::CancellationToken;

use crate::{Loader, ModFile, ModHit, ModsClient, ProjectKind, Query, Result, SearchPage, Sort, Source, encode};

const OFFICIAL: &str = "https://api.curseforge.com";
const MIRROR: &str = "https://mod.mcimirror.top/curseforge";
const MINECRAFT: u32 = 432;

fn class_id(kind: ProjectKind) -> u32 {
    match kind {
        ProjectKind::Mod => 6,
        ProjectKind::ResourcePack => 12,
        ProjectKind::Shader => 6552,
        ProjectKind::DataPack => 6945,
        ProjectKind::Modpack => 4471,
    }
}

/// CurseForge's `modLoaderType`; it has no Legacy Fabric, Ornithe or Rift, and
/// lists mods for the first two as Fabric mods.
fn loader_type(loader: Loader) -> Option<u32> {
    match loader {
        Loader::Forge => Some(1),
        Loader::Fabric | Loader::LegacyFabric | Loader::Ornithe => Some(4),
        Loader::Quilt => Some(5),
        Loader::NeoForge => Some(6),
        Loader::Rift => None,
    }
}

impl ModsClient {
    fn curseforge_base(&self) -> &'static str {
        if self.curseforge_key.is_some() { OFFICIAL } else { MIRROR }
    }

    async fn curseforge_json<T: serde::de::DeserializeOwned>(&self, path: &str, cancel: &CancellationToken) -> Result<T> {
        let url = format!("{}{path}", self.curseforge_base());
        match &self.curseforge_key {
            Some(key) => self.get_json(&url, &[("x-api-key", key.as_str())], cancel).await,
            None => self.get_json(&url, &[], cancel).await,
        }
    }
}

pub(crate) fn search_path(query: &Query) -> String {
    // sortField: 1 featured, 2 popularity, 3 last updated, 6 total downloads.
    let sort = match query.sort {
        Sort::Relevance => 2,
        Sort::Downloads => 6,
        Sort::Updated => 3,
    };
    let mut path = format!(
        "/v1/mods/search?gameId={MINECRAFT}&classId={}&searchFilter={}&sortField={sort}&sortOrder=desc&index={}&pageSize={}",
        class_id(query.kind),
        encode(query.text.trim()),
        query.offset,
        query.limit.clamp(1, 50),
    );
    if let Some(v) = &query.game_version {
        path.push_str(&format!("&gameVersion={}", encode(v)));
    }
    if let Some(t) = query.loader.filter(|_| query.kind == ProjectKind::Mod).and_then(loader_type) {
        path.push_str(&format!("&modLoaderType={t}"));
    }
    path
}

pub(crate) async fn search(client: &ModsClient, query: &Query, cancel: &CancellationToken) -> Result<SearchPage> {
    #[derive(Deserialize)]
    struct Response {
        data: Vec<Mod>,
        pagination: Pagination,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pagination {
        total_count: u64,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mod {
        id: u64,
        name: String,
        #[serde(default)]
        slug: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        download_count: f64,
        logo: Option<Logo>,
        #[serde(default)]
        authors: Vec<Named>,
        #[serde(default)]
        categories: Vec<Named>,
        links: Option<Links>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Logo {
        thumbnail_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Links {
        website_url: Option<String>,
    }

    let response: Response = client.curseforge_json(&search_path(query), cancel).await?;
    // CurseForge caps searchable results at 10,000.
    let total = response.pagination.total_count.min(10_000);
    Ok(SearchPage {
        total,
        hits: response
            .data
            .into_iter()
            .map(|m| ModHit {
                source: Source::CurseForge,
                id: m.id.to_string(),
                slug: m.slug,
                page_url: m.links.and_then(|l| l.website_url).unwrap_or_default(),
                title: m.name,
                author: m.authors.into_iter().next().map(|a| a.name).unwrap_or_default(),
                description: m.summary,
                icon_url: m.logo.and_then(|l| l.thumbnail_url).filter(|u| !u.is_empty()),
                downloads: m.download_count as u64,
                categories: m.categories.into_iter().map(|c| c.name).collect(),
                also: Vec::new(),
            })
            .collect(),
    })
}

pub(crate) async fn best_file(
    client: &ModsClient,
    id: &str,
    game_version: &str,
    loader: Option<Loader>,
    cancel: &CancellationToken,
) -> Result<Option<ModFile>> {
    #[derive(Deserialize)]
    struct Response {
        data: Vec<File>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        id: u64,
        display_name: String,
        file_name: String,
        download_url: Option<String>,
        #[serde(default)]
        hashes: Vec<Hash>,
        file_length: Option<u64>,
        /// 1 release, 2 beta, 3 alpha.
        release_type: u32,
        file_date: String,
        #[serde(default)]
        dependencies: Vec<Dependency>,
    }
    #[derive(Deserialize)]
    struct Hash {
        value: String,
        algo: u32,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Dependency {
        mod_id: u64,
        /// 3 is "required".
        relation_type: u32,
    }

    let mut path = format!(
        "/v1/mods/{}/files?gameVersion={}&pageSize=50",
        encode(id),
        encode(game_version)
    );
    if let Some(t) = loader.and_then(loader_type) {
        path.push_str(&format!("&modLoaderType={t}"));
    }
    let mut files = client.curseforge_json::<Response>(&path, cancel).await?.data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let chosen = files.iter().find(|f| f.release_type == 1).or_else(|| files.first());
    Ok(chosen.map(|f| {
        // Files whose authors disabled third-party downloads have no URL, but the CDN still serves them.
        let cdn = cdn_url(f.id, &f.file_name);
        let (url, fallbacks) = match &f.download_url {
            Some(url) if !url.is_empty() => (url.clone(), vec![cdn]),
            _ => (cdn, Vec::new()),
        };
        ModFile {
            file_name: f.file_name.clone(),
            url,
            fallbacks,
            sha1: f.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone()),
            size: f.file_length,
            version_name: f.display_name.clone(),
            required: f
                .dependencies
                .iter()
                .filter(|d| d.relation_type == 3)
                .map(|d| d.mod_id.to_string())
                .collect(),
        }
    }))
}

fn cdn_url(file_id: u64, file_name: &str) -> String {
    format!(
        "https://mediafilez.forgecdn.net/files/{}/{}/{}",
        file_id / 1000,
        file_id % 1000,
        encode(file_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_paths() {
        let path = search_path(&Query {
            text: "jei".into(),
            game_version: Some("1.21.1".into()),
            loader: Some(Loader::NeoForge),
            sort: Sort::Downloads,
            kind: ProjectKind::Mod,
            offset: 20,
            limit: 20,
        });
        assert_eq!(
            path,
            "/v1/mods/search?gameId=432&classId=6&searchFilter=jei&sortField=6&sortOrder=desc&index=20&pageSize=20&gameVersion=1.21.1&modLoaderType=6"
        );
        assert_eq!(cdn_url(9322591, "a b.jar"), "https://mediafilez.forgecdn.net/files/9322/591/a%20b.jar");
    }
}
