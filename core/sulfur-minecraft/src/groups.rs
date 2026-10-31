//! Grouping the version list into major updates (`1.21`, `26.3`, …) for display.

use crate::manifest::{ManifestVersion, VersionKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Snapshots newer than the latest release.
    Upcoming,
    Release,
    Beta,
    Alpha,
    /// Mojang's April Fools versions, which lead to no release.
    AprilFools,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionGroup {
    /// `1.21`, `26.3`, `beta`, `alpha`, `april-fools`, or the upcoming version (`26.4`, else `upcoming`).
    pub key: String,
    pub kind: GroupKind,
    /// Name of the update, e.g. "Tricky Trials", where known.
    pub name: Option<&'static str>,
    /// Releases (or betas/alphas), newest first.
    pub releases: Vec<ManifestVersion>,
    /// Snapshots, pre-releases and release candidates leading up to this group, newest first.
    pub previews: Vec<ManifestVersion>,
}

impl VersionGroup {
    /// The version whose artwork represents the group: its first release, or
    /// for upcoming versions the newest snapshot and for April Fools the newest joke.
    pub fn representative(&self) -> Option<&ManifestVersion> {
        match self.kind {
            GroupKind::Upcoming => self.previews.first(),
            GroupKind::AprilFools => self.releases.first(),
            _ => self.releases.last(),
        }
    }
}

/// Major-update key of a release id: `1.21.4` → `1.21`, `26.1.2` → `26.1`.
pub fn major_key(id: &str) -> String {
    let mut parts = id.split(['.', '-', ' ']);
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok() => format!("{a}.{b}"),
        _ => id.to_owned(),
    }
}

/// Groups versions (in any order) into major updates, newest first. Each
/// preview joins the first release published after it; April Fools versions
/// get a group of their own at the end.
pub fn group(versions: &[ManifestVersion]) -> Vec<VersionGroup> {
    let mut sorted: Vec<&ManifestVersion> = versions.iter().collect();
    // RFC 3339 timestamps with the same offset sort chronologically as text.
    sorted.sort_by(|a, b| a.release_time.cmp(&b.release_time));

    let mut groups: Vec<VersionGroup> = Vec::new();
    let mut pending: Vec<ManifestVersion> = Vec::new();
    let mut april_fools: Vec<ManifestVersion> = Vec::new();
    for v in sorted {
        if v.is_april_fools() {
            april_fools.insert(0, v.clone());
            continue;
        }
        let (key, kind) = match v.kind {
            VersionKind::Release => (major_key(&v.id), GroupKind::Release),
            VersionKind::OldBeta => ("beta".to_owned(), GroupKind::Beta),
            VersionKind::OldAlpha => ("alpha".to_owned(), GroupKind::Alpha),
            VersionKind::Snapshot | VersionKind::Other => {
                pending.push(v.clone());
                continue;
            }
        };
        let index = match groups.iter().position(|g| g.key == key) {
            Some(i) => i,
            None => {
                groups.push(VersionGroup {
                    name: update_name(&key),
                    key,
                    kind,
                    releases: Vec::new(),
                    previews: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        group.releases.insert(0, v.clone());
        for preview in pending.drain(..) {
            group.previews.insert(0, preview);
        }
    }
    if !pending.is_empty() {
        pending.reverse();
        let key = upcoming_key(&pending[0].id);
        groups.push(VersionGroup {
            name: update_name(&key),
            key,
            kind: GroupKind::Upcoming,
            releases: Vec::new(),
            previews: pending,
        });
    }

    if !april_fools.is_empty() {
        groups.push(VersionGroup {
            key: APRIL_FOOLS.to_owned(),
            kind: GroupKind::AprilFools,
            name: update_name(APRIL_FOOLS),
            releases: april_fools,
            previews: Vec::new(),
        });
    }

    // Newest first, with betas, alphas and April Fools versions at the end.
    let rank = |g: &VersionGroup| match g.kind {
        GroupKind::Upcoming => 0,
        GroupKind::Release => 1,
        GroupKind::Beta => 2,
        GroupKind::Alpha => 3,
        GroupKind::AprilFools => 4,
    };
    groups.reverse();
    groups.sort_by_key(rank);
    groups
}

/// Key of the April Fools group.
pub const APRIL_FOOLS: &str = "april-fools";

/// `26.4-snapshot-6` → `26.4`; weekly snapshots (`25w41a`) have no version yet.
fn upcoming_key(snapshot: &str) -> String {
    let key = major_key(snapshot);
    if key == snapshot { "upcoming".to_owned() } else { key }
}

/// Official names of major updates and game drops.
pub fn update_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "26.3" => "Wilderness Bound",
        "26.2" => "Chaos Cubed",
        "26.1" => "Tiny Takeover",
        "1.21" => "Tricky Trials",
        "1.20" => "Trails & Tales",
        "1.19" => "The Wild Update",
        "1.18" => "Caves & Cliffs: Part II",
        "1.17" => "Caves & Cliffs: Part I",
        "1.16" => "Nether Update",
        "1.15" => "Buzzy Bees",
        "1.14" => "Village & Pillage",
        "1.13" => "Update Aquatic",
        "1.12" => "World of Color",
        "1.11" => "Exploration Update",
        "1.10" => "Frostburn Update",
        "1.9" => "Combat Update",
        "1.8" => "Bountiful Update",
        "1.7" => "The Update that Changed the World",
        "1.6" => "Horse Update",
        "1.5" => "Redstone Update",
        "1.4" => "Pretty Scary Update",
        "1.0" => "Adventure Update",
        "beta" => "Beta",
        "alpha" => "Alpha & Classic",
        APRIL_FOOLS => "April Fools",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, kind: VersionKind, time: &str) -> ManifestVersion {
        ManifestVersion {
            id: id.into(),
            kind,
            url: String::new(),
            release_time: format!("{time}T00:00:00+00:00"),
            sha1: String::new(),
        }
    }

    #[test]
    fn major_keys() {
        assert_eq!(major_key("1.21.4"), "1.21");
        assert_eq!(major_key("1.8"), "1.8");
        assert_eq!(major_key("26.1.2"), "26.1");
        assert_eq!(major_key("26.4-snapshot-6"), "26.4");
        assert_eq!(major_key("25w41a"), "25w41a");
    }

    #[test]
    fn groups_previews_with_the_next_release() {
        use VersionKind::*;
        let list = [
            v("26.4-snapshot-1", Snapshot, "2026-10-01"),
            v("26.3", Release, "2026-09-01"),
            v("26.3-rc-1", Snapshot, "2026-08-25"),
            v("1.21.11", Release, "2025-12-01"),
            v("1.21.10", Release, "2025-10-01"),
            v("25w41a", Snapshot, "2025-10-08"),
            v("1.21", Release, "2024-06-13"),
            v("b1.7.3", OldBeta, "2011-07-08"),
            v("24w14potato", Snapshot, "2024-04-01"),
            v("1.RV-Pre1", Snapshot, "2016-03-31"),
            v("26w14a", Snapshot, "2026-04-01"),
        ];
        let groups = group(&list);
        let keys: Vec<_> = groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, ["26.4", "26.3", "1.21", "beta", "april-fools"]);
        let fools: Vec<_> = groups[4].releases.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(fools, ["26w14a", "24w14potato", "1.RV-Pre1"]);
        assert!(groups[1].previews.iter().all(|p| p.id != "26w14a"));
        assert_eq!(groups[0].kind, GroupKind::Upcoming);
        assert_eq!(groups[1].previews[0].id, "26.3-rc-1");
        assert_eq!(groups[1].name, Some("Wilderness Bound"));
        let ids: Vec<_> = groups[2].releases.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["1.21.11", "1.21.10", "1.21"]);
        assert_eq!(groups[2].previews[0].id, "25w41a");
        assert_eq!(groups[2].representative().unwrap().id, "1.21");
        assert_eq!(groups[0].representative().unwrap().id, "26.4-snapshot-1");
    }
}
