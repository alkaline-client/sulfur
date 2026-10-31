//! Where this machine is, as far as account rules are concerned.
//!
//! Mojang does not sell Minecraft: Java Edition in mainland China (NetEase
//! holds the license there), so offline accounts are the way players there
//! play it. Everywhere else an offline account needs a Microsoft account that
//! owns the game. Like HMCL, the region comes from the system time zone,
//! with the locale's region only breaking ties the time zone cannot settle.

/// Time zones whose only region is mainland China (tzdb `zone1970.tab`,
/// plus the aliases in `backward`). These are tzdb identifiers, spelled as
/// the operating system reports them.
const MAINLAND_ZONES: &[&str] = &[
    "Asia/Shanghai",
    "Asia/Urumqi",
    "Asia/Chongqing",
    "Asia/Chungking",
    "Asia/Harbin",
    "Asia/Kashgar",
    "PRC",
];

/// Time zones of other regions that share UTC+8 with mainland China, so a
/// `zh-CN` locale there does not make the machine mainland. Also tzdb
/// identifiers, legacy aliases included.
const NEARBY_ZONES: &[&str] = &[
    "Asia/Hong_Kong",
    "Hongkong",
    "Asia/Macau",
    "Asia/Macao",
    "Asia/Taipei",
    "ROC",
    "Asia/Singapore",
    "Singapore",
    "Asia/Kuala_Lumpur",
    "Asia/Manila",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    MainlandChina,
    Elsewhere,
}

impl Region {
    /// The region of this machine.
    pub fn detect() -> Self {
        let zone = iana_time_zone::get_timezone().ok();
        let locale = sys_locale::get_locale();
        let region = Self::from_parts(zone.as_deref(), locale.as_deref());
        tracing::info!(?zone, ?locale, ?region, "detected region");
        region
    }

    /// Decides from an IANA time zone id and a BCP 47 locale (`zh-CN`,
    /// `zh_CN.UTF-8`, `zh-Hans-CN`). A known time zone wins; the locale
    /// only counts when the zone is missing or unknown.
    pub fn from_parts(zone: Option<&str>, locale: Option<&str>) -> Self {
        match zone {
            Some(z) if MAINLAND_ZONES.contains(&z) => Self::MainlandChina,
            // `Etc/UTC` and friends say nothing about where the machine is.
            Some(z) if NEARBY_ZONES.contains(&z) || (z.contains('/') && !z.starts_with("Etc/")) => Self::Elsewhere,
            _ if locale.and_then(locale_region).as_deref() == Some("CN") => Self::MainlandChina,
            _ => Self::Elsewhere,
        }
    }
}

/// The region subtag of a locale: `zh-Hans-CN` → `CN`, `zh_CN.UTF-8` → `CN`.
fn locale_region(locale: &str) -> Option<String> {
    let tag = locale.split(['.', '@']).next()?;
    tag.split(['-', '_'])
        .skip(1)
        .find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|part| part.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_zone_decides() {
        assert_eq!(Region::from_parts(Some("Asia/Shanghai"), Some("en-US")), Region::MainlandChina);
        assert_eq!(Region::from_parts(Some("Asia/Urumqi"), None), Region::MainlandChina);
        assert_eq!(Region::from_parts(Some("Asia/Hong_Kong"), Some("zh-CN")), Region::Elsewhere);
        assert_eq!(Region::from_parts(Some("Asia/Taipei"), Some("zh-TW")), Region::Elsewhere);
        assert_eq!(Region::from_parts(Some("Europe/Berlin"), Some("zh-CN")), Region::Elsewhere);
    }

    #[test]
    fn locale_settles_unknown_zones() {
        assert_eq!(Region::from_parts(None, Some("zh_CN.UTF-8")), Region::MainlandChina);
        assert_eq!(Region::from_parts(Some("UTC"), Some("zh-Hans-CN")), Region::MainlandChina);
        assert_eq!(Region::from_parts(Some("Etc/UTC"), Some("zh-CN")), Region::MainlandChina);
        assert_eq!(Region::from_parts(Some("UTC"), Some("zh-HK")), Region::Elsewhere);
        assert_eq!(Region::from_parts(None, None), Region::Elsewhere);
    }
}
