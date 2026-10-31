//! `sulfur-pack <stub> <app folder> <entry> <output>`: writes the stub with the
//! app folder appended, as one executable. `<entry>` is the app's launcher
//! relative to the folder's parent, e.g. `Sulfur/bin/Sulfur`.

use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use sulfur_bundle::{Meta, trailer};

/// Strong compression: the file is downloaded once per release, unpacked once per build.
const LEVEL: i32 = 19;

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [stub, app, entry, output] = args.as_slice() else {
        eprintln!("usage: sulfur-pack <stub> <app folder> <entry> <output>");
        std::process::exit(2);
    };
    if let Err(error) = pack(stub, app, &entry.to_string_lossy(), output) {
        eprintln!("sulfur-pack: {error}");
        std::process::exit(1);
    }
}

fn pack(stub: &Path, app: &Path, entry: &str, output: &Path) -> io::Result<()> {
    let name = app.file_name().ok_or_else(|| io::Error::other("the app folder has no name"))?;
    if !app.parent().unwrap_or(Path::new(".")).join(entry).is_file() {
        return Err(io::Error::other(format!("{entry} is not in {}", app.display())));
    }
    let mut out = BufWriter::new(File::create(output)?);
    io::copy(&mut File::open(stub)?, &mut out)?;

    let mut counted = Counted { inner: &mut out, len: 0, hash: Sha256::new() };
    {
        let mut encoder = zstd::Encoder::new(&mut counted, LEVEL)?;
        encoder.multithread(std::thread::available_parallelism().map_or(1, |n| n.get() as u32))?;
        let mut tar = tar::Builder::new(encoder);
        // The macOS app bundle has symlinks inside; keep them as links.
        tar.follow_symlinks(false);
        tar.append_dir_all(name, app)?;
        tar.into_inner()?.finish()?;
    }
    let (len, hash) = (counted.len, counted.hash.finalize());
    let meta = Meta { id: hex::encode(&hash[..8]), entry: entry.replace('\\', "/") };
    out.write_all(&trailer(&meta, len))?;
    out.into_inner().map_err(|e| e.into_error())?.sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o755))?;
    }
    println!("{} ({:.1} MiB, build {})", output.display(), std::fs::metadata(output)?.len() as f64 / 1048576.0, meta.id);
    Ok(())
}

/// Counts and hashes what passes through.
struct Counted<W> {
    inner: W,
    len: u64,
    hash: Sha256,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.len += n as u64;
        self.hash.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
