//! The global version list (`version_manifest_v2.json`).

use serde::Deserialize;

pub const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone, Debug, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub release_time: String,
    pub sha1: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Other,
}

impl ManifestVersion {
    /// April Fools versions: every snapshot Mojang published on April 1 (UTC)
    /// was one, plus 1.RV-Pre1, which came out on March 31. Names alone do
    /// not tell (2026's is a plain `26w14a`).
    pub fn is_april_fools(&self) -> bool {
        self.kind == VersionKind::Snapshot
            && (self.release_time.get(4..10) == Some("-04-01") || self.id == "1.RV-Pre1")
    }
}

impl VersionManifest {
    pub fn find(&self, id: &str) -> Option<&ManifestVersion> {
        self.versions.iter().find(|v| v.id == id)
    }
}
