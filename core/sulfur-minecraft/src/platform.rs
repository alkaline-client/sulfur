/// Operating system as named in Mojang's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }

    pub fn mojang_name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::MacOs => "osx",
            Self::Linux => "linux",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    Arm64,
    Other,
}

impl Arch {
    pub fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Self::Arm64
        } else if cfg!(target_arch = "x86") {
            Self::X86
        } else {
            Self::Other
        }
    }

    /// Value substituted for `${arch}` in legacy native classifiers.
    pub fn bits(self) -> &'static str {
        match self {
            Self::X86 => "32",
            _ => "64",
        }
    }

    /// Matches the `os.arch` field of a rule.
    pub fn matches_rule(self, arch: &str) -> bool {
        match arch {
            "x86" => self == Self::X86,
            "x86_64" | "amd64" | "x64" => self == Self::X86_64,
            "arm64" | "aarch64" => self == Self::Arm64,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    pub fn current() -> Self {
        Self {
            os: Os::current(),
            arch: Arch::current(),
        }
    }

    pub fn classpath_separator(self) -> &'static str {
        if self.os == Os::Windows { ";" } else { ":" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rule_arch_spellings() {
        // Cleanroom writes "x64"; Mojang "x86"; others "amd64" or "aarch64".
        for name in ["x86_64", "amd64", "x64"] {
            assert!(Arch::X86_64.matches_rule(name));
        }
        assert!(Arch::Arm64.matches_rule("arm64") && Arch::Arm64.matches_rule("aarch64"));
        assert!(!Arch::Arm64.matches_rule("x64") && !Arch::X86_64.matches_rule("arm32"));
    }
}
