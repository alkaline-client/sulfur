//! Versions preserved by Omniarchive (<https://omniarchive.net>) that Mojang
//! no longer offers: lost alphas, indev and infdev builds, pre-releases,
//! April Fools 2.0.
//!
//! Omniarchive publishes plain client jars in browsable folders and no
//! launcher metadata, so these versions are listed by their jar names and
//! launched with the metadata of the nearest official version of the same era
//! (same launch wrapper, libraries and assets), with the jar swapped.

use serde::{Deserialize, Serialize};
use sulfur_download::{CancellationToken, Checksum, DownloadTask, Downloader};

use crate::{
    McError, Result,
    install::{InstallMonitor, InstallStage, Installer},
    manifest::{ManifestVersion, VersionKind, VersionManifest},
    version::VersionJson,
};

const SITE: &str = "https://omniarchive.net";

/// Client folders of the archive, oldest era first.
pub const CATEGORIES: &[&str] = &["preclassic", "classic", "indev", "infdev", "alpha", "beta", "release", "april-fools"];

/// A client jar in the archive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivedJar {
    /// The jar's file name without `.jar`; also the installed version id.
    pub id: String,
    pub file_name: String,
    /// One of [`CATEGORIES`].
    pub category: String,
    pub url: String,
    /// Size as the archive shows it, e.g. `1.4 MiB`.
    pub size: String,
}

/// Lists the archive's client jars, leaving out versions Mojang still offers
/// under the same id.
pub async fn list(downloader: &Downloader, official: &VersionManifest, cancel: &CancellationToken) -> Result<Vec<ArchivedJar>> {
    let mut jars: Vec<(usize, ArchivedJar)> = Vec::new();
    // Each round fetches one level of folders at once; folders nest one
    // level deep (release/1.3/…) and the walk stays inside each category.
    let mut level: Vec<(usize, String)> =
        CATEGORIES.iter().enumerate().map(|(i, c)| (i, format!("/archive/java/client/{c}/"))).collect();
    for depth in 0..3 {
        if level.is_empty() {
            break;
        }
        let pages = futures_util::future::join_all(level.iter().map(|(_, path)| {
            let url = format!("{SITE}{path}");
            async move { downloader.fetch(&url, cancel).await }
        }))
        .await;
        let mut next = Vec::new();
        for ((category, path), page) in level.iter().zip(pages) {
            let root = format!("/archive/java/client/{}/", CATEGORIES[*category]);
            for link in links(&String::from_utf8_lossy(&page?)) {
                if let Some(jar) = jar_entry(&link, CATEGORIES[*category]) {
                    jars.push((*category, jar));
                } else if depth < 2 && link.href.starts_with(&root) && link.href.len() > path.len() && link.href.ends_with('/') {
                    next.push((*category, link.href));
                }
            }
        }
        level = next;
    }
    jars.sort_by_key(|(category, _)| *category);
    let mut seen = std::collections::HashSet::new();
    Ok(jars
        .into_iter()
        .map(|(_, jar)| jar)
        .filter(|j| official.find(&j.id).is_none() && seen.insert(j.id.clone()))
        .collect())
}

/// Installs an archived jar as version `jar.id`.
pub async fn install(
    installer: &Installer,
    official: &VersionManifest,
    jar: &ArchivedJar,
    monitor: &InstallMonitor,
    cancel: &CancellationToken,
) -> Result<VersionJson> {
    monitor.set_stage(InstallStage::Preparing);
    let template = template_for(official, jar)
        .ok_or_else(|| McError::Invalid(format!("no official version to base {} on", jar.id)))?;
    let dir = installer.dir();
    let template_path = dir.version_dir(&jar.id).join(".template.json");
    let task = DownloadTask::new(&template.url, &template_path).checksum(Checksum::sha1_hex(&template.sha1));
    installer.batch(vec![task], &monitor.progress, cancel).await?;

    let mut json: serde_json::Value = crate::error::read_json(&template_path)?;
    json["id"] = jar.id.clone().into();
    json["downloads"] = serde_json::json!({ "client": { "url": jar.url } });
    let json_path = dir.version_json(&jar.id);
    let bytes = serde_json::to_vec_pretty(&json).map_err(|e| McError::Invalid(e.to_string()))?;
    std::fs::write(&json_path, bytes).map_err(|e| McError::io(&json_path, e))?;
    let _ = std::fs::remove_file(&template_path);
    tracing::info!(id = %jar.id, template = %template.id, "installing archived version");

    installer.install_json(VersionJson::read(&json_path)?, monitor, cancel).await
}

/// The official version whose metadata launches `jar`: the nearest one of
/// the same era by version order (`a1.0.1_01` → `a1.0.4`, which still needs
/// the applet wrapper; `a1.0.12` → `a1.0.11`, which does not).
pub fn template_for<'a>(official: &'a VersionManifest, jar: &ArchivedJar) -> Option<&'a ManifestVersion> {
    let id = jar.id.as_str();
    let pinned = match family(id) {
        // Indev has no official build left; it launches like infdev and classic.
        "in" => Some("inf-20100618"),
        // 2.0 (2013) was built on 1.5.1.
        _ if id.starts_with("2.0") => Some("1.5.1"),
        _ => None,
    };
    if let Some(pinned) = pinned {
        return official.find(pinned);
    }
    // Another build of an official version (`17w13a-0805`, `1.12-pre3-1316`,
    // `b1.2_02-launcher`) launches like it; the longest such id wins.
    let base = official
        .versions
        .iter()
        .filter(|v| id.strip_prefix(v.id.as_str()).is_some_and(|rest| rest.starts_with(['-', '_'])))
        .max_by_key(|v| v.id.len());
    if base.is_some() {
        return base;
    }
    // A weekly snapshot leads up to the next release: `13w01a` → 1.5.1.
    if family(id) == "weekly" {
        let week = format!("20{}-{:02}", &id[..2], (id[3..5].parse::<u32>().unwrap_or(1) * 7 / 30 + 1).min(12));
        return official
            .versions
            .iter()
            .filter(|v| v.kind == VersionKind::Release && v.release_time.as_str() > week.as_str())
            .min_by(|a, b| a.release_time.cmp(&b.release_time));
    }
    let mut candidates: Vec<&ManifestVersion> = official
        .versions
        .iter()
        .filter(|v| family(&v.id) == family(id))
        .filter(|v| family(id) != "release" || v.kind == VersionKind::Release)
        .collect();
    // Pre-releases launch like the release they led to: `1.6-pre-…` → `1.6.4`.
    if family(id) == "release" {
        let major = crate::groups::major_key(id);
        let same_update = candidates.iter().filter(|v| crate::groups::major_key(&v.id) == major);
        if let Some(newest) = same_update.max_by(|a, b| natural_cmp(&a.id, &b.id)) {
            return Some(newest);
        }
    }
    candidates.sort_by(|a, b| natural_cmp(&a.id, &b.id));
    candidates
        .iter()
        .rev()
        .find(|v| natural_cmp(&v.id, id).is_le())
        .or_else(|| candidates.first())
        .copied()
}

/// Era of a version id: `rd`, `c`, `in`, `inf`, `a`, `b`, `weekly`
/// (snapshots like `13w16a`) or `release`.
fn family(id: &str) -> &str {
    let prefix = id.split(|c: char| c.is_ascii_digit() || c == '-').next().unwrap_or_default();
    let b = id.as_bytes();
    match prefix {
        "rd" | "c" | "in" | "inf" | "a" | "b" => prefix,
        _ if b.len() >= 5 && b[..2].iter().all(u8::is_ascii_digit) && b[2] == b'w' && b[3..5].iter().all(u8::is_ascii_digit) => {
            "weekly"
        }
        _ => "release",
    }
}

/// Compares ids with numbers by value: `a1.0.4` < `a1.0.11`.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn tokens(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let bytes = s.as_bytes();
        for i in 1..=bytes.len() {
            if i == bytes.len() || bytes[i].is_ascii_digit() != bytes[start].is_ascii_digit() {
                out.push((bytes[start].is_ascii_digit(), &s[start..i]));
                start = i;
            }
        }
        out
    }
    let (ta, tb) = (tokens(a), tokens(b));
    for (x, y) in ta.iter().zip(&tb) {
        let order = match (x, y) {
            ((true, x), (true, y)) => x.parse::<u64>().unwrap_or(0).cmp(&y.parse::<u64>().unwrap_or(0)),
            ((_, x), (_, y)) => x.cmp(y),
        };
        if order.is_ne() {
            return order;
        }
    }
    ta.len().cmp(&tb.len())
}

struct Link {
    href: String,
    /// Text of the table row after the link (date and size columns).
    rest: String,
}

/// `<a href>` targets of a directory page, with the rest of their table row.
fn links(html: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for row in html.split("<tr>").skip(1) {
        let row = row.split("</tr>").next().unwrap_or_default();
        let Some(start) = row.find("href=\"") else { continue };
        let href = &row[start + 6..];
        let Some(end) = href.find('"') else { continue };
        let rest = row.rsplit("<td>").next().unwrap_or_default();
        out.push(Link {
            href: href[..end].replace("&amp;", "&"),
            rest: rest.split('<').next().unwrap_or_default().trim().to_owned(),
        });
    }
    out
}

fn jar_entry(link: &Link, category: &str) -> Option<ArchivedJar> {
    if !link.href.starts_with("https://") || !link.href.ends_with(".jar") {
        return None;
    }
    let file_name = link.href.rsplit('/').next()?.to_owned();
    Some(ArchivedJar {
        id: file_name.strip_suffix(".jar")?.to_owned(),
        file_name,
        category: category.to_owned(),
        url: link.href.clone(),
        size: link.rest.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(ids: &[(&str, VersionKind, &str)]) -> VersionManifest {
        VersionManifest {
            latest: crate::manifest::Latest {
                release: String::new(),
                snapshot: String::new(),
            },
            versions: ids
                .iter()
                .map(|(id, kind, time)| ManifestVersion {
                    id: (*id).into(),
                    kind: *kind,
                    url: String::new(),
                    release_time: (*time).into(),
                    sha1: String::new(),
                })
                .collect(),
        }
    }

    fn jar(id: &str) -> ArchivedJar {
        ArchivedJar {
            id: id.into(),
            file_name: format!("{id}.jar"),
            category: String::new(),
            url: String::new(),
            size: String::new(),
        }
    }

    #[test]
    fn picks_the_nearest_official_version_of_the_era() {
        use VersionKind::*;
        let m = manifest(&[
            ("a1.0.4", OldAlpha, ""),
            ("a1.0.5_01", OldAlpha, ""),
            ("a1.0.11", OldAlpha, ""),
            ("inf-20100618", OldAlpha, ""),
            ("1.4.7", Release, "2012-12-28T00:00:00+00:00"),
            ("1.5.1", Release, "2013-03-20T00:00:00+00:00"),
            ("1.6.4", Release, "2013-09-19T00:00:00+00:00"),
            ("1.12", Release, "2017-06-02T00:00:00+00:00"),
            ("1.12-pre3", Snapshot, ""),
            ("13w16a", Snapshot, "2013-04-18T00:00:00+00:00"),
        ]);
        let pick = |id| template_for(&m, &jar(id)).map(|v| v.id.as_str());
        assert_eq!(pick("a1.0.1_01"), Some("a1.0.4"));
        assert_eq!(pick("a1.0.6"), Some("a1.0.5_01"));
        assert_eq!(pick("a1.0.12"), Some("a1.0.11"));
        assert_eq!(pick("in-20091223-1459"), Some("inf-20100618"));
        assert_eq!(pick("2.0-blue"), Some("1.5.1"));
        assert_eq!(pick("1.6-pre-06251516"), Some("1.6.4"));
        assert_eq!(pick("1.4-pre"), Some("1.4.7"));
        assert_eq!(pick("1.6.5-test"), Some("1.6.4"));
        assert_eq!(pick("b1.0"), None);
        assert_eq!(pick("1.12-pre3-1316"), Some("1.12-pre3"));
        assert_eq!(pick("13w16a-1530"), Some("13w16a"));
        assert_eq!(pick("13w01a"), Some("1.5.1"));
        assert_eq!(pick("13w17a"), Some("1.6.4"));
    }

    #[test]
    fn reads_directory_pages() {
        let html = r#"<table>
<tr><td><a href="/archive/java/client">../</a></td><td>-</td><td>-</td></tr>
<tr><td><a href="/archive/java/client/release/1.3/">1.3</a></td><td>-</td><td>-</td></tr>
<tr><td><a href="https://vault.omniarchive.uk/archive/java/client-alpha/a1.0.1_01.jar">a1.0.1_01.jar</a></td><td>03/11/2022 17:10:01</td><td>722 KiB</td></tr>
<tr><td><a href="https://vault.omniarchive.uk/archive/java/client-beta/b1.3-pcgamer.exe">b1.3-pcgamer.exe</a></td><td>x</td><td>1 MiB</td></tr>
</table>"#;
        let found = links(html);
        assert_eq!(found.len(), 4);
        assert_eq!(found[1].href, "/archive/java/client/release/1.3/");
        let jars: Vec<_> = found.iter().filter_map(|l| jar_entry(l, "alpha")).collect();
        assert_eq!(jars.len(), 1);
        assert_eq!(jars[0].id, "a1.0.1_01");
        assert_eq!(jars[0].size, "722 KiB");
    }
}
