//! Searching and downloading mods from Modrinth and CurseForge.
//!
//! Both sources are reduced to the same small model: [`ModHit`] for search
//! results and [`ModFile`] for the file that fits a game version and loader.
//! CurseForge needs an API key; without one, requests go to the MCIM mirror,
//! which serves the same API without a key. [`merge`] combines results of
//! both sources, folding the same project on both into one hit.

mod curseforge;
mod merge;
mod modrinth;
pub mod mrpack;

pub use merge::merge;

use sulfur_download::{CancellationToken, DownloadError, Downloader};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Modrinth,
    CurseForge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Loader {
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    LegacyFabric,
    Ornithe,
    Rift,
}

/// What kind of project to look for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProjectKind {
    #[default]
    Mod,
    ResourcePack,
    DataPack,
    Shader,
    /// Modrinth modpacks (`.mrpack`), installed as a version of their own.
    Modpack,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Updated,
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub kind: ProjectKind,
    pub text: String,
    pub game_version: Option<String>,
    pub loader: Option<Loader>,
    pub sort: Sort,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModHit {
    pub source: Source,
    /// Project id on its source (Modrinth id or CurseForge numeric id).
    pub id: String,
    /// URL name of the project, e.g. `sodium`.
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub page_url: String,
    /// The same project on other sources, found by [`merge`].
    pub also: Vec<ProjectRef>,
}

/// A project on one source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRef {
    pub source: Source,
    pub id: String,
    pub page_url: String,
}

impl ModHit {
    /// This project on every source it was found on, this hit's own first.
    pub fn refs(&self) -> Vec<ProjectRef> {
        let own = ProjectRef {
            source: self.source,
            id: self.id.clone(),
            page_url: self.page_url.clone(),
        };
        std::iter::once(own).chain(self.also.iter().cloned()).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchPage {
    pub hits: Vec<ModHit>,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModFile {
    pub file_name: String,
    pub url: String,
    /// Extra download locations, tried in order after `url`.
    pub fallbacks: Vec<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub version_name: String,
    /// Projects this file requires, on the same source.
    pub required: Vec<String>,
}

/// One release of a modpack and its `.mrpack` file.
#[derive(Clone, Debug, PartialEq)]
pub struct PackVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub published: String,
    /// A release, not a beta or alpha.
    pub stable: bool,
    pub file: ModFile,
}

#[derive(Debug, thiserror::Error)]
pub enum ModsError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("unexpected response from {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    /// A modpack file that cannot be used.
    #[error("{0}")]
    Pack(String),
}

pub type Result<T, E = ModsError> = std::result::Result<T, E>;

/// Client for both mod sources. Cheap to create per request.
pub struct ModsClient {
    downloader: Downloader,
    curseforge_key: Option<String>,
}

impl ModsClient {
    /// `curseforge_key` uses the official CurseForge API; without it the MCIM mirror is used.
    pub fn new(downloader: Downloader, curseforge_key: Option<String>) -> Self {
        Self {
            downloader,
            curseforge_key: curseforge_key.filter(|k| !k.trim().is_empty()),
        }
    }

    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    pub async fn search(&self, source: Source, query: &Query, cancel: &CancellationToken) -> Result<SearchPage> {
        match source {
            Source::Modrinth => modrinth::search(self, query, cancel).await,
            Source::CurseForge => curseforge::search(self, query, cancel).await,
        }
    }

    /// Searches several sources at once and merges the results. A source
    /// that fails is skipped while another answers; `total` is the largest
    /// total of any source, as each is paged with the same offset.
    pub async fn search_many(&self, sources: &[Source], query: &Query, cancel: &CancellationToken) -> Result<SearchPage> {
        let pages = futures_util::future::join_all(sources.iter().map(|s| self.search(*s, query, cancel))).await;
        let mut lists = Vec::new();
        let mut total = 0;
        let mut first_error = None;
        for page in pages {
            match page {
                Ok(page) => {
                    total = total.max(page.total);
                    lists.push(page.hits);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mod search failed on one source");
                    first_error.get_or_insert(e);
                }
            }
        }
        match first_error {
            Some(e) if lists.is_empty() => Err(e),
            _ => Ok(SearchPage {
                hits: merge(Vec::new(), lists),
                total,
            }),
        }
    }

    /// The newest file of project `id` for this game version and loader
    /// (releases preferred over betas), if any. The loader only applies to mods.
    pub async fn best_file(
        &self,
        source: Source,
        kind: ProjectKind,
        id: &str,
        game_version: &str,
        loader: Option<Loader>,
        cancel: &CancellationToken,
    ) -> Result<Option<ModFile>> {
        let loader = loader.filter(|_| kind == ProjectKind::Mod);
        match source {
            Source::Modrinth => modrinth::best_file(self, kind, id, game_version, loader, cancel).await,
            Source::CurseForge => curseforge::best_file(self, id, game_version, loader, cancel).await,
        }
    }

    /// Releases of Modrinth modpack `id`, newest first, with their `.mrpack` files.
    pub async fn pack_versions(&self, id: &str, cancel: &CancellationToken) -> Result<Vec<PackVersion>> {
        modrinth::pack_versions(self, id, cancel).await
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        cancel: &CancellationToken,
    ) -> Result<T> {
        let bytes = self.downloader.fetch_with_headers(url, headers, cancel).await?;
        serde_json::from_slice(&bytes).map_err(|source| ModsError::Json {
            url: url.to_owned(),
            source,
        })
    }
}

pub(crate) use sulfur_download::encode_component as encode;
