use std::{fs::File, io, io::Read, path::Path};

use sha1::{Digest, Sha1};
use sha2::Sha256;

/// Expected digest of a downloaded file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Checksum {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl Checksum {
    /// Parses a hex-encoded SHA-1 digest, as used throughout Mojang's metadata.
    pub fn sha1_hex(hex: &str) -> Option<Self> {
        let mut out = [0u8; 20];
        hex::decode_to_slice(hex.trim(), &mut out).ok()?;
        Some(Self::Sha1(out))
    }

    /// Parses a hex-encoded SHA-256 digest.
    pub fn sha256_hex(hex: &str) -> Option<Self> {
        let mut out = [0u8; 32];
        hex::decode_to_slice(hex.trim(), &mut out).ok()?;
        Some(Self::Sha256(out))
    }

    pub(crate) fn hasher(&self) -> Hasher {
        match self {
            Self::Sha1(_) => Hasher::Sha1(Sha1::new()),
            Self::Sha256(_) => Hasher::Sha256(Sha256::new()),
        }
    }
}

impl std::fmt::Display for Checksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sha1(d) => write!(f, "sha1:{}", hex::encode(d)),
            Self::Sha256(d) => write!(f, "sha256:{}", hex::encode(d)),
        }
    }
}

/// Incremental hasher matching a [`Checksum`] variant.
pub(crate) enum Hasher {
    Sha1(Sha1),
    Sha256(Sha256),
}

impl Hasher {
    pub(crate) fn update(&mut self, data: &[u8]) {
        match self {
            Self::Sha1(h) => h.update(data),
            Self::Sha256(h) => h.update(data),
        }
    }

    pub(crate) fn matches(self, expected: &Checksum) -> bool {
        match (self, expected) {
            (Self::Sha1(h), Checksum::Sha1(e)) => h.finalize().as_slice() == e,
            (Self::Sha256(h), Checksum::Sha256(e)) => h.finalize().as_slice() == e,
            _ => false,
        }
    }
}

/// Hashes the file at `path` and compares it with `expected`.
///
/// This is blocking; call it from `spawn_blocking` in async code.
pub fn verify_file(path: &Path, expected: &Checksum) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut hasher = expected.hasher();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.matches(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_verifies_sha1() {
        // sha1("hello")
        let c = Checksum::sha1_hex("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
        let mut h = c.hasher();
        h.update(b"hello");
        assert!(h.matches(&c));
        assert!(Checksum::sha1_hex("zz").is_none());
    }
}
