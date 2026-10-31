//! Combining search results from several sources.
//!
//! Many projects are published on both Modrinth and CurseForge. Two hits from
//! different sources are the same project when their URL names or titles match
//! once case, spaces and punctuation are ignored, and their authors look alike
//! (author names often differ slightly between sites, e.g. `jellysquid3` and
//! `JellySquid`). When the authors do not look alike, both the URL name and
//! the title must match.

use crate::ModHit;

/// Appends `incoming` lists to `existing`, interleaving them by rank (the
/// first hit of each list, then the second of each, …). Hits that duplicate
/// one already present are folded into it as an alternative source, adding
/// their downloads.
pub fn merge(existing: Vec<ModHit>, incoming: Vec<Vec<ModHit>>) -> Vec<ModHit> {
    let mut out = existing;
    let longest = incoming.iter().map(Vec::len).max().unwrap_or(0);
    let mut lists: Vec<_> = incoming.into_iter().map(Vec::into_iter).collect();
    for _ in 0..longest {
        for list in &mut lists {
            let Some(hit) = list.next() else { continue };
            match out.iter_mut().find(|known| same_project(known, &hit)) {
                Some(known) => {
                    known.downloads += hit.downloads;
                    known.also.extend(hit.refs());
                }
                None => out.push(hit),
            }
        }
    }
    out
}

fn same_project(known: &ModHit, hit: &ModHit) -> bool {
    // Never two projects of one source, nor a source the hit already has.
    if known.refs().iter().any(|r| r.source == hit.source) {
        return false;
    }
    let slug = !hit.slug.is_empty() && normalize(&known.slug) == normalize(&hit.slug);
    let title = normalize(&known.title) == normalize(&hit.title) && !normalize(&hit.title).is_empty();
    if !slug && !title {
        return false;
    }
    authors_alike(&known.author, &hit.author) || (slug && title)
}

fn authors_alike(a: &str, b: &str) -> bool {
    let (a, b) = (normalize(a), normalize(b));
    a.is_empty() || b.is_empty() || a.contains(&b) || b.contains(&a)
}

/// Lowercase letters and digits only: `Just Enough Items (JEI)` → `justenoughitemsjei`.
fn normalize(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Source;

    fn hit(source: Source, id: &str, slug: &str, title: &str, author: &str, downloads: u64) -> ModHit {
        ModHit {
            source,
            id: id.into(),
            slug: slug.into(),
            title: title.into(),
            author: author.into(),
            description: String::new(),
            icon_url: None,
            downloads,
            categories: Vec::new(),
            page_url: format!("https://{id}"),
            also: Vec::new(),
        }
    }

    #[test]
    fn folds_the_same_project_and_interleaves_the_rest() {
        let modrinth = vec![
            hit(Source::Modrinth, "AANobbMI", "sodium", "Sodium", "jellysquid3", 100),
            hit(Source::Modrinth, "a", "backpacks", "Backpacks", "alice", 5),
        ];
        let curseforge = vec![
            hit(Source::CurseForge, "394468", "sodium", "Sodium", "JellySquid", 50),
            hit(Source::CurseForge, "b", "backpacks", "Backpacks Mod", "bob", 7),
            hit(Source::CurseForge, "c", "jei", "Just Enough Items (JEI)", "mezz", 9),
        ];
        let merged = merge(Vec::new(), vec![modrinth, curseforge]);
        let ids: Vec<&str> = merged.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["AANobbMI", "a", "b", "c"]);
        assert_eq!(merged[0].downloads, 150);
        assert_eq!(merged[0].also.len(), 1);
        assert_eq!(merged[0].also[0].id, "394468");
        // Same name, different authors, and the titles differ: kept apart.
        assert!(merged[1].also.is_empty());
    }

    #[test]
    fn never_merges_within_one_source_and_merges_across_pages() {
        let page1 = merge(Vec::new(), vec![vec![hit(Source::Modrinth, "x", "lithium", "Lithium", "caffeine", 1)]]);
        let page2 = merge(
            page1,
            vec![
                vec![hit(Source::Modrinth, "y", "lithium", "Lithium", "caffeine", 1)],
                vec![hit(Source::CurseForge, "z", "lithium", "Lithium", "", 2)],
            ],
        );
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].also[0].id, "z");
        assert_eq!(page2[0].downloads, 3);
    }
}
