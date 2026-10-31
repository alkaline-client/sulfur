/// Rewrites a URL prefix to an alternative host, e.g. an official CDN to a mirror.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorRule {
    pub from: String,
    pub to: String,
}

impl MirrorRule {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// Expands one URL into an ordered list of sources to try.
#[derive(Clone, Debug, Default)]
pub struct MirrorSet {
    rules: Vec<MirrorRule>,
    prefer_mirror: bool,
}

impl MirrorSet {
    /// `prefer_mirror` puts mirrored URLs before the original; otherwise mirrors
    /// are only used as fallbacks.
    pub fn new(rules: Vec<MirrorRule>, prefer_mirror: bool) -> Self {
        Self {
            rules,
            prefer_mirror,
        }
    }

    pub fn sources(&self, url: &str) -> Vec<String> {
        let mirrored = self
            .rules
            .iter()
            .filter_map(|r| url.strip_prefix(r.from.as_str()).map(|rest| format!("{}{rest}", r.to)));
        let mut out = Vec::with_capacity(1 + self.rules.len());
        if self.prefer_mirror {
            out.extend(mirrored);
            out.push(url.to_owned());
        } else {
            out.push(url.to_owned());
            out.extend(mirrored);
        }
        let mut seen = std::collections::HashSet::new();
        out.retain(|u| seen.insert(u.clone()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_sources_by_preference() {
        let rules = vec![MirrorRule::new("https://a.example/", "https://m.example/a/")];
        let fallback = MirrorSet::new(rules.clone(), false);
        assert_eq!(
            fallback.sources("https://a.example/x.jar"),
            ["https://a.example/x.jar", "https://m.example/a/x.jar"]
        );
        let preferred = MirrorSet::new(rules, true);
        assert_eq!(preferred.sources("https://a.example/x.jar")[0], "https://m.example/a/x.jar");
        assert_eq!(preferred.sources("https://b.example/y").len(), 1);
    }
}
