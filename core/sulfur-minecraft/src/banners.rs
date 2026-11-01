//! Artwork for major updates.
//!
//! First choice is the official key art, without logos, collected in the
//! Minecraft Update Artwork archive (github.com/TQNL/minecraft-update-artwork),
//! which also has small previews for the version grid. Updates it lacks use
//! the official launcher's patch notes (key art for every version since 1.13),
//! then the lead image of their Minecraft Wiki page. April Fools uses the
//! launcher banner of the newest joke, which the wiki keeps as `File:<id>.jpg`.
//! Summaries always come from the patch notes.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sulfur_download::{CancellationToken, Downloader};

use crate::{McError, Result, groups::VersionGroup};

pub const PATCH_NOTES_URL: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const LAUNCHER_CONTENT: &str = "https://launchercontent.mojang.com";
const WIKI_API: &str = "https://minecraft.wiki/api.php";
const ARTWORK_ARCHIVE: &str = "https://raw.githubusercontent.com/TQNL/minecraft-update-artwork/main/";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    /// Group key, see [`VersionGroup::key`].
    pub key: String,
    pub image_url: Option<String>,
    /// A small version of the image for the version grid, when there is one.
    #[serde(default)]
    pub preview_url: Option<String>,
    /// One or two sentences from the update's patch notes.
    pub summary: Option<String>,
}

#[derive(Deserialize)]
struct Catalogue {
    assets: Vec<CatalogueAsset>,
}

#[derive(Deserialize)]
struct CatalogueAsset {
    update: String,
    file: String,
    preview: Option<String>,
    classification: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct PatchNotes {
    entries: Vec<PatchNote>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchNote {
    version: String,
    image: Option<Image>,
    #[serde(default)]
    short_text: Option<String>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

/// Finds artwork for each group. Network failures leave groups without art
/// rather than failing the whole call.
pub async fn fetch(downloader: &Downloader, groups: &[VersionGroup], cancel: &CancellationToken) -> Vec<Banner> {
    let notes: HashMap<String, PatchNote> = match fetch_json::<PatchNotes>(downloader, PATCH_NOTES_URL, cancel).await {
        Ok(notes) => notes.entries.into_iter().map(|n| (n.version.clone(), n)).collect(),
        Err(e) => {
            tracing::warn!(error = %e, "patch notes unavailable");
            HashMap::new()
        }
    };
    let archive: HashMap<String, CatalogueAsset> = match fetch_json::<Catalogue>(downloader, &format!("{ARTWORK_ARCHIVE}catalogue.json"), cancel).await {
        Ok(catalogue) => catalogue
            .assets
            .into_iter()
            // The archive's own pick per update, and only Mojang's artwork (not fan art or reconstructions).
            .filter(|a| a.primary && a.classification == "Official artwork")
            .map(|a| (update_key(&a.update), a))
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "artwork archive unavailable");
            HashMap::new()
        }
    };
    let mut banners: Vec<Banner> = groups
        .iter()
        .map(|g| {
            let note = g.representative().and_then(|v| notes.get(&v.id));
            let art = g.name.and_then(|name| archive.get(&update_key(name)));
            Banner {
                key: g.key.clone(),
                image_url: art
                    .map(|a| format!("{ARTWORK_ARCHIVE}{}", a.file))
                    .or_else(|| note.and_then(|n| n.image.as_ref()).map(|i| absolute(&i.url))),
                preview_url: art.and_then(|a| a.preview.as_ref()).map(|p| format!("{ARTWORK_ARCHIVE}{p}")),
                summary: note.and_then(|n| n.short_text.as_deref()).map(summarize),
            }
        })
        .collect();

    let missing: Vec<usize> = banners
        .iter()
        .enumerate()
        .filter(|(_, b)| b.image_url.is_none())
        .map(|(i, _)| i)
        .collect();
    // April Fools: the newest joke's launcher banner.
    if let Some(i) = missing.iter().copied().find(|&i| groups[i].kind == crate::groups::GroupKind::AprilFools)
        && let Some(newest) = groups[i].representative()
    {
        let file = format!("File:{}.jpg", newest.id);
        match wiki_files(downloader, std::slice::from_ref(&file), cancel).await {
            Ok(found) => banners[i].image_url = found.get(&file).cloned(),
            Err(e) => tracing::warn!(error = %e, "wiki files unavailable"),
        }
    }
    // Groups whose page image repeats a neighbor's take a wiki file of their own.
    let own: Vec<(usize, String)> = missing
        .iter()
        .filter_map(|&i| own_file(&groups[i].key).map(|file| (i, file.to_owned())))
        .collect();
    if !own.is_empty() {
        let files: Vec<String> = own.iter().map(|(_, file)| file.clone()).collect();
        match wiki_files(downloader, &files, cancel).await {
            Ok(found) => {
                for (i, file) in &own {
                    banners[*i].image_url = found.get(file).cloned();
                }
            }
            Err(e) => tracing::warn!(error = %e, "wiki files unavailable"),
        }
    }
    let missing: Vec<usize> = missing.into_iter().filter(|&i| banners[i].image_url.is_none()).collect();
    if !missing.is_empty() {
        let titles: Vec<String> = missing.iter().map(|&i| wiki_title(&groups[i])).collect();
        match wiki_images(downloader, &titles, cancel).await {
            Ok(found) => {
                for (&i, title) in missing.iter().zip(&titles) {
                    banners[i].image_url = found.get(title).cloned();
                }
            }
            Err(e) => tracing::warn!(error = %e, "wiki images unavailable"),
        }
    }
    banners
}

/// An update's name reduced to letters and digits, so "Caves & Cliffs: Part II"
/// matches "Caves and Cliffs Part II" and "World of Color" matches "World of Color Update".
fn update_key(name: &str) -> String {
    let words = name.to_lowercase().replace('&', " and ");
    let key: String = words.chars().filter(char::is_ascii_alphanumeric).collect();
    let key = key.strip_prefix("the").unwrap_or(&key);
    key.strip_suffix("update").unwrap_or(key).to_owned()
}

/// A wiki file for a group whose page image would repeat another's: 1.0's
/// page shows the same world as 1.1's, so 1.0 shows its title screen, as the
/// cards of 1.2 to 1.5 do.
fn own_file(key: &str) -> Option<&'static str> {
    match key {
        "1.0" => Some("File:Java Edition 1.0.0 menu.png"),
        _ => None,
    }
}

/// The wiki page about a group; the newest joke's page for April Fools.
fn wiki_title(group: &VersionGroup) -> String {
    match group.key.as_str() {
        "beta" => "Java Edition Beta".into(),
        "alpha" => "Java Edition Alpha".into(),
        crate::groups::APRIL_FOOLS => match group.representative() {
            Some(newest) => format!("Java Edition {}", newest.id),
            None => "April Fools' Day jokes".into(),
        },
        key => format!("Java Edition {key}"),
    }
}

/// Lead images of wiki pages, keyed by page title.
async fn wiki_images(
    downloader: &Downloader,
    titles: &[String],
    cancel: &CancellationToken,
) -> Result<HashMap<String, String>> {
    #[derive(Deserialize)]
    struct Response {
        query: Option<Query>,
    }
    #[derive(Deserialize)]
    struct Query {
        #[serde(default)]
        pages: HashMap<String, Page>,
        /// Titles spelled differently than asked, then pages moved elsewhere
        /// ("Java Edition 1.0" is "Java Edition 1.0.0").
        #[serde(default)]
        normalized: Vec<Rename>,
        #[serde(default)]
        redirects: Vec<Rename>,
    }
    #[derive(Deserialize)]
    struct Rename {
        from: String,
        to: String,
    }
    #[derive(Deserialize)]
    struct Page {
        title: String,
        original: Option<Original>,
    }
    #[derive(Deserialize)]
    struct Original {
        source: String,
    }

    let mut found = HashMap::new();
    // The API accepts up to 50 titles per request.
    for chunk in titles.chunks(50) {
        let joined = chunk.iter().map(|t| t.replace(' ', "_")).collect::<Vec<_>>().join("|");
        let url = format!(
            "{WIKI_API}?action=query&format=json&prop=pageimages&piprop=original&redirects=1&titles={}",
            sulfur_download::encode_component(&joined)
        );
        let Some(query) = fetch_json::<Response>(downloader, &url, cancel).await?.query else { continue };
        let renamed: HashMap<&str, &str> = query
            .normalized
            .iter()
            .chain(&query.redirects)
            .map(|r| (r.from.as_str(), r.to.as_str()))
            .collect();
        let images: HashMap<&str, &str> = query
            .pages
            .values()
            .filter_map(|p| Some((p.title.as_str(), p.original.as_ref()?.source.as_str())))
            .collect();
        for title in chunk {
            // Follow the asked title through its renames to the page that answered.
            let mut current = title.replace(' ', "_");
            for _ in 0..3 {
                match renamed.get(current.as_str()).or_else(|| renamed.get(current.replace('_', " ").as_str())) {
                    Some(next) => current = (*next).to_owned(),
                    None => break,
                }
            }
            if let Some(source) = images.get(current.replace('_', " ").as_str()) {
                found.insert(title.clone(), (*source).to_owned());
            }
        }
    }
    Ok(found)
}

/// Addresses of wiki files (`File:<name>`), keyed by the asked title.
async fn wiki_files(
    downloader: &Downloader,
    titles: &[String],
    cancel: &CancellationToken,
) -> Result<HashMap<String, String>> {
    #[derive(Deserialize)]
    struct Response {
        query: Option<Query>,
    }
    #[derive(Deserialize)]
    struct Query {
        #[serde(default)]
        pages: HashMap<String, Page>,
    }
    #[derive(Deserialize)]
    struct Page {
        title: String,
        #[serde(default)]
        imageinfo: Vec<Info>,
    }
    #[derive(Deserialize)]
    struct Info {
        url: String,
    }
    let url = format!(
        "{WIKI_API}?action=query&format=json&prop=imageinfo&iiprop=url&titles={}",
        sulfur_download::encode_component(&titles.join("|"))
    );
    let response: Response = fetch_json(downloader, &url, cancel).await?;
    let pages = response.query.map(|q| q.pages).unwrap_or_default();
    Ok(pages
        .into_values()
        .filter_map(|p| Some((p.title, p.imageinfo.into_iter().next()?.url)))
        .filter(|(title, _)| titles.contains(title))
        .collect())
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    downloader: &Downloader,
    url: &str,
    cancel: &CancellationToken,
) -> Result<T> {
    let bytes = downloader.fetch(url, cancel).await?;
    serde_json::from_slice(&bytes).map_err(|source| McError::Json { path: url.into(), source })
}

fn absolute(url: &str) -> String {
    if url.starts_with("http") { url.to_owned() } else { format!("{LAUNCHER_CONTENT}{url}") }
}

/// First sentences of the patch notes' teaser, up to roughly 200 characters.
fn summarize(text: &str) -> String {
    let text = decode_entities(text.lines().next().unwrap_or_default().trim());
    let mut out = String::new();
    for sentence in text.split_inclusive(['.', '!', '?']) {
        if !out.is_empty() && out.len() + sentence.len() > 200 {
            break;
        }
        out.push_str(sentence);
    }
    out.trim().to_owned()
}

fn decode_entities(text: &str) -> String {
    text.replace("&#x26;", "&")
        .replace("&amp;", "&")
        .replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_teasers() {
        assert_eq!(
            summarize("Caves &#x26; Cliffs is here. It is big! More text follows here.\nSecond line"),
            "Caves & Cliffs is here. It is big! More text follows here."
        );
        let long = format!("{}. Tail.", "a".repeat(250));
        assert_eq!(summarize(&long), format!("{}.", "a".repeat(250)));
    }

    #[test]
    fn matches_update_names() {
        assert_eq!(update_key("Caves & Cliffs: Part II"), update_key("Caves & Cliffs Part II"));
        assert_eq!(update_key("World of Color"), update_key("World of Color Update"));
        assert_eq!(update_key("The Wild Update"), "wild");
        assert_ne!(update_key("Caves & Cliffs: Part I"), update_key("Caves & Cliffs Part II"));
    }

    #[test]
    fn makes_urls_absolute() {
        assert_eq!(absolute("/v2/images/x.jpg"), "https://launchercontent.mojang.com/v2/images/x.jpg");

    }
}
