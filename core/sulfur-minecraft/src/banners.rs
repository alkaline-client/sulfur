//! Artwork for major updates.
//!
//! The official launcher's patch notes carry key art for every version since
//! 1.13; older updates fall back to the lead image of their Minecraft Wiki page.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sulfur_download::{CancellationToken, Downloader};

use crate::{McError, Result, groups::VersionGroup};

pub const PATCH_NOTES_URL: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const LAUNCHER_CONTENT: &str = "https://launchercontent.mojang.com";
const WIKI_API: &str = "https://minecraft.wiki/api.php";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    /// Group key, see [`VersionGroup::key`].
    pub key: String,
    pub image_url: Option<String>,
    /// One or two sentences from the update's patch notes.
    pub summary: Option<String>,
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
    let mut banners: Vec<Banner> = groups
        .iter()
        .map(|g| {
            let note = g.representative().and_then(|v| notes.get(&v.id));
            Banner {
                key: g.key.clone(),
                image_url: note.and_then(|n| n.image.as_ref()).map(|i| absolute(&i.url)),
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
    if !missing.is_empty() {
        let titles: Vec<String> = missing.iter().map(|&i| wiki_title(&banners[i].key)).collect();
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

fn wiki_title(key: &str) -> String {
    match key {
        "beta" => "Java Edition Beta".into(),
        "alpha" => "Java Edition Alpha".into(),
        crate::groups::APRIL_FOOLS => "April Fools' Day".into(),
        _ => format!("Java Edition {key}"),
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
            "{WIKI_API}?action=query&format=json&prop=pageimages&piprop=original&titles={}",
            sulfur_download::encode_component(&joined)
        );
        let response: Response = fetch_json(downloader, &url, cancel).await?;
        for page in response.query.map(|q| q.pages).unwrap_or_default().into_values() {
            if let Some(original) = page.original {
                found.insert(page.title, original.source);
            }
        }
    }
    Ok(found)
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
    fn makes_urls_absolute() {
        assert_eq!(absolute("/v2/images/x.jpg"), "https://launchercontent.mojang.com/v2/images/x.jpg");
        assert_eq!(wiki_title("1.12"), "Java Edition 1.12");
    }
}
