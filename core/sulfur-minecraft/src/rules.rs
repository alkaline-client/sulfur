//! Mojang's allow/disallow rules for libraries and arguments.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::platform::Platform;

#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// A regex over the OS version. Only used for cosmetic JVM flags on old
    /// Windows releases, so it is treated as always matching.
    pub version: Option<String>,
}

/// What rules are evaluated against.
#[derive(Clone, Debug)]
pub struct RuleContext {
    pub platform: Platform,
    /// Enabled launcher features, e.g. `has_custom_resolution`.
    pub features: BTreeSet<String>,
}

impl RuleContext {
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            features: BTreeSet::new(),
        }
    }

    pub fn with_feature(mut self, feature: &str) -> Self {
        self.features.insert(feature.to_owned());
        self
    }
}

impl Rule {
    fn matches(&self, ctx: &RuleContext) -> bool {
        if let Some(os) = &self.os {
            if os.name.as_deref().is_some_and(|n| n != ctx.platform.os.mojang_name()) {
                return false;
            }
            if os.arch.as_deref().is_some_and(|a| !ctx.platform.arch.matches_rule(a)) {
                return false;
            }
        }
        self.features
            .iter()
            .all(|(name, &wanted)| ctx.features.contains(name) == wanted)
    }
}

/// An empty rule list allows; otherwise the last matching rule decides.
pub fn allowed(rules: &[Rule], ctx: &RuleContext) -> bool {
    if rules.is_empty() {
        return true;
    }
    rules
        .iter()
        .rev()
        .find(|r| r.matches(ctx))
        .is_some_and(|r| r.action == RuleAction::Allow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Os};

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn evaluates_os_arch_and_features() {
        let linux = RuleContext::new(Platform { os: Os::Linux, arch: Arch::X86_64 });
        let mac = RuleContext::new(Platform { os: Os::MacOs, arch: Arch::Arm64 });
        let not_mac = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert!(allowed(&not_mac, &linux));
        assert!(!allowed(&not_mac, &mac));
        assert!(!allowed(&rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#), &linux));
        let res = rules(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
        assert!(!allowed(&res, &linux));
        assert!(allowed(&res, &linux.clone().with_feature("has_custom_resolution")));
    }
}
