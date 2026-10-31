//! Searches both sources and picks a file, against the live APIs.
//!
//! ```text
//! cargo run -p sulfur-mods --example search -- [QUERY] [GAME_VERSION]
//! ```

use sulfur_download::{CancellationToken, DownloadConfig, Downloader, MirrorSet};
use sulfur_mods::{Loader, ModsClient, ProjectKind, Query, Source};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = args.first().cloned().unwrap_or_else(|| "sodium".into());
    let game = args.get(1).cloned().unwrap_or_else(|| "1.21.1".into());
    let downloader = Downloader::new(DownloadConfig::default(), MirrorSet::default())?;
    let client = ModsClient::new(downloader, std::env::var("CURSEFORGE_API_KEY").ok());
    let cancel = CancellationToken::new();
    for (source, loader) in [(Source::Modrinth, Loader::Fabric), (Source::CurseForge, Loader::NeoForge)] {
        let query = Query { text: text.clone(), game_version: Some(game.clone()), loader: Some(loader), limit: 3, ..Default::default() };
        let page = client.search(source, &query, &cancel).await?;
        println!("{source:?}: {} results", page.total);
        for hit in &page.hits {
            let file = client.best_file(source, ProjectKind::Mod, &hit.id, &game, Some(loader), &cancel).await?;
            println!(
                "  {} by {} ({} downloads) -> {:?}",
                hit.title,
                hit.author,
                hit.downloads,
                file.map(|f| (f.file_name, f.sha1.is_some(), f.required.len()))
            );
        }
    }
    // Both sources at once, with projects on both folded together.
    let query = Query { text: text.clone(), game_version: Some(game.clone()), loader: Some(Loader::Fabric), limit: 5, ..Default::default() };
    let merged = client.search_many(&[Source::Modrinth, Source::CurseForge], &query, &cancel).await?;
    println!("Both: {} results", merged.total);
    for hit in &merged.hits {
        let sources: Vec<_> = hit.refs().iter().map(|r| format!("{:?}", r.source)).collect();
        println!("  {} by {} on {}", hit.title, hit.author, sources.join(" + "));
    }
    Ok(())
}
