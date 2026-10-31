//! Download sources: official servers, the BMCLAPI mirror for the game and
//! loaders, and the MCIM mirror for Modrinth and CurseForge.

use sulfur_core::DownloadSource;
use sulfur_download::{MirrorRule, MirrorSet};

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/";
/// MCIM (mcmod-info-mirror) mirrors the Modrinth and CurseForge APIs and caches
/// their files for mainland China; elsewhere its file URLs redirect to the originals.
const MCIM: &str = "https://mod.mcimirror.top/";

/// Mirror rules for a [`DownloadSource`].
pub fn mirror_set(source: DownloadSource) -> MirrorSet {
    let rules = || {
        vec![
            MirrorRule::new("https://piston-meta.mojang.com/", BMCLAPI),
            MirrorRule::new("https://piston-data.mojang.com/", BMCLAPI),
            MirrorRule::new("https://launchermeta.mojang.com/", BMCLAPI),
            MirrorRule::new("https://launcher.mojang.com/", BMCLAPI),
            MirrorRule::new("https://libraries.minecraft.net/", format!("{BMCLAPI}maven/")),
            MirrorRule::new("https://resources.download.minecraft.net/", format!("{BMCLAPI}assets/")),
            MirrorRule::new("https://maven.minecraftforge.net/", format!("{BMCLAPI}maven/")),
            MirrorRule::new("https://maven.neoforged.net/releases/", format!("{BMCLAPI}maven/")),
            MirrorRule::new("https://maven.fabricmc.net/", format!("{BMCLAPI}maven/")),
            MirrorRule::new("https://meta.fabricmc.net/", format!("{BMCLAPI}fabric-meta/")),
            MirrorRule::new("https://api.modrinth.com/", format!("{MCIM}modrinth/")),
            MirrorRule::new("https://cdn.modrinth.com/", MCIM),
            MirrorRule::new("https://mediafilez.forgecdn.net/", MCIM),
            MirrorRule::new("https://edge.forgecdn.net/", MCIM),
        ]
    };
    match source {
        DownloadSource::Official => MirrorSet::default(),
        DownloadSource::Auto => MirrorSet::new(rules(), false),
        DownloadSource::Mirror => MirrorSet::new(rules(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_mod_sites() {
        let set = mirror_set(DownloadSource::Mirror);
        assert_eq!(
            set.sources("https://api.modrinth.com/v2/search?query=x")[0],
            "https://mod.mcimirror.top/modrinth/v2/search?query=x"
        );
        assert_eq!(
            set.sources("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar")[0],
            "https://mod.mcimirror.top/data/AANobbMI/versions/x/sodium.jar"
        );
        assert_eq!(
            set.sources("https://mediafilez.forgecdn.net/files/9322/591/a.jar")[0],
            "https://mod.mcimirror.top/files/9322/591/a.jar"
        );
        assert_eq!(mirror_set(DownloadSource::Official).sources("https://cdn.modrinth.com/x").len(), 1);
    }
}
