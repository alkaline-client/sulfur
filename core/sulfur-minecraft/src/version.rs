//! Per-version metadata (`versions/<id>/<id>.json`).

use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;

use crate::rules::Rule;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub main_class: Option<String>,
    /// Pre-1.13 game arguments as one whitespace-separated string.
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    /// Version whose jar is used; set by mod loaders and by [`VersionJson::inherit`].
    #[serde(default)]
    pub jar: Option<String>,
    /// Log4j configuration; Mojang's current files are patched against Log4Shell.
    #[serde(default)]
    pub logging: Option<Logging>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Logging {
    #[serde(default)]
    pub client: Option<LoggingConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingConfig {
    /// JVM argument with a `${path}` placeholder, e.g. `-Dlog4j.configurationFile=${path}`.
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn values(&self) -> &[String] {
        match self {
            Self::One(v) => std::slice::from_ref(v),
            Self::Many(v) => v,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VersionDownloads {
    pub client: Option<Artifact>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    pub major_version: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    /// Maven coordinate `group:artifact:version[:classifier][@ext]`.
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Legacy natives: OS name to classifier (may contain `${arch}`).
    #[serde(default)]
    pub natives: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
    /// Maven repository base for libraries without `downloads` (mod loaders).
    #[serde(default)]
    pub url: Option<String>,
    /// Checksum and size of the artifact for libraries without `downloads` (Fabric metadata).
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: BTreeMap<String, Artifact>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl VersionJson {
    pub fn read(path: &Path) -> crate::Result<Self> {
        crate::error::read_json(path)
    }

    /// Applies a parent version (`inheritsFrom`): the child's values win,
    /// libraries are child-first and arguments are appended.
    pub fn inherit(mut self, parent: VersionJson) -> VersionJson {
        let mut libraries = std::mem::take(&mut self.libraries);
        libraries.extend(parent.libraries);
        let arguments = match (parent.arguments, self.arguments.take()) {
            (Some(mut p), Some(c)) => {
                p.game.extend(c.game);
                p.jvm.extend(c.jvm);
                Some(p)
            }
            (p, c) => c.or(p),
        };
        VersionJson {
            jar: self.jar.or(parent.jar).or(Some(parent.id)),
            inherits_from: None,
            kind: self.kind.or(parent.kind),
            main_class: self.main_class.or(parent.main_class),
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            arguments,
            asset_index: self.asset_index.or(parent.asset_index),
            assets: self.assets.or(parent.assets),
            downloads: self.downloads.or(parent.downloads),
            libraries,
            java_version: self.java_version.or(parent.java_version),
            logging: self.logging.or(parent.logging),
            id: self.id,
        }
    }

    /// Major Java version this version needs (Java 8 when unspecified).
    pub fn java_major(&self) -> u32 {
        self.java_version.as_ref().map_or(8, |j| j.major_version)
    }

    /// `(major, minor)` of the Log4j 2 core library on the classpath, if any.
    /// `2.0-beta9` (Minecraft 1.7–1.11) is `(2, 0)`.
    pub fn log4j_version(&self) -> Option<(u32, u32)> {
        let lib = self.libraries.iter().find(|l| l.name.starts_with("org.apache.logging.log4j:log4j-core:"))?;
        let version = lib.name.split(':').nth(2)?;
        let mut numbers = version.split('-').next()?.split('.').map(|n| n.parse::<u32>().ok());
        Some((numbers.next()??, numbers.next().flatten().unwrap_or(0)))
    }

    /// Id of the version whose jar is launched.
    pub fn jar_id(&self) -> &str {
        self.jar.as_deref().unwrap_or(&self.id)
    }
}
