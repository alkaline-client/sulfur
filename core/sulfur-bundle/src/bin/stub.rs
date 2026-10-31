//! Sulfur's single executable: unpacks the app folder appended to it (once
//! per build) and starts the app's own launcher with the same arguments.

// No console window on Windows; the app is a GUI program.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
};

use sulfur_bundle::{Payload, read_payload, unpack_root};

/// Marks a folder as completely unpacked.
const COMPLETE: &str = ".complete";

fn main() {
    if let Err(error) = run() {
        report(&format!("Sulfur could not start: {error}"));
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut file = File::open(&exe)?;
    let payload = read_payload(&mut file)?;
    let root = unpack_root()?;
    let dir = root.join(&payload.meta.id);
    if !dir.join(COMPLETE).is_file() {
        unpack(&mut file, &payload, &root, &dir)?;
        remove_older(&root, &payload.meta.id);
    }
    start(&dir.join(&payload.meta.entry), &exe, std::env::args_os().skip(1).collect())
}

/// Unpacks into a temporary folder first, so an interrupted run never leaves
/// a half-unpacked app behind that looks complete.
fn unpack(file: &mut File, payload: &Payload, root: &Path, dir: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let partial = root.join(format!(".{}.{}.partial", payload.meta.id, std::process::id()));
    let _ = fs::remove_dir_all(&partial);
    file.seek(SeekFrom::Start(payload.offset))?;
    let decoder = ruzstd::decoding::StreamingDecoder::new(io::BufReader::new(file.take(payload.len))).map_err(io::Error::other)?;
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.set_overwrite(true);
    let result = archive.unpack(&partial).and_then(|()| File::create(partial.join(COMPLETE)).map(drop));
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&partial);
        return Err(error);
    }
    match fs::rename(&partial, dir) {
        Ok(()) => Ok(()),
        // Another copy started at the same time and won the race.
        Err(_) if dir.join(COMPLETE).is_file() => {
            let _ = fs::remove_dir_all(&partial);
            Ok(())
        }
        Err(error) => {
            // A stale, incomplete folder of the same build is in the way.
            let _ = fs::remove_dir_all(dir);
            fs::rename(&partial, dir).inspect_err(|_| {
                let _ = fs::remove_dir_all(&partial);
            }).map_err(|_| error)
        }
    }
}

/// Removes other builds' folders; one still running (Windows keeps its files
/// locked) is left for next time.
fn remove_older(root: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let stale_partial = name.to_string_lossy().ends_with(".partial");
        if name != keep && (stale_partial || entry.path().join(COMPLETE).is_file()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// The app, told where this executable is (`SULFUR_EXECUTABLE`) so it can
/// point desktop entries at it rather than at the unpacked copy.
fn command(program: &PathBuf, exe: &Path, args: Vec<OsString>) -> Command {
    let mut command = Command::new(program);
    command.args(args).env("SULFUR_EXECUTABLE", exe);
    command
}

#[cfg(unix)]
fn start(program: &PathBuf, exe: &Path, args: Vec<OsString>) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    // Replaces this process, so exit codes and signals reach the caller as usual.
    Err(command(program, exe, args).exec())
}

#[cfg(windows)]
fn start(program: &PathBuf, exe: &Path, args: Vec<OsString>) -> io::Result<()> {
    command(program, exe, args).spawn().map(drop)
}

#[cfg(windows)]
fn report(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(message), wide("Sulfur"));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
}

#[cfg(not(windows))]
fn report(message: &str) {
    eprintln!("{message}");
}
