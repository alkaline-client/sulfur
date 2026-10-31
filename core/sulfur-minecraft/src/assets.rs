//! Asset indexes (`assets/indexes/<id>.json`).

use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;

pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    pub objects: BTreeMap<String, AssetObject>,
    /// Pre-1.7 indexes: assets must also exist under `assets/virtual/<id>`.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// Very old versions read assets from `<game dir>/resources`.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    /// `ab/abcdef...`, the object's path below `assets/objects` and the CDN root.
    pub fn path_fragment(&self) -> String {
        format!("{}/{}", &self.hash[..2.min(self.hash.len())], self.hash)
    }

    pub fn url(&self) -> String {
        format!("{RESOURCES_URL}{}", self.path_fragment())
    }
}

impl AssetIndex {
    pub fn read(path: &Path) -> crate::Result<Self> {
        crate::error::read_json(path)
    }
}
