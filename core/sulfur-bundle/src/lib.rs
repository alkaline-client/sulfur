//! Sulfur as one executable.
//!
//! Compose Desktop builds an app folder: a trimmed Java runtime, the jars and
//! the Rust core. `sulfur-pack` appends that folder, as a zstd-compressed tar,
//! to the small `sulfur-stub` program; the stub unpacks it once into the
//! cache and starts the app's own launcher from there.
//!
//! Layout of the executable:
//!
//! ```text
//! stub | payload (tar.zst) | meta (JSON) | meta length (u64 LE) | payload length (u64 LE) | MAGIC
//! ```

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 8] = b"SULFURPK";
const TRAILER: u64 = 8 + 8 + 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// Names the unpacked folder; changes whenever the payload does.
    pub id: String,
    /// The app's launcher, relative to the unpacked folder.
    pub entry: String,
}

/// Where the payload sits in the executable.
#[derive(Debug)]
pub struct Payload {
    pub meta: Meta,
    pub offset: u64,
    pub len: u64,
}

/// Reads the trailer of a packed executable.
pub fn read_payload(file: &mut File) -> io::Result<Payload> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("not a packed Sulfur executable: {what}"));
    let size = file.seek(SeekFrom::End(0))?;
    if size < TRAILER {
        return Err(invalid("too small"));
    }
    file.seek(SeekFrom::End(-(TRAILER as i64)))?;
    let mut trailer = [0u8; TRAILER as usize];
    file.read_exact(&mut trailer)?;
    if &trailer[16..] != MAGIC {
        return Err(invalid("no payload"));
    }
    let meta_len = u64::from_le_bytes(trailer[0..8].try_into().unwrap());
    let len = u64::from_le_bytes(trailer[8..16].try_into().unwrap());
    let offset = size
        .checked_sub(TRAILER + meta_len + len)
        .ok_or_else(|| invalid("lengths out of range"))?;
    file.seek(SeekFrom::Start(offset + len))?;
    let mut meta = vec![0u8; meta_len as usize];
    file.read_exact(&mut meta)?;
    let meta: Meta = serde_json::from_slice(&meta).map_err(|_| invalid("bad metadata"))?;
    if meta.id.is_empty() || meta.id.contains(['/', '\\', '.']) || meta.entry.contains("..") {
        return Err(invalid("bad metadata"));
    }
    Ok(Payload { meta, offset, len })
}

/// The trailer for a payload of `len` bytes, written after it.
pub fn trailer(meta: &Meta, len: u64) -> Vec<u8> {
    let json = serde_json::to_vec(meta).expect("metadata serializes");
    let mut out = json.clone();
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(MAGIC);
    out
}

/// Folder the app is unpacked into: under `SULFUR_PORTABLE` when set (like the
/// app's own data), else in the user's cache.
pub fn unpack_root() -> io::Result<PathBuf> {
    if let Some(portable) = std::env::var_os("SULFUR_PORTABLE").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(portable).join("cache").join("app"));
    }
    let dirs = directories::ProjectDirs::from("dev", "Sulfur", "Sulfur")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(dirs.cache_dir().join("app"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_back_what_pack_writes() {
        let path = std::env::temp_dir().join(format!("sulfur-bundle-test-{}", std::process::id()));
        let meta = Meta { id: "abc123".into(), entry: "Sulfur/bin/Sulfur".into() };
        let mut file = File::create(&path).unwrap();
        file.write_all(b"stub bytes").unwrap();
        file.write_all(b"payload").unwrap();
        file.write_all(&trailer(&meta, 7)).unwrap();
        drop(file);
        let payload = read_payload(&mut File::open(&path).unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(payload.meta, meta);
        assert_eq!((payload.offset, payload.len), (10, 7));
    }

    #[test]
    fn rejects_a_plain_file() {
        let path = std::env::temp_dir().join(format!("sulfur-bundle-plain-{}", std::process::id()));
        std::fs::write(&path, b"just a program, nothing appended to it").unwrap();
        let result = read_payload(&mut File::open(&path).unwrap());
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }
}
