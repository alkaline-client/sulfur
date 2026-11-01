//! Alkaline, a Minecraft 1.8.9 PvP client with its own Vulkan renderer.
//!
//! Alkaline is not a version JSON over the vanilla jar: it is a program of
//! its own (a Gradle `installDist` folder with `bin/game-client`) that reads
//! the vanilla 1.8.9 jar only for its assets. Sulfur finds the client (or
//! its source checkout, which it builds) or downloads a release, gives it
//! Java 25 and the jar, and starts it like any other game.
//!
//! Releases come from Alkaline's GitHub releases, or from a feed (see
//! [`Feed`]) such as one on Alkaline's own server. A release carries one zip
//! per platform, `alkaline-<version>-<os>-<arch>.zip` (`linux-x64`,
//! `windows-x64`, `macos-arm64`, ...), laid out as Gradle's `distZip`: one
//! folder holding `bin/` and `lib/`. Sulfur installs it into
//! [`release_dir`], where [`locate`] looks.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::SystemTime,
};

use serde::Deserialize;
use sulfur_download::{CancellationToken, Checksum, DownloadTask};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
    McError, Result,
    account::{Account, AccountKind},
    install::{InstallMonitor, InstallStage, Installer},
    launch::LaunchCommand,
    platform::{Arch, Os, Platform},
};

/// The id Alkaline has among installed versions.
pub const VERSION_ID: &str = "Alkaline";
/// The vanilla version whose jar Alkaline reads its assets from.
pub const GAME_VERSION: &str = "1.8.9";
/// The Java Alkaline is built for and runs on.
pub const JAVA_MAJOR: u32 = 25;

/// The Gradle task that builds the client's `installDist` folder.
const BUILD_TASK: &str = ":apps:game-client:installDist";

/// Where the client is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Client {
    /// A built client: the folder holding `bin/` and `lib/`.
    Installed(PathBuf),
    /// Alkaline's `legacy` source checkout, built with its Gradle wrapper.
    Source(PathBuf),
}

impl Client {
    /// The folder the client runs from (built from source, it may not exist yet).
    pub fn home(&self) -> PathBuf {
        match self {
            Self::Installed(home) => home.clone(),
            Self::Source(root) => install_dir(root),
        }
    }

    /// Whether the source checkout has no build, or one older than its sources.
    pub fn needs_build(&self) -> bool {
        let Self::Source(root) = self else { return false };
        // Gradle leaves jars it did not change alone, so a build also leaves a stamp.
        let Some(built) = newest_file(&install_dir(root).join("lib"), 1).max(modified(&build_stamp(root))) else {
            return true;
        };
        let sources = ["apps", "modules"]
            .iter()
            .filter_map(|dir| newest_file(&root.join(dir), 8))
            .chain(["build.gradle.kts", "settings.gradle.kts", "gradle/libs.versions.toml"].iter().filter_map(|f| modified(&root.join(f))))
            .max();
        sources.is_some_and(|sources| sources > built)
    }
}

/// Finds the client: `explicit` (a client folder or a source checkout), then
/// `ALKALINE_CLIENT_HOME`, then `~/.alkaline/legacy/client`, then a source
/// checkout in `~/Projects/Alkaline/legacy`.
pub fn locate(explicit: Option<&Path>) -> Option<Client> {
    if let Some(dir) = explicit {
        return classify(dir);
    }
    let home = home_dir();
    std::env::var_os("ALKALINE_CLIENT_HOME")
        .map(PathBuf::from)
        .into_iter()
        .chain(home.iter().map(|h| h.join(".alkaline/legacy/client")))
        .chain(home.iter().map(|h| h.join("Projects/Alkaline/legacy")))
        .find_map(|dir| classify(&dir))
}

/// What `dir` holds: a built client, a source checkout, or neither.
fn classify(dir: &Path) -> Option<Client> {
    if start_script(dir).is_file() {
        Some(Client::Installed(dir.to_owned()))
    } else if dir.join("apps/game-client").is_dir() && wrapper(dir).is_file() {
        Some(Client::Source(dir.to_owned()))
    } else {
        None
    }
}

/// Alkaline's own data folder (settings, profiles, plugins, music).
pub fn data_dir() -> PathBuf {
    home_dir().unwrap_or_default().join(".alkaline/legacy")
}

/// Where a downloaded release lives; [`locate`] looks here.
pub fn release_dir() -> PathBuf {
    data_dir().join("client")
}

/// The version of the release in [`release_dir`], if Sulfur installed one.
pub fn installed_release() -> Option<String> {
    let version = std::fs::read_to_string(release_dir().join(RELEASE_STAMP)).ok()?;
    Some(version.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Records which release [`release_dir`] holds.
const RELEASE_STAMP: &str = ".sulfur-release";

/// Alkaline's releases on GitHub, newest first.
pub const GITHUB_RELEASES: &str = "https://api.github.com/repos/alkaline-client/alkaline/releases?per_page=20";

/// The same releases as an Atom feed, which has no rate limit (the API allows
/// 60 requests an hour per address without signing in, which shared networks
/// run out of), but no file list.
const GITHUB_ATOM: &str = "https://github.com/alkaline-client/alkaline/releases.atom";

/// Where GitHub serves a release's files.
const GITHUB_DOWNLOADS: &str = "https://github.com/alkaline-client/alkaline/releases/download";

/// A release of Alkaline, with its download for this platform if it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// Release notes, in Markdown.
    pub notes: String,
    /// `YYYY-MM-DD`, or empty.
    pub published: String,
    /// A page about the release, or empty.
    pub page_url: String,
    pub download: Option<Download>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Download {
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// The release feed on a server of Alkaline's own:
///
/// ```json
/// { "version": "0.2.0", "published": "2026-11-02", "notes": "…", "page": "https://…",
///   "downloads": { "linux-x64": { "url": "https://…/alkaline-0.2.0-linux-x64.zip", "sha256": "…", "size": 123 } } }
/// ```
#[derive(Debug, Deserialize)]
pub struct Feed {
    pub version: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub downloads: std::collections::BTreeMap<String, Download>,
}

/// How release files name a platform: `linux-x64`, `windows-arm64`, `macos-arm64`, ...
pub fn platform_key(platform: Platform) -> String {
    let os = match platform.os {
        Os::Windows => "windows",
        Os::MacOs => "macos",
        Os::Linux => "linux",
    };
    let arch = match platform.arch {
        Arch::Arm64 => "arm64",
        Arch::X86 => "x86",
        _ => "x64",
    };
    format!("{os}-{arch}")
}

/// The newest release: from `feed` (a [`Feed`] address) if given, else from
/// GitHub, where it is the newest one with a download for `platform`, or
/// the newest at all if none has one. `None` if there is no release yet.
pub async fn latest_release(
    installer: &Installer,
    feed: Option<&str>,
    platform: Platform,
    cancel: &CancellationToken,
) -> Result<Option<Release>> {
    let downloader = installer.downloader();
    let key = platform_key(platform);
    match feed {
        Some(url) => {
            let bytes = downloader.fetch(url, cancel).await?;
            let atom = r#"<feed><title>Release notes from alkaline</title>
            <entry><id>tag:github.com,2008:Repository/1/v0.2.0</id><updated>2026-11-03T10:00:00Z</updated>
            <link rel="alternate" type="text/html" href="https://github.com/alkaline-client/alkaline/releases/tag/v0.2.0"/>
            <title>Alkaline 0.2.0</title></entry>
            <entry><link rel="alternate" type="text/html" href="https://github.com/alkaline-client/alkaline/releases/tag/v0.1.0"/></entry></feed>"#;
        let release = parse_github_atom(atom, "linux-x64").unwrap();
        assert_eq!((release.version.as_str(), release.published.as_str()), ("0.2.0", "2026-11-03"));
        assert_eq!(
            release.download.unwrap().url,
            "https://github.com/alkaline-client/alkaline/releases/download/v0.2.0/alkaline-0.2.0-linux-x64.zip"
        );
        assert_eq!(parse_github_atom("<feed></feed>", "linux-x64"), None);

        let feed: Feed = serde_json::from_slice(&bytes).map_err(|source| McError::Json { path: url.into(), source })?;
            Ok(Some(release_from_feed(feed, &key)))
        }
        None => {
            let headers = [("Accept", "application/vnd.github+json")];
            match downloader.fetch_with_headers(GITHUB_RELEASES, &headers, cancel).await {
                Ok(bytes) => parse_github(&bytes, &key),
                Err(e) if !cancel.is_cancelled() => {
                    tracing::info!(error = %e, "GitHub's API refused, reading the releases feed");
                    match downloader.fetch(GITHUB_ATOM, cancel).await {
                        Ok(bytes) => Ok(parse_github_atom(&String::from_utf8_lossy(&bytes), &key)),
                        // The repository is not public yet, so nothing is released.
                        Err(sulfur_download::DownloadError::Status { status: 404, .. }) => Ok(None),
                        Err(e) => Err(e.into()),
                    }
                }
                Err(e) => Err(e.into()),
            }
        }
    }
}

fn release_from_feed(mut feed: Feed, key: &str) -> Release {
    Release {
        download: feed.downloads.remove(key),
        version: feed.version,
        notes: feed.notes,
        published: feed.published.chars().take(10).collect(),
        page_url: feed.page,
    }
}

fn parse_github(bytes: &[u8], key: &str) -> Result<Option<Release>> {
    #[derive(Deserialize)]
    struct GithubRelease {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        published_at: Option<String>,
        #[serde(default)]
        html_url: String,
        #[serde(default)]
        assets: Vec<GithubAsset>,
    }
    #[derive(Deserialize)]
    struct GithubAsset {
        name: String,
        browser_download_url: String,
        #[serde(default)]
        size: Option<u64>,
        /// `sha256:<hex>` on newer releases.
        #[serde(default)]
        digest: Option<String>,
    }
    let releases: Vec<GithubRelease> =
        serde_json::from_slice(bytes).map_err(|source| McError::Json { path: GITHUB_RELEASES.into(), source })?;
    let suffix = format!("-{key}.zip");
    let releases: Vec<_> = releases.into_iter().filter(|r| !r.draft).collect();
    let has_download = |r: &GithubRelease| r.assets.iter().any(|a| a.name.starts_with("alkaline-") && a.name.ends_with(&suffix));
    let Some(release) = releases.iter().position(has_download).or((!releases.is_empty()).then_some(0)).map(|i| &releases[i]) else {
        return Ok(None);
    };
    let download = release.assets.iter().find(|a| a.name.starts_with("alkaline-") && a.name.ends_with(&suffix)).map(|a| Download {
        url: a.browser_download_url.clone(),
        sha256: a.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_owned),
        size: a.size,
    });
    Ok(Some(Release {
        version: release.tag_name.trim_start_matches('v').to_owned(),
        notes: release.body.clone().unwrap_or_default(),
        published: release.published_at.as_deref().unwrap_or_default().chars().take(10).collect(),
        page_url: release.html_url.clone(),
        download,
    }))
}

/// The newest release in GitHub's Atom feed. The feed lists no files, so the
/// download is where the naming rule puts it (`<tag>/alkaline-<version>-<platform>.zip`).
fn parse_github_atom(xml: &str, key: &str) -> Option<Release> {
    let entry = xml.split("<entry>").nth(1)?.split("</entry>").next()?;
    let tag = |name: &str| -> Option<&str> {
        let start = entry.find(&format!("<{name}"))?;
        let after = &entry[start..];
        let open = after.find('>')? + 1;
        let close = after.find(&format!("</{name}>"))?;
        Some(after[open..close].trim())
    };
    let page_url = entry
        .split("href=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned();
    let tag_name = page_url.rsplit("/tag/").next().filter(|t| !t.is_empty() && page_url.contains("/tag/"))?.to_owned();
    let version = tag_name.trim_start_matches('v').to_owned();
    Some(Release {
        download: Some(Download {
            url: format!("{GITHUB_DOWNLOADS}/{tag_name}/alkaline-{version}-{key}.zip"),
            sha256: None,
            size: None,
        }),
        notes: String::new(),
        published: tag("updated").unwrap_or_default().chars().take(10).collect(),
        page_url,
        version,
    })
}

/// Downloads `release` and puts it into [`release_dir`] in place of the one
/// there; a failed download or unpack leaves the old one working.
pub async fn install_release(installer: &Installer, release: &Release, monitor: &InstallMonitor, cancel: &CancellationToken) -> Result<()> {
    let download = release.download.as_ref().ok_or_else(|| {
        McError::Invalid(format!("Alkaline {} has no download for {}", release.version, platform_key(installer.platform())))
    })?;
    let target = release_dir();
    let archive = data_dir().join(format!("alkaline-{}.zip", release.version));
    let task = DownloadTask::new(&download.url, &archive)
        .size(download.size)
        .checksum(download.sha256.as_deref().and_then(Checksum::sha256_hex));
    monitor.set_stage(InstallStage::Downloading);
    installer.batch(vec![task], &monitor.progress, cancel).await?;

    monitor.set_stage(InstallStage::Extracting);
    let (archive_path, target_dir) = (archive.clone(), target.clone());
    tokio::task::spawn_blocking(move || crate::runtime::unpack(&archive_path, &target_dir, true))
        .await
        .map_err(|e| McError::Invalid(format!("unpack worker failed: {e}")))??;
    let _ = std::fs::remove_file(&archive);
    let script = start_script(&target);
    if !script.is_file() {
        return Err(McError::Invalid(format!("the Alkaline {} download has no {}", release.version, script.display())));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).map_err(|e| McError::io(&script, e))?;
    }
    let stamp = target.join(RELEASE_STAMP);
    std::fs::write(&stamp, &release.version).map_err(|e| McError::io(&stamp, e))
}

/// Builds the client from `root` with its Gradle wrapper, passing each output
/// line to `line`. `java_home` is used when it is a JDK; otherwise Gradle
/// finds one itself.
pub async fn build(root: &Path, java_home: Option<&Path>, mut line: impl FnMut(&str)) -> Result<()> {
    let mut command = if cfg!(windows) {
        let mut command = tokio::process::Command::new("cmd");
        command.arg("/C").arg(wrapper(root));
        command
    } else {
        tokio::process::Command::new(wrapper(root))
    };
    command
        .args([BUILD_TASK, "--console=plain"])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(home) = java_home.filter(|h| h.join("bin").join(exe("javac")).is_file()) {
        command.env("JAVA_HOME", home);
    }
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = command.spawn().map_err(McError::Spawn)?;
    let mut stdout = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut stderr = BufReader::new(child.stderr.take().expect("piped")).lines();
    // The last lines say why a build failed.
    let mut tail = std::collections::VecDeque::new();
    let (mut out_open, mut err_open) = (true, true);
    while out_open || err_open {
        let next = tokio::select! {
            l = stdout.next_line(), if out_open => l.ok().flatten().or_else(|| { out_open = false; None }),
            l = stderr.next_line(), if err_open => l.ok().flatten().or_else(|| { err_open = false; None }),
        };
        if let Some(text) = next {
            line(&text);
            if tail.len() == 12 {
                tail.pop_front();
            }
            tail.push_back(text);
        }
    }
    let status = child.wait().await.map_err(|e| McError::io(root, e))?;
    if status.success() {
        let stamp = build_stamp(root);
        std::fs::write(&stamp, b"").map_err(|e| McError::io(&stamp, e))?;
        return Ok(());
    }
    let reason = tail
        .iter()
        .rev()
        .find(|l| l.contains("What went wrong") || l.starts_with("e: ") || l.contains("error:"))
        .or(tail.back())
        .cloned()
        .unwrap_or_default();
    Err(McError::Invalid(format!("building Alkaline failed ({status}): {reason}")))
}

/// What a launch needs besides the client.
pub struct LaunchOptions {
    pub java: PathBuf,
    /// The vanilla 1.8.9 client jar, for Alkaline's assets.
    pub minecraft_jar: Option<PathBuf>,
    pub max_memory_mib: u32,
    pub min_memory_mib: u32,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    /// Join this server on start.
    pub server: Option<String>,
    pub wrapper: Vec<String>,
    pub env: Vec<(String, String)>,
    /// The player to sign in as: Alkaline reads it from its environment
    /// (an offline account goes without a token).
    pub account: Option<Account>,
}

/// The command that starts the client in `home` through its start script,
/// which keeps the classpath order Gradle wrote.
pub fn command(home: &Path, opts: &LaunchOptions) -> LaunchCommand {
    let script = start_script(home);
    let mut program = vec![];
    if cfg!(windows) {
        program.extend(["cmd".to_owned(), "/C".to_owned()]);
    }
    let mut parts: Vec<String> = opts.wrapper.iter().cloned().chain(program).collect();
    parts.push(script.to_string_lossy().into_owned());
    parts.extend(opts.game_args.iter().cloned());
    if let Some(server) = &opts.server {
        parts.extend(["--join".to_owned(), server.clone()]);
    }

    let mut java_opts = vec![format!("-Xmx{}m", opts.max_memory_mib)];
    if opts.min_memory_mib > 0 {
        java_opts.push(format!("-Xms{}m", opts.min_memory_mib.min(opts.max_memory_mib)));
    }
    java_opts.extend(opts.jvm_args.iter().cloned());
    let java_home = opts.java.parent().and_then(Path::parent).unwrap_or(&opts.java);
    let mut env = vec![
        ("JAVA_HOME".to_owned(), java_home.to_string_lossy().into_owned()),
        // The start script splits JAVA_OPTS on spaces, so arguments with spaces are quoted.
        ("JAVA_OPTS".to_owned(), java_opts.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")),
    ];
    if let Some(jar) = &opts.minecraft_jar {
        env.push(("ALKALINE_MINECRAFT_JAR".to_owned(), jar.to_string_lossy().into_owned()));
    }
    if let Some(account) = &opts.account {
        env.push(("ALKALINE_PLAYER_NAME".to_owned(), account.name.clone()));
        env.push(("ALKALINE_PLAYER_UUID".to_owned(), account.uuid.clone()));
        if account.kind == AccountKind::Microsoft {
            // In the environment rather than the arguments, which other users can read.
            env.push(("ALKALINE_ACCESS_TOKEN".to_owned(), account.access_token.clone()));
        }
    }
    env.extend(opts.env.iter().cloned());

    let program = PathBuf::from(parts.remove(0));
    LaunchCommand { program, args: parts, working_dir: data_dir(), env }
}

fn quote(arg: &str) -> String {
    if arg.contains(char::is_whitespace) { format!("\"{arg}\"") } else { arg.to_owned() }
}

fn install_dir(root: &Path) -> PathBuf {
    root.join("apps/game-client/build/install/game-client")
}

fn build_stamp(root: &Path) -> PathBuf {
    install_dir(root).join(".sulfur-built")
}

fn start_script(home: &Path) -> PathBuf {
    home.join("bin").join(if cfg!(windows) { "game-client.bat" } else { "game-client" })
}

fn wrapper(root: &Path) -> PathBuf {
    root.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" })
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn modified(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

/// The newest modification time of a file under `dir`, `depth` levels deep at
/// most, skipping build output, tests and hidden folders.
fn newest_file(dir: &Path, depth: u32) -> Option<SystemTime> {
    let mut newest = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let Ok(kind) = entry.file_type() else { continue };
        let time = if kind.is_dir() {
            if depth == 0 || name == "build" || name == "test" || name.to_string_lossy().starts_with('.') {
                continue;
            }
            newest_file(&entry.path(), depth - 1)
        } else {
            entry.metadata().ok().and_then(|m| m.modified().ok())
        };
        newest = newest.max(time);
    }
    newest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sulfur-alkaline-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn classifies_client_and_source() {
        let installed = temp("installed");
        std::fs::create_dir_all(installed.join("bin")).unwrap();
        std::fs::write(start_script(&installed), "").unwrap();
        assert_eq!(locate(Some(&installed)), Some(Client::Installed(installed.clone())));

        let source = temp("source");
        std::fs::create_dir_all(source.join("apps/game-client/src")).unwrap();
        std::fs::write(wrapper(&source), "").unwrap();
        assert_eq!(locate(Some(&source)), Some(Client::Source(source.clone())));
        assert!(Client::Source(source.clone()).needs_build());

        // Built after the last source change.
        std::fs::write(source.join("apps/game-client/src/Main.java"), "").unwrap();
        std::fs::create_dir_all(install_dir(&source).join("lib")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(install_dir(&source).join("lib/game-client.jar"), "").unwrap();
        assert!(!Client::Source(source.clone()).needs_build());

        assert_eq!(locate(Some(&source.join("missing"))), None);
    }

    #[test]
    fn reads_github_releases_and_feeds() {
        let github = br#"[
            {"tag_name":"v0.3.0","draft":true,"assets":[{"name":"alkaline-0.3.0-linux-x64.zip","browser_download_url":"https://d/3"}]},
            {"tag_name":"v0.2.0","body":"Windows only","published_at":"2026-11-03T10:00:00Z","html_url":"https://r/2",
             "assets":[{"name":"alkaline-0.2.0-windows-x64.zip","browser_download_url":"https://d/2w"}]},
            {"tag_name":"v0.1.0","body":"First","published_at":"2026-11-01T10:00:00Z","html_url":"https://r/1",
             "assets":[{"name":"alkaline-0.1.0-linux-x64.zip","browser_download_url":"https://d/1","size":9,"digest":"sha256:ab"}]}
        ]"#;
        // The newest release with a download for this platform, drafts left out.
        let release = parse_github(github, "linux-x64").unwrap().unwrap();
        assert_eq!(release.version, "0.1.0");
        assert_eq!(release.published, "2026-11-01");
        assert_eq!(release.download, Some(Download { url: "https://d/1".into(), sha256: Some("ab".into()), size: Some(9) }));
        // Without one, the newest release, so the UI can say there is none for this system.
        let release = parse_github(github, "macos-arm64").unwrap().unwrap();
        assert_eq!((release.version.as_str(), release.download), ("0.2.0", None));
        assert_eq!(parse_github(b"[]", "linux-x64").unwrap(), None);

        let feed: Feed = serde_json::from_slice(
            br#"{"version":"0.4.0","published":"2026-12-01","downloads":{"linux-x64":{"url":"https://s/4.zip","sha256":"cd"}}}"#,
        )
        .unwrap();
        let release = release_from_feed(feed, "linux-x64");
        assert_eq!(release.version, "0.4.0");
        assert_eq!(release.download.unwrap().url, "https://s/4.zip");
        assert_eq!(platform_key(Platform { os: Os::Windows, arch: Arch::Arm64 }), "windows-arm64");
    }

    #[test]
    fn command_passes_java_jar_and_server() {
        let opts = LaunchOptions {
            java: PathBuf::from("/jdk-25/bin/java"),
            minecraft_jar: Some(PathBuf::from("/mc/versions/1.8.9/1.8.9.jar")),
            max_memory_mib: 4096,
            min_memory_mib: 0,
            jvm_args: vec!["-Dname=a b".into()],
            game_args: vec!["--stats".into()],
            server: Some("localhost:25565".into()),
            wrapper: vec!["gamemoderun".into()],
            env: vec![("A".into(), "1".into())],
            account: Some(Account::microsoft("Steve", "abc", "token", "x")),
        };
        let cmd = command(Path::new("/client"), &opts);
        if cfg!(windows) {
            return;
        }
        assert_eq!(cmd.program, PathBuf::from("gamemoderun"));
        assert_eq!(cmd.args, ["/client/bin/game-client", "--stats", "--join", "localhost:25565"]);
        let env = |k: &str| cmd.env.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        assert_eq!(env("JAVA_HOME"), Some("/jdk-25"));
        assert_eq!(env("JAVA_OPTS"), Some("-Xmx4096m \"-Dname=a b\""));
        assert_eq!(env("ALKALINE_MINECRAFT_JAR"), Some("/mc/versions/1.8.9/1.8.9.jar"));
        assert_eq!(env("A"), Some("1"));
        assert_eq!(env("ALKALINE_PLAYER_NAME"), Some("Steve"));
        assert_eq!(env("ALKALINE_ACCESS_TOKEN"), Some("token"));
        assert!(!cmd.args.iter().any(|a| a.contains("token")));
    }
}
