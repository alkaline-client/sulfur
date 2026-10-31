//! The desktop wallpaper, so the theme can take its color from it.
//!
//! Each platform keeps it somewhere else: Windows in the registry and its
//! transcoded copy, macOS behind `NSWorkspace`, and every Linux desktop in its
//! own settings (GNOME, Cinnamon and MATE in GSettings, KDE Plasma, COSMIC,
//! Xfce, LXQt and LXDE in their config files). Wayland compositors without a
//! desktop of their own (Hyprland, Niri, Sway, River…) leave it to a separate
//! program: hyprpaper, swww/awww, wpaperd, swaybg, wbg, waypaper, feh or nitrogen.
//!
//! Everything here is best effort: `None` means "unknown", never an error.

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Wallpaper {
    /// A picture file.
    Image(PathBuf),
    /// A plain color, `0xRRGGBB`.
    Color(u32),
}

/// The current wallpaper of the primary screen, if it can be found.
pub fn detect() -> Option<Wallpaper> {
    #[cfg(windows)]
    let found = windows::detect();
    #[cfg(target_os = "macos")]
    let found = macos::detect();
    #[cfg(all(unix, not(target_os = "macos")))]
    let found = linux::detect(&linux::Env::current());
    tracing::debug!(?found, "desktop wallpaper");
    found
}

/// Extensions Skia (and so the UI) decodes directly.
const DECODABLE: &[&str] = &["png", "jpg", "jpeg", "jpe", "webp", "gif", "bmp", "ico", "wbmp"];

/// Extensions worth taking from a folder of wallpapers.
const PICTURES: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "jxl", "avif", "heic", "heif", "tif", "tiff", "svg"];

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

/// A copy of `picture` the UI can decode, at most 1024 pixels wide, made in
/// `cache_dir` when its format needs converting (HEIC, JPEG XL, AVIF, TIFF,
/// SVG, a macOS video wallpaper). Converters are whatever the system has:
/// QuickLook and `sips` on macOS, ImageMagick, `djxl` or `rsvg-convert` elsewhere.
pub fn decodable(picture: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let ext = extension(picture);
    // Windows' TranscodedWallpaper has no extension and is a JPEG or BMP.
    if ext.is_empty() || DECODABLE.contains(&ext.as_str()) {
        return Some(picture.to_owned());
    }
    let modified = std::fs::metadata(picture).and_then(|m| m.modified()).ok()?;
    let stamp = modified.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
    let key = format!("{}-{stamp}", picture.to_string_lossy());
    let out = cache_dir.join(format!("wallpaper-{:016x}.png", fnv1a(key.as_bytes())));
    if out.is_file() {
        return Some(out);
    }
    std::fs::create_dir_all(cache_dir).ok()?;
    let (input, output) = (picture.to_string_lossy().into_owned(), out.to_string_lossy().into_owned());
    let attempts: Vec<(&str, Vec<String>)> = vec![
        ("sips", vec!["-s".into(), "format".into(), "png".into(), "-Z".into(), "1024".into(), input.clone(), "--out".into(), output.clone()]),
        ("magick", vec![format!("{input}[0]"), "-resize".into(), "1024x1024>".into(), output.clone()]),
        ("convert", vec![format!("{input}[0]"), "-resize".into(), "1024x1024>".into(), output.clone()]),
        ("djxl", vec![input.clone(), output.clone()]),
        ("rsvg-convert", vec!["-w".into(), "1024".into(), "-o".into(), output.clone(), input.clone()]),
    ];
    for (program, args) in attempts {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if run(program, &args).is_some() && out.is_file() {
            return Some(out);
        }
    }
    // QuickLook thumbnails everything macOS can show, videos included; it names the file itself.
    if cfg!(target_os = "macos") {
        let dir = out.with_extension("ql");
        let _ = std::fs::create_dir_all(&dir);
        if run("qlmanage", &["-t", "-s", "1024", "-o", &dir.to_string_lossy(), &input]).is_some() {
            let name = format!("{}.png", picture.file_name()?.to_string_lossy());
            if std::fs::rename(dir.join(name), &out).is_ok() {
                let _ = std::fs::remove_dir_all(&dir);
                return Some(out);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    None
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// Runs a short command and returns its standard output, or `None` when it
/// is missing, fails or takes longer than three seconds.
fn run(program: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

/// A picture file, or a picture inside a folder of them (a slideshow, a KDE
/// wallpaper package).
fn picture(path: &Path) -> Option<Wallpaper> {
    if path.is_file() {
        return Some(Wallpaper::Image(path.to_owned()));
    }
    if !path.is_dir() {
        return None;
    }
    // KDE packages: contents/images/<width>x<height>.<ext>; take the largest.
    let images = path.join("contents/images");
    if images.is_dir() {
        let best = pictures_in(&images).into_iter().max_by_key(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (w, h) = stem.split_once('x').unwrap_or_default();
            let decodable = DECODABLE.contains(&extension(p).as_str());
            (decodable, w.parse::<u64>().unwrap_or(0) * h.parse::<u64>().unwrap_or(0))
        });
        return best.map(Wallpaper::Image);
    }
    pictures_in(path).into_iter().next().map(Wallpaper::Image)
}

fn pictures_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && PICTURES.contains(&extension(p).as_str()))
        .collect();
    found.sort();
    found
}

/// `file:///a%20b` → `/a b`; plain paths pass through.
fn from_uri(value: &str) -> PathBuf {
    let Some(rest) = value.strip_prefix("file://") else { return PathBuf::from(value) };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

/// `#rgb`, `#rrggbb` or `#rrrrggggbbbb` (GSettings colors).
fn parse_hex_color(value: &str) -> Option<u32> {
    let hex = value.trim().trim_matches('\'').strip_prefix('#')?;
    let channel = |s: &str| u32::from_str_radix(s, 16).ok();
    let (r, g, b) = match hex.len() {
        3 => (channel(&hex[0..1])? * 17, channel(&hex[1..2])? * 17, channel(&hex[2..3])? * 17),
        6 => (channel(&hex[0..2])?, channel(&hex[2..4])?, channel(&hex[4..6])?),
        12 => (channel(&hex[0..2])?, channel(&hex[4..6])?, channel(&hex[8..10])?),
        _ => return None,
    };
    Some((r << 16) | (g << 8) | b)
}

#[cfg_attr(not(windows), allow(dead_code))]
mod windows {
    use super::*;

    /// Windows 10 and 11: no picture means a solid color (`Colors\Background`).
    /// Otherwise the transcoded copy is the picture actually shown (it follows
    /// slideshows and Spotlight); the registry path is the fallback.
    pub fn detect() -> Option<Wallpaper> {
        let path = run("reg", &["query", r"HKCU\Control Panel\Desktop", "/v", "WallPaper"])
            .and_then(|out| reg_value(&out, "WallPaper"));
        if path.as_deref().is_some_and(|p| p.trim().is_empty()) {
            return run("reg", &["query", r"HKCU\Control Panel\Colors", "/v", "Background"])
                .and_then(|out| reg_value(&out, "Background"))
                .and_then(|rgb| parse_rgb_triplet(&rgb))
                .map(Wallpaper::Color);
        }
        let transcoded = std::env::var_os("APPDATA")
            .map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Themes\TranscodedWallpaper"));
        transcoded
            .filter(|p| p.is_file())
            .map(Wallpaper::Image)
            .or_else(|| path.and_then(|p| picture(Path::new(p.trim()))))
    }

    /// The value of `name` in `reg query` output (`    name    REG_SZ    value`).
    pub fn reg_value(output: &str, name: &str) -> Option<String> {
        output.lines().find_map(|line| {
            let rest = line.trim_start().strip_prefix(name)?;
            let rest = rest.trim_start();
            let ty_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            rest[..ty_end].starts_with("REG_").then(|| rest[ty_end..].trim().to_owned())
        })
    }

    /// `"58 110 165"` → `0x3A6EA5`.
    pub fn parse_rgb_triplet(value: &str) -> Option<u32> {
        let parts: Vec<u32> = value.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
        match parts[..] {
            [r, g, b] if r < 256 && g < 256 && b < 256 => Some((r << 16) | (g << 8) | b),
            _ => None,
        }
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos {
    use super::*;

    /// `NSWorkspace`'s answer through JavaScript for Automation, which needs no
    /// permission (it sends no Apple events). Before macOS 14 the Dock's
    /// database is the fallback.
    pub fn detect() -> Option<Wallpaper> {
        const SCRIPT: &str = "ObjC.import('AppKit'); \
            $.NSWorkspace.sharedWorkspace.desktopImageURLForScreen($.NSScreen.mainScreen).path.js";
        if let Some(path) = run("osascript", &["-l", "JavaScript", "-e", SCRIPT]).map(|p| p.trim().to_owned())
            && let Some(found) = picture(Path::new(&path))
        {
            return Some(found);
        }
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let db = home.join("Library/Application Support/Dock/desktoppicture.db");
        let value = run("sqlite3", &[&db.to_string_lossy(), "SELECT value FROM data ORDER BY rowid DESC LIMIT 1"])?;
        picture(&super::linux::expand_home(value.trim(), &home))
    }
}

#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
mod linux {
    use super::*;

    /// What the probes look at, so tests can point them at a fake home.
    pub struct Env {
        pub home: PathBuf,
        pub config: PathBuf,
        pub state: PathBuf,
        /// Lowercased names from `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP` and `DESKTOP_SESSION`.
        pub desktops: Vec<String>,
    }

    impl Env {
        pub fn current() -> Self {
            let var = |name: &str| std::env::var(name).unwrap_or_default();
            let home = PathBuf::from(var("HOME"));
            let dir = |name: &str, default: &str| {
                std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(default))
            };
            let desktops = [var("XDG_CURRENT_DESKTOP"), var("XDG_SESSION_DESKTOP"), var("DESKTOP_SESSION")]
                .iter()
                .flat_map(|v| v.split(':').map(|d| d.trim().to_ascii_lowercase()).collect::<Vec<_>>())
                .filter(|d| !d.is_empty())
                .collect();
            Self { config: dir("XDG_CONFIG_HOME", ".config"), state: dir("XDG_STATE_HOME", ".local/state"), home, desktops }
        }

        fn is(&self, names: &[&str]) -> bool {
            self.desktops.iter().any(|d| names.iter().any(|n| d.contains(n)))
        }
    }

    type Probe = fn(&Env) -> Option<Wallpaper>;

    pub fn detect(env: &Env) -> Option<Wallpaper> {
        // The running desktop's own setting first; then the stand-alone
        // wallpaper programs; GSettings last, since it answers GNOME's
        // default even where GNOME is not the desktop.
        let own: &[Probe] = if env.is(&["cosmic"]) {
            &[cosmic]
        } else if env.is(&["kde", "plasma"]) {
            &[kde]
        } else if env.is(&["xfce"]) {
            &[xfce]
        } else if env.is(&["cinnamon"]) {
            &[cinnamon]
        } else if env.is(&["mate"]) {
            &[mate]
        } else if env.is(&["lxqt"]) {
            &[lxqt]
        } else if env.is(&["lxde"]) {
            &[lxde]
        } else if env.is(&["gnome", "unity", "pantheon", "budgie", "ubuntu", "deepin"]) {
            &[gnome]
        } else if env.is(&["hyprland"]) {
            &[hyprpaper]
        } else {
            &[]
        };
        let standalone: &[Probe] = &[swww, wpaperd, running_programs, hyprpaper, waypaper, feh, nitrogen];
        let desktops: &[Probe] = &[cosmic, kde, xfce, lxqt, lxde, cinnamon, mate, gnome];
        own.iter().chain(standalone).chain(desktops).find_map(|probe| probe(env))
    }

    pub fn expand_home(value: &str, home: &Path) -> PathBuf {
        match value.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => from_uri(value),
        }
    }

    fn read(path: PathBuf) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    // ----- GSettings desktops ---------------------------------------------

    fn gsetting(schema: &str, key: &str) -> Option<String> {
        let out = run("gsettings", &["get", schema, key])?;
        let value = out.trim().trim_matches('\'').to_owned();
        (!value.is_empty()).then_some(value)
    }

    /// GNOME and its relatives: `picture-uri` (`picture-uri-dark` in dark
    /// style), or the primary color when pictures are off.
    fn gsettings_desktop(schema: &str, uri_key: &str, dark_key: Option<&str>) -> Option<Wallpaper> {
        if gsetting(schema, "picture-options").as_deref() == Some("none") {
            return gsetting(schema, "primary-color").and_then(|c| parse_hex_color(&c)).map(Wallpaper::Color);
        }
        let dark = dark_key.filter(|_| gsetting("org.gnome.desktop.interface", "color-scheme").as_deref() == Some("prefer-dark"));
        let uri = dark.and_then(|k| gsetting(schema, k)).or_else(|| gsetting(schema, uri_key))?;
        let path = from_uri(&uri);
        if extension(&path) == "xml" {
            return gnome_slideshow(&read(path)?).and_then(|p| picture(&p));
        }
        picture(&path)
    }

    fn gnome(_: &Env) -> Option<Wallpaper> {
        gsettings_desktop("org.gnome.desktop.background", "picture-uri", Some("picture-uri-dark"))
    }

    fn cinnamon(_: &Env) -> Option<Wallpaper> {
        gsettings_desktop("org.cinnamon.desktop.background", "picture-uri", None)
    }

    fn mate(_: &Env) -> Option<Wallpaper> {
        gsettings_desktop("org.mate.background", "picture-filename", None)
    }

    /// The first picture of a GNOME slideshow (`<static><file>…</file>`).
    pub fn gnome_slideshow(xml: &str) -> Option<PathBuf> {
        let start = xml.find("<file>")? + "<file>".len();
        let end = start + xml[start..].find("</file>")?;
        Some(PathBuf::from(xml[start..end].trim()))
    }

    // ----- Desktops with config files --------------------------------------

    fn cosmic(env: &Env) -> Option<Wallpaper> {
        let state = env.state.join("cosmic/com.system76.CosmicBackground/v1/wallpapers");
        if let Some(found) = read(state).and_then(|s| ron_path(&s)).and_then(|p| picture(&p)) {
            return Some(found);
        }
        let config = env.config.join("cosmic/com.system76.CosmicBackground/v1");
        let all = read(config.join("all"))?;
        cosmic_source(&all, picture)
    }

    /// `source: Path("…")` or `source: Color(Single((r, g, b)))` (0–1 floats;
    /// a gradient gives its first color).
    pub fn cosmic_source(text: &str, resolve: impl Fn(&Path) -> Option<Wallpaper>) -> Option<Wallpaper> {
        let source = &text[text.find("source:")?..];
        if source["source:".len()..].trim_start().starts_with("Color") {
            let start = source.find("((")? + 2;
            let end = start + source[start..].find(')')?;
            let parts: Vec<f32> = source[start..end].split(',').map(|v| v.trim().parse()).collect::<Result<_, _>>().ok()?;
            let [r, g, b] = parts[..] else { return None };
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
            return Some(Wallpaper::Color((byte(r) << 16) | (byte(g) << 8) | byte(b)));
        }
        resolve(&ron_path(source)?)
    }

    /// The first `Path("…")` in a RON document.
    pub fn ron_path(text: &str) -> Option<PathBuf> {
        let start = text.find("Path(\"")? + "Path(\"".len();
        let end = start + text[start..].find('"')?;
        Some(PathBuf::from(&text[start..end]))
    }

    fn kde(env: &Env) -> Option<Wallpaper> {
        let config = read(env.config.join("plasma-org.kde.plasma.desktop-appletsrc"));
        if let Some(value) = config.as_deref().and_then(plasma_image)
            && let Some(found) = picture(&expand_home(&value, &env.home))
        {
            return Some(found);
        }
        // Plasma's own default, which it does not write down.
        ["/usr/share/wallpapers/Next", "/usr/share/wallpapers/Flow"].iter().find_map(|d| picture(Path::new(d)))
    }

    /// `Image=` of the first desktop with a picture wallpaper, or the first
    /// folder of a slideshow, from `plasma-org.kde.plasma.desktop-appletsrc`.
    pub fn plasma_image(ini: &str) -> Option<String> {
        let mut section = "";
        let mut slideshow = None;
        for line in ini.lines().map(str::trim) {
            if line.starts_with('[') {
                section = line;
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            if section.ends_with("[Wallpaper][org.kde.image][General]") && key == "Image" && !value.is_empty() {
                return Some(value.to_owned());
            }
            if section.ends_with("[Wallpaper][org.kde.slideshow][General]") && key == "SlidePaths" && slideshow.is_none() {
                slideshow = value.split(',').next().map(str::to_owned).filter(|v| !v.is_empty());
            }
        }
        slideshow
    }

    fn xfce(env: &Env) -> Option<Wallpaper> {
        let xml = read(env.config.join("xfce4/xfconf/xfce-perchannel-xml/xfce4-desktop.xml"))?;
        xfce_images(&xml).iter().find_map(|p| picture(p))
    }

    /// Every `last-image` value, in file order (screen 0, first monitor first).
    pub fn xfce_images(xml: &str) -> Vec<PathBuf> {
        xml.lines()
            .filter(|l| l.contains("name=\"last-image\""))
            .filter_map(|l| {
                let start = l.find("value=\"")? + "value=\"".len();
                let end = start + l[start..].find('"')?;
                Some(PathBuf::from(&l[start..end]))
            })
            .collect()
    }

    fn ini_value(text: &str, key: &str) -> Option<String> {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key && !v.trim().is_empty()).then(|| v.trim().to_owned())
        })
    }

    fn lxqt(env: &Env) -> Option<Wallpaper> {
        let text = read(env.config.join("pcmanfm-qt/lxqt/settings.conf"))?;
        picture(&expand_home(&ini_value(&text, "Wallpaper")?, &env.home))
    }

    fn lxde(env: &Env) -> Option<Wallpaper> {
        let text = read(env.config.join("pcmanfm/LXDE/desktop-items-0.conf"))?;
        picture(&expand_home(&ini_value(&text, "wallpaper")?, &env.home))
    }

    // ----- Stand-alone wallpaper programs ----------------------------------

    /// hyprpaper: ask it over IPC, else read its config (both syntaxes:
    /// `wallpaper = monitor,path` and a `wallpaper { path = … }` block).
    fn hyprpaper(env: &Env) -> Option<Wallpaper> {
        if let Some(found) = run("hyprctl", &["hyprpaper", "listactive"]).and_then(|out| first_existing_path(&out, &env.home)) {
            return Some(found);
        }
        let config = read(env.config.join("hypr/hyprpaper.conf"))?;
        hyprpaper_paths(&config).iter().find_map(|p| picture(&expand_home(p, &env.home)))
    }

    pub fn hyprpaper_paths(config: &str) -> Vec<String> {
        let mut paths = Vec::new();
        let mut preload = Vec::new();
        for line in config.lines().map(|l| l.split('#').next().unwrap_or_default().trim()) {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "wallpaper" => paths.push(value.split_once(',').map_or(value, |(_, p)| p).trim().to_owned()),
                "path" => paths.push(value.to_owned()),
                "preload" => preload.push(value.to_owned()),
                _ => {}
            }
        }
        paths.extend(preload);
        paths
    }

    /// swww, renamed awww in 2025: `query` names the image on each output.
    fn swww(env: &Env) -> Option<Wallpaper> {
        ["awww", "swww"].iter().find_map(|program| first_existing_path(&run(program, &["query"])?, &env.home))
    }

    fn wpaperd(env: &Env) -> Option<Wallpaper> {
        first_existing_path(&run("wpaperctl", &["all-wallpapers"])?, &env.home)
    }

    fn waypaper(env: &Env) -> Option<Wallpaper> {
        let text = read(env.config.join("waypaper/config.ini"))?;
        picture(&expand_home(&ini_value(&text, "wallpaper")?, &env.home))
    }

    fn feh(env: &Env) -> Option<Wallpaper> {
        quoted_paths(&read(env.home.join(".fehbg"))?).iter().find_map(|p| picture(p))
    }

    fn nitrogen(env: &Env) -> Option<Wallpaper> {
        let text = read(env.config.join("nitrogen/bg-saved.cfg"))?;
        picture(&expand_home(&ini_value(&text, "file")?, &env.home))
    }

    /// swaybg (what Sway and many Niri setups start), wbg and xwallpaper
    /// keep the picture on their command line.
    fn running_programs(env: &Env) -> Option<Wallpaper> {
        for entry in std::fs::read_dir("/proc").ok()?.flatten() {
            let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else { continue };
            let args: Vec<String> = cmdline.split(|b| *b == 0).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
            if let Some(found) = wallpaper_argument(&args).and_then(|p| picture(&expand_home(&p, &env.home))) {
                return Some(found);
            }
        }
        None
    }

    pub fn wallpaper_argument(args: &[String]) -> Option<String> {
        let program = Path::new(args.first()?).file_name()?.to_str()?;
        match program {
            "swaybg" => args.windows(2).find(|w| w[0] == "-i" || w[0] == "--image").map(|w| w[1].clone()),
            "wbg" | "xwallpaper" => args.iter().skip(1).rev().find(|a| !a.starts_with('-') && !a.is_empty()).cloned(),
            _ => None,
        }
    }

    /// The first absolute (or `~/`) path in a tool's output that is a picture.
    fn first_existing_path(output: &str, home: &Path) -> Option<Wallpaper> {
        output
            .split(|c: char| c.is_whitespace() || c == ',' || c == '\'' || c == '"')
            .filter(|t| t.starts_with('/') || t.starts_with("~/"))
            .find_map(|t| picture(&expand_home(t, home)).filter(|_| PICTURES.contains(&extension(Path::new(t)).as_str())))
    }

    /// Single-quoted arguments of a shell line (`feh --bg-fill '/a b.jpg'`).
    pub fn quoted_paths(script: &str) -> Vec<PathBuf> {
        script.split('\'').skip(1).step_by(2).filter(|s| s.starts_with('/')).map(PathBuf::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        linux::{Env, cosmic_source, gnome_slideshow, hyprpaper_paths, plasma_image, quoted_paths, ron_path, wallpaper_argument, xfce_images},
        windows::{parse_rgb_triplet, reg_value},
        *,
    };

    #[test]
    fn decodes_file_uris_and_colors() {
        assert_eq!(from_uri("file:///usr/share/a%20b.jpg"), PathBuf::from("/usr/share/a b.jpg"));
        assert_eq!(from_uri("/plain/path.png"), PathBuf::from("/plain/path.png"));
        assert_eq!(parse_hex_color("'#023c88'"), Some(0x023c88));
        assert_eq!(parse_hex_color("#fff"), Some(0xffffff));
        assert_eq!(parse_hex_color("#ffff00000000"), Some(0xff0000));
        assert_eq!(parse_rgb_triplet("58 110 165"), Some(0x3a6ea5));
        assert_eq!(parse_rgb_triplet("1 2"), None);
    }

    #[test]
    fn reads_windows_registry_output() {
        let out = "\r\nHKEY_CURRENT_USER\\Control Panel\\Desktop\r\n    WallPaper    REG_SZ    C:\\Users\\a\\Pictures\\b c.jpg\r\n\r\n";
        assert_eq!(reg_value(out, "WallPaper").as_deref(), Some("C:\\Users\\a\\Pictures\\b c.jpg"));
        let empty = "\r\nHKEY_CURRENT_USER\\Control Panel\\Desktop\r\n    WallPaper    REG_SZ    \r\n";
        assert_eq!(reg_value(empty, "WallPaper").as_deref(), Some(""));
        assert_eq!(reg_value(out, "WallPaperStyle"), None);
    }

    #[test]
    fn reads_cosmic_sources() {
        let path = "(\n    output: \"all\",\n    source: Path(\"/usr/share/backgrounds/cosmic/a.jpg\"),\n    filter_by_theme: true,\n)";
        let resolved = cosmic_source(path, |p| Some(Wallpaper::Image(p.to_owned())));
        assert_eq!(resolved, Some(Wallpaper::Image("/usr/share/backgrounds/cosmic/a.jpg".into())));
        let color = "(\n    output: \"all\",\n    source: Color(Single((0.2, 0.4, 1.0))),\n)";
        assert_eq!(cosmic_source(color, |_| None), Some(Wallpaper::Color(0x3366ff)));
        let state = "[\n    (\"DP-1\", Path(\"/home/u/Pictures/x.png\")),\n]";
        assert_eq!(ron_path(state), Some(PathBuf::from("/home/u/Pictures/x.png")));
    }

    #[test]
    fn reads_plasma_xfce_and_gnome_files() {
        let plasma = "[Containments][1]\nwallpaperplugin=org.kde.image\n\n\
            [Containments][1][Wallpaper][org.kde.image][General]\nImage=file:///home/u/w.png\nPreviewImage=x\n";
        assert_eq!(plasma_image(plasma).as_deref(), Some("file:///home/u/w.png"));
        let slideshow = "[Containments][7][Wallpaper][org.kde.slideshow][General]\nSlidePaths=/home/u/Walls/,/usr/share/wallpapers/\n";
        assert_eq!(plasma_image(slideshow).as_deref(), Some("/home/u/Walls/"));

        let xfce = r#"<property name="monitoreDP-1" type="empty">
            <property name="workspace0" type="empty">
              <property name="last-image" type="string" value="/usr/share/backgrounds/xfce/xfce-shapes.svg"/>"#;
        assert_eq!(xfce_images(xfce), [PathBuf::from("/usr/share/backgrounds/xfce/xfce-shapes.svg")]);

        let xml = "<background><static><duration>8.0</duration><file>/usr/share/backgrounds/a.jpg</file></static></background>";
        assert_eq!(gnome_slideshow(xml), Some(PathBuf::from("/usr/share/backgrounds/a.jpg")));
    }

    #[test]
    fn reads_wayland_wallpaper_programs() {
        let old = "preload = ~/w/a.png\nwallpaper = DP-1,~/w/a.png # main\n";
        assert_eq!(hyprpaper_paths(old), ["~/w/a.png", "~/w/a.png"]);
        let block = "wallpaper {\n    monitor = DP-1\n    path = /home/u/b.jpg\n}\n";
        assert_eq!(hyprpaper_paths(block), ["/home/u/b.jpg"]);

        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(wallpaper_argument(&args(&["/usr/bin/swaybg", "-o", "*", "-i", "/a.png", "-m", "fill"])).as_deref(), Some("/a.png"));
        assert_eq!(wallpaper_argument(&args(&["wbg", "-s", "/b.jpg"])).as_deref(), Some("/b.jpg"));
        assert_eq!(wallpaper_argument(&args(&["bash", "-i", "/a.png"])), None);

        assert_eq!(quoted_paths("#!/bin/sh\nfeh --no-fehbg --bg-fill '/home/u/a b.jpg' \n"), [PathBuf::from("/home/u/a b.jpg")]);
    }

    #[test]
    fn finds_pictures_in_kde_packages_and_folders() {
        let root = std::env::temp_dir().join(format!("sulfur-wallpaper-{}", std::process::id()));
        let images = root.join("Next/contents/images");
        std::fs::create_dir_all(&images).unwrap();
        for name in ["1920x1080.png", "5120x2880.png", "3840x2160.avif"] {
            std::fs::write(images.join(name), b"").unwrap();
        }
        assert_eq!(picture(&root.join("Next")), Some(Wallpaper::Image(images.join("5120x2880.png"))));

        let state = root.join("state/cosmic/com.system76.CosmicBackground/v1");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("wallpapers"), format!("[(\"DP-1\", Path(\"{}\"))]", images.join("1920x1080.png").display())).unwrap();
        let env = Env { home: root.clone(), config: root.join("config"), state: root.join("state"), desktops: vec!["cosmic".into()] };
        assert_eq!(linux::detect(&env), Some(Wallpaper::Image(images.join("1920x1080.png"))));

        let pic = images.join("1920x1080.png");
        assert_eq!(decodable(&pic, &root), Some(pic));
        std::fs::remove_dir_all(root).unwrap();
    }
}
