//! Minecraft: Java Edition news, from the feed the official launcher shows.

use serde::{Deserialize, Serialize};
use sulfur_download::{CancellationToken, Downloader};

use crate::{McError, Result};

pub const NEWS_URL: &str = "https://launchercontent.mojang.com/v2/news.json";
const LAUNCHER_CONTENT: &str = "https://launchercontent.mojang.com";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsEntry {
    pub id: String,
    pub title: String,
    /// One-line teaser.
    pub text: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub image_url: Option<String>,
    /// The article on minecraft.net.
    pub link: String,
}

#[derive(Deserialize)]
struct Feed {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(default)]
    id: String,
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    date: String,
    play_page_image: Option<Image>,
    news_page_image: Option<Image>,
    #[serde(default)]
    read_more_link: String,
    #[serde(default)]
    news_type: Vec<String>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

/// Java Edition entries of the feed, newest first.
pub fn parse(bytes: &[u8]) -> Result<Vec<NewsEntry>> {
    let feed: Feed = serde_json::from_slice(bytes).map_err(|source| McError::Json { path: NEWS_URL.into(), source })?;
    let mut entries: Vec<NewsEntry> = feed
        .entries
        .into_iter()
        .filter(|e| e.news_type.iter().any(|t| t == "Java"))
        .map(|e| NewsEntry {
            image_url: e.news_page_image.or(e.play_page_image).map(|i| {
                if i.url.starts_with("http") { i.url } else { format!("{LAUNCHER_CONTENT}{}", i.url) }
            }),
            id: e.id,
            title: e.title,
            text: e.text,
            date: e.date,
            link: e.read_more_link,
        })
        .collect();
    // ISO dates sort as text; the feed is mostly ordered already.
    entries.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(entries)
}

pub async fn fetch(downloader: &Downloader, cancel: &CancellationToken) -> Result<Vec<NewsEntry>> {
    parse(&downloader.fetch(NEWS_URL, cancel).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_java_entries_newest_first() {
        let feed = br#"{"version":2,"entries":[
            {"title":"Old","date":"2026-01-02","text":"a","newsType":["Java"],"readMoreLink":"https://x/old",
             "playPageImage":{"title":"p","url":"/v2/images/p.jpg"}},
            {"title":"Bedrock","date":"2026-09-01","newsType":["Bedrock"]},
            {"title":"New","date":"2026-10-30","newsType":["Java","News page"],"readMoreLink":"https://x/new",
             "newsPageImage":{"title":"n","url":"/v2/images/n.jpg"},"playPageImage":{"title":"p","url":"/v2/images/q.jpg"}}
        ]}"#;
        let news = parse(feed).unwrap();
        assert_eq!(news.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), ["New", "Old"]);
        assert_eq!(news[0].image_url.as_deref(), Some("https://launchercontent.mojang.com/v2/images/n.jpg"));
        assert_eq!(news[1].link, "https://x/old");
    }
}
