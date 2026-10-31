//! Installs a Minecraft version from the command line and reports throughput.
//!
//! ```text
//! cargo run --release -p sulfur-minecraft --example install -- [VERSION] [GAME_DIR] \
//!     [--mirror|--official] [--connections N] [--clients N] [--http2] [--reuse OTHER_GAME_DIR] [--no-reuse] [--launch NAME]
//!     [--loader fabric|legacyfabric|ornithe|quilt|forge|neoforge|cleanroom|rift|optifine[:VERSION]]
//! ```

use std::{sync::Arc, time::{Duration, Instant}};

use sulfur_core::DownloadSource;
use sulfur_download::{CancellationToken, DownloadConfig, Downloader, SpeedMeter};
use sulfur_minecraft::{
    account::Account,
    install::{InstallMonitor, Installer},
    java,
    launch::{self, LaunchOptions},
    loader::{self, LoaderKind, LoaderRequest},
    layout::GameDir,
    platform::Platform,
    process::GameProcess,
    sources,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into())).init();
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut take_flag = |flag: &str| args.iter().position(|a| a == flag).map(|i| args.remove(i)).is_some();
    let source = if take_flag("--mirror") {
        DownloadSource::Mirror
    } else if take_flag("--official") {
        DownloadSource::Official
    } else {
        DownloadSource::Auto
    };
    // Benchmarks want every file from the network, not from the official launcher's folder.
    let no_reuse = take_flag("--no-reuse");
    let http2 = take_flag("--http2");
    let mut config = DownloadConfig { http1_only: !http2, ..DownloadConfig::default() };
    if let Some(i) = args.iter().position(|a| a == "--connections") {
        config.max_connections = args[i + 1].parse()?;
        args.drain(i..=i + 1);
    }
    if let Some(i) = args.iter().position(|a| a == "--clients") {
        config.http_clients = args[i + 1].parse()?;
        args.drain(i..=i + 1);
    }
    let reuse = args.iter().position(|a| a == "--reuse").map(|i| {
        let dir = GameDir::new(args[i + 1].clone());
        args.drain(i..=i + 1);
        dir
    });
    let loader_arg = args.iter().position(|a| a == "--loader").map(|i| {
        let value = args[i + 1].clone();
        args.drain(i..=i + 1);
        value
    });
    let launch_as = args.iter().position(|a| a == "--launch").map(|i| {
        let name = args[i + 1].clone();
        args.drain(i..=i + 1);
        name
    });

    let dir = GameDir::new(args.get(1).cloned().unwrap_or_else(|| "minecraft".into()));
    let downloader = Downloader::new(config, sources::mirror_set(source))?;
    let installer = Installer::new(downloader, dir.clone(), Platform::current())
        .reuse_from(sulfur_minecraft::layout::official_game_dir().into_iter().filter(|_| !no_reuse).chain(reuse));
    let cancel = CancellationToken::new();

    let manifest = installer.fetch_manifest(&cancel).await?;
    let id = args.first().cloned().unwrap_or_else(|| manifest.latest.release.clone());
    // Installed versions that are not in the manifest (mod loaders) are launched as they are.
    let entry = match manifest.find(&id) {
        Some(entry) => Some(entry.clone()),
        None if dir.version_json(&id).is_file() => None,
        None => return Err(format!("unknown version {id}").into()),
    };

    let monitor = Arc::new(InstallMonitor::default());
    let started = Instant::now();
    let _ = &monitor;
    let reporter = {
        let monitor = monitor.clone();
        tokio::spawn(async move {
            let mut speed = SpeedMeter::default();
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let s = monitor.progress.snapshot();
                eprintln!(
                    "{:?} {:>5.1}%  files {}/{} (skipped {}, reused {})  {:.1}/{:.1} MiB  {:.2} MiB/s",
                    monitor.stage(),
                    s.fraction() * 100.0,
                    s.files_done,
                    s.files_total,
                    s.files_skipped,
                    s.files_reused,
                    s.bytes_done as f64 / 1048576.0,
                    s.bytes_total as f64 / 1048576.0,
                    speed.sample(s.bytes_done) / 1048576.0,
                );
            }
        })
    };
    let result = match &entry {
        Some(entry) => installer.install(entry, &monitor, &cancel).await.map(drop),
        None => Ok(()),
    };
    reporter.abort();
    let s = monitor.progress.snapshot();
    let secs = started.elapsed().as_secs_f64();
    result?;
    println!(
        "installed {id}: {} files ({} skipped, {} reused), {:.1} MiB in {secs:.1}s, avg {:.2} MiB/s",
        s.files_total,
        s.files_skipped,
        s.files_reused,
        s.bytes_done as f64 / 1048576.0,
        (s.bytes_done as f64 / 1048576.0) / secs
    );

    let mut id = id;
    if let Some(arg) = loader_arg {
        let (kind, version) = arg.split_once(':').map_or((arg.as_str(), None), |(k, v)| (k, Some(v.to_owned())));
        let kind = match kind {
            "fabric" => LoaderKind::Fabric,
            "forge" => LoaderKind::Forge,
            "neoforge" => LoaderKind::NeoForge,
            "optifine" => LoaderKind::OptiFine,
            "legacyfabric" => LoaderKind::LegacyFabric,
            "ornithe" => LoaderKind::Ornithe,
            "quilt" => LoaderKind::Quilt,
            "rift" => LoaderKind::Rift,
            "cleanroom" => LoaderKind::Cleanroom,
            other => return Err(format!("unknown loader {other}").into()),
        };
        let version = match version {
            Some(v) => v,
            None => {
                let all = loader::versions(installer.downloader(), kind, &id, &cancel).await?;
                let pick = all.iter().find(|v| v.stable).or(all.first()).ok_or("no loader versions")?;
                pick.version.clone()
            }
        };
        let base = launch::load_version(&dir, &id)?;
        let java = find_java(&installer, base.java_major(), &cancel).await?;
        let request = LoaderRequest {
            kind,
            game_version: id.clone(),
            name: format!("{id}-{kind:?}-{version}").to_lowercase(),
            loader_version: version,
            installer_url: None,
        };
        let work = std::env::temp_dir().join("sulfur-loader-example");
        let started = Instant::now();
        installer.install_loader(&request, &java.path, &work, &monitor, &cancel).await?;
        println!("installed {} in {:.1}s", request.name, started.elapsed().as_secs_f64());
        id = request.name;
    }

    if let Some(name) = launch_as {
        let version = launch::load_version(&dir, &id)?;
        let java = find_java(&installer, version.java_major(), &cancel).await?;
        let opts = LaunchOptions {
            java: java.path.clone(),
            run_dir: dir.run_dir(&id, true),
            account: Account::offline(&name),
            max_memory_mib: 2048,
            resolution: None,
            fullscreen: false,
            extra_jvm_args: vec![],
            log4j_fix: true,
            launcher_name: "Sulfur".into(),
            launcher_version: env!("CARGO_PKG_VERSION").into(),
        };
        launch::prepare_jar(&dir, &version)?;
        sulfur_minecraft::install::ensure_natives(&dir, &version, Platform::current())?;
        let cmd = launch::build(&dir, &version, &opts, Platform::current())?;
        println!("launching with {} (Java {})", java.path.display(), java.version);
        let game = GameProcess::spawn(&cmd)?;
        let mut next = 0;
        while game.is_running() {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let (n, lines) = game.logs_since(next);
            next = n;
            lines.iter().for_each(|l| println!("[game] {l}"));
        }
        let (_, lines) = game.logs_since(next);
        lines.iter().for_each(|l| println!("[game] {l}"));
        println!("game exited with {:?}", game.exit_code());
    }
    Ok(())
}

/// A Java that suits `required`, downloading one into `<game dir>/java` when
/// none installed does (like the launcher with automatic Java downloads on).
async fn find_java(
    installer: &Installer,
    required: u32,
    cancel: &CancellationToken,
) -> Result<java::JavaInstall, Box<dyn std::error::Error>> {
    let root = installer.dir().root().join("java");
    let javas = java::discover_in(sulfur_minecraft::runtime::executables(&root)).await;
    if let Some(java) = java::pick_compatible(&javas, required, Platform::current()) {
        return Ok(java.clone());
    }
    println!("downloading Java {required}");
    let monitor = InstallMonitor::default();
    Ok(sulfur_minecraft::runtime::install(installer, &root, required, &monitor, cancel).await?)
}
