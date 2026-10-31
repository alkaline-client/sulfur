//! Building the `java` command line for a version.

use std::{collections::HashMap, path::PathBuf};

use crate::{
    McError, Result,
    account::Account,
    assets::AssetIndex,
    layout::GameDir,
    library,
    platform::{Os, Platform},
    rules::{self, RuleContext},
    version::{Argument, VersionJson},
};

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub java: PathBuf,
    /// Working directory for saves, mods and options; see [`GameDir::run_dir`].
    pub run_dir: PathBuf,
    pub account: Account,
    pub max_memory_mib: u32,
    pub resolution: Option<(u32, u32)>,
    pub fullscreen: bool,
    pub extra_jvm_args: Vec<String>,
    /// Apply the Log4Shell fix to versions with a vulnerable Log4j.
    pub log4j_fix: bool,
    pub launcher_name: String,
    pub launcher_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

/// Loads `id` and its `inheritsFrom` chain into one resolved version.
pub fn load_version(dir: &GameDir, id: &str) -> Result<VersionJson> {
    let mut version = read_installed(dir, id)?;
    let mut depth = 0;
    while let Some(parent) = version.inherits_from.clone() {
        depth += 1;
        if depth > 8 {
            return Err(McError::Invalid(format!("inheritance chain of {id} is too deep")));
        }
        version = version.inherit(read_installed(dir, &parent)?);
    }
    Ok(version)
}

fn read_installed(dir: &GameDir, id: &str) -> Result<VersionJson> {
    let path = dir.version_json(id);
    if !path.is_file() {
        return Err(McError::NotInstalled(id.to_owned()));
    }
    VersionJson::read(&path)
}

/// The jar a version runs from: `versions/<id>/<id>.jar`. Like the official
/// launcher, versions that inherit their jar (mod loaders) get a link to the
/// parent's jar under their own name; Forge and NeoForge rely on that name to
/// keep the unpatched game off their module path (`-DignoreList=…${version_name}.jar`).
pub fn prepare_jar(dir: &GameDir, version: &VersionJson) -> Result<()> {
    let own = dir.version_jar(&version.id);
    if version.jar_id() == version.id || own.is_file() {
        return Ok(());
    }
    let parent = dir.version_jar(version.jar_id());
    if !parent.is_file() {
        return Err(McError::NotInstalled(version.jar_id().to_owned()));
    }
    std::fs::hard_link(&parent, &own)
        .or_else(|_| std::fs::copy(&parent, &own).map(drop))
        .map_err(|e| McError::io(&own, e))
}

/// The jar on the classpath: the version's own copy if [`prepare_jar`] made one.
fn launch_jar(dir: &GameDir, version: &VersionJson) -> PathBuf {
    let own = dir.version_jar(&version.id);
    if own.is_file() { own } else { dir.version_jar(version.jar_id()) }
}

pub fn build(dir: &GameDir, version: &VersionJson, opts: &LaunchOptions, platform: Platform) -> Result<LaunchCommand> {
    let mut ctx = RuleContext::new(platform);
    if opts.resolution.is_some() {
        ctx = ctx.with_feature("has_custom_resolution");
    }
    let main_class = version
        .main_class
        .clone()
        .ok_or_else(|| McError::Invalid(format!("{} has no main class", version.id)))?;

    let mut classpath: Vec<String> = library::resolve_all(version, &ctx)
        .iter()
        .filter(|lib| !lib.extract)
        .map(|lib| dir.library(&lib.path).to_string_lossy().into_owned())
        .collect();
    classpath.push(launch_jar(dir, version).to_string_lossy().into_owned());

    let index_id = version
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| version.assets.clone())
        .unwrap_or_else(|| "legacy".into());
    let game_assets = match AssetIndex::read(&dir.asset_index(&index_id)) {
        // Pre-1.6 versions read sounds from the shared copy made at install time.
        Ok(index) if index.map_to_resources => dir.root().join("resources"),
        Ok(index) if index.is_virtual => dir.virtual_assets(&index_id),
        _ => dir.assets(),
    };
    let path = |p: PathBuf| p.to_string_lossy().into_owned();
    let (width, height) = opts.resolution.unwrap_or((854, 480));
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", opts.account.name.clone()),
        ("auth_uuid", opts.account.uuid.clone()),
        ("auth_access_token", opts.account.access_token.clone()),
        ("auth_session", opts.account.access_token.clone()),
        ("auth_xuid", opts.account.xuid.clone()),
        ("clientid", String::new()),
        ("user_type", opts.account.user_type().into()),
        ("user_properties", "{}".into()),
        ("version_name", version.id.clone()),
        ("version_type", version.kind.clone().unwrap_or_else(|| "release".into())),
        ("game_directory", path(opts.run_dir.clone())),
        ("assets_root", path(dir.assets())),
        ("game_assets", path(game_assets)),
        ("assets_index_name", index_id),
        ("natives_directory", path(dir.natives_dir(&version.id))),
        ("library_directory", path(dir.libraries())),
        ("classpath_separator", platform.classpath_separator().into()),
        ("classpath", classpath.join(platform.classpath_separator())),
        ("primary_jar", path(launch_jar(dir, version))),
        ("launcher_name", opts.launcher_name.clone()),
        ("launcher_version", opts.launcher_version.clone()),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
    ]);

    let mut args = vec![format!("-Xmx{}M", opts.max_memory_mib)];
    if opts.log4j_fix {
        args.extend(log4j_args(dir, version));
    }
    args.extend(opts.extra_jvm_args.iter().cloned());
    let jvm = version.arguments.as_ref().map(|a| evaluate(&a.jvm, &ctx)).unwrap_or_default();
    // Pre-1.13 versions have no JVM arguments of their own; a loader may add
    // some (Ornithe does), which must not replace the classpath and natives setup.
    let has_classpath = jvm.iter().any(|a| a.contains("${classpath}"));
    args.extend(jvm);
    if !has_classpath {
        args.extend(legacy_jvm_args(platform));
    }
    args.push(main_class);
    match (&version.arguments, &version.minecraft_arguments) {
        (Some(arguments), _) if !arguments.game.is_empty() => args.extend(evaluate(&arguments.game, &ctx)),
        (_, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(str::to_owned));
            if opts.resolution.is_some() {
                args.extend(["--width", "${resolution_width}", "--height", "${resolution_height}"].map(String::from));
            }
        }
        _ => {}
    }
    if opts.fullscreen {
        args.push("--fullscreen".into());
    }

    Ok(LaunchCommand {
        program: opts.java.clone(),
        args: args.iter().map(|a| substitute(a, &vars)).collect(),
        working_dir: opts.run_dir.clone(),
    })
}

/// JVM arguments that close Log4Shell (CVE-2021-44228) for versions whose
/// Log4j predates 2.17: Mojang's patched logging configuration, which disables
/// message lookups in every Log4j version, and the `formatMsgNoLookups` switch
/// that Log4j 2.10+ understands on its own (it covers loaders that replace the
/// configuration). Newer versions get nothing.
pub fn log4j_args(dir: &GameDir, version: &VersionJson) -> Vec<String> {
    let Some(log4j) = version.log4j_version() else { return Vec::new() };
    if log4j >= (2, 17) {
        return Vec::new();
    }
    let mut args = Vec::new();
    if let Some(config) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = dir.log_config(&config.file.id);
        if path.is_file() {
            args.push(config.argument.replace("${path}", &path.to_string_lossy()));
        }
    }
    if log4j >= (2, 10) {
        args.push("-Dlog4j2.formatMsgNoLookups=true".into());
    }
    args
}

fn evaluate(arguments: &[Argument], ctx: &RuleContext) -> Vec<String> {
    arguments
        .iter()
        .flat_map(|arg| match arg {
            Argument::Plain(value) => vec![value.clone()],
            Argument::Conditional { rules, value } if rules::allowed(rules, ctx) => value.values().to_vec(),
            Argument::Conditional { .. } => Vec::new(),
        })
        .collect()
}

fn legacy_jvm_args(platform: Platform) -> Vec<String> {
    let mut args = Vec::new();
    match platform.os {
        Os::MacOs => args.push("-XstartOnFirstThread".to_owned()),
        Os::Windows => args.push(
            "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_owned(),
        ),
        Os::Linux => {}
    }
    args.extend(
        [
            "-Djava.library.path=${natives_directory}",
            "-Dminecraft.launcher.brand=${launcher_name}",
            "-Dminecraft.launcher.version=${launcher_version}",
            "-cp",
            "${classpath}",
        ]
        .map(String::from),
    );
    args
}

/// Replaces every `${name}` with its value; unknown placeholders are kept.
fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Arch;

    #[test]
    fn substitutes_placeholders() {
        let vars = HashMap::from([("a", "1".to_owned()), ("b", "x}y".to_owned())]);
        assert_eq!(substitute("-D${a}=${b}", &vars), "-D1=x}y");
        assert_eq!(substitute("${missing}-${a}", &vars), "${missing}-1");
        assert_eq!(substitute("${unterminated", &vars), "${unterminated");
    }

    #[test]
    fn keeps_the_classpath_when_a_loader_adds_jvm_arguments_to_a_legacy_version() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.8.9-ornithe","type":"release","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
            "minecraftArguments":"--username ${auth_player_name}",
            "arguments":{"jvm":["-Dfabric.gameVersion=1.8.9"],"game":[]},
            "libraries":[{"name":"a:b:1","downloads":{"artifact":{"path":"a/b/1/b-1.jar","url":"u"}}}]}"#,
        )
        .unwrap();
        let dir = GameDir::new(std::env::temp_dir().join("g"));
        let opts = LaunchOptions {
            java: "/usr/bin/java".into(),
            run_dir: dir.run_dir("x", true),
            account: Account::offline("Steve"),
            max_memory_mib: 1024,
            resolution: None,
            fullscreen: false,
            extra_jvm_args: vec![],
            log4j_fix: false,
            launcher_name: "Sulfur".into(),
            launcher_version: "0.1".into(),
        };
        let cmd = build(&dir, &version, &opts, Platform { os: Os::Linux, arch: Arch::X86_64 }).unwrap();
        assert!(cmd.args.contains(&"-Dfabric.gameVersion=1.8.9".to_owned()));
        let cp = cmd.args.iter().position(|a| a == "-cp").expect("classpath");
        assert!(cmd.args[cp + 1].contains("b-1.jar"));
        assert!(cmd.args.iter().any(|a| a.starts_with("-Djava.library.path=")));
    }

    #[test]
    fn applies_log4j_fix_only_to_vulnerable_versions() {
        let dir = GameDir::new(std::env::temp_dir().join(format!("sulfur-log4j-{}", std::process::id())));
        let version = |log4j: &str| -> VersionJson {
            serde_json::from_str(&format!(
                r#"{{"id":"x","libraries":[{{"name":"org.apache.logging.log4j:log4j-core:{log4j}"}}],
                "logging":{{"client":{{"argument":"-Dlog4j.configurationFile=${{path}}",
                "file":{{"id":"client-1.12.xml","sha1":"0","size":1,"url":"u"}},"type":"log4j2-xml"}}}}}}"#
            ))
            .unwrap()
        };
        assert_eq!(log4j_args(&dir, &version("2.17.1")), Vec::<String>::new());
        assert_eq!(log4j_args(&dir, &version("2.8.1")), Vec::<String>::new());
        assert_eq!(log4j_args(&dir, &version("2.14.1")), ["-Dlog4j2.formatMsgNoLookups=true"]);

        let config = dir.log_config("client-1.12.xml");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, "<Configuration/>").unwrap();
        let args = log4j_args(&dir, &version("2.0-beta9"));
        assert_eq!(args, [format!("-Dlog4j.configurationFile={}", config.display())]);
        assert_eq!(version("2.0-beta9").log4j_version(), Some((2, 0)));
        std::fs::remove_dir_all(dir.root()).unwrap();
    }

    #[test]
    fn builds_modern_command() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.21","type":"release","mainClass":"net.minecraft.client.main.Main",
            "assetIndex":{"id":"17","sha1":"0","size":1,"url":"u"},
            "arguments":{
              "game":["--username","${auth_player_name}",
                {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
                 "value":["--width","${resolution_width}"]}],
              "jvm":[{"rules":[{"action":"allow","os":{"name":"osx"}}],"value":["-XstartOnFirstThread"]},
                "-cp","${classpath}"]},
            "libraries":[{"name":"a:b:1","downloads":{"artifact":{"path":"a/b/1/b-1.jar","url":"u"}}}]}"#,
        )
        .unwrap();
        let dir = GameDir::new(std::env::temp_dir().join("g"));
        let opts = LaunchOptions {
            java: "/usr/bin/java".into(),
            run_dir: dir.run_dir("1.21", true),
            account: Account::offline("Steve"),
            max_memory_mib: 2048,
            resolution: None,
            fullscreen: false,
            extra_jvm_args: vec![],
            log4j_fix: true,
            launcher_name: "Sulfur".into(),
            launcher_version: "0.1".into(),
        };
        let linux = Platform { os: Os::Linux, arch: Arch::X86_64 };
        let cmd = build(&dir, &version, &opts, linux).unwrap();
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(cmd.args[0], "-Xmx2048M");
        assert_eq!(cmd.args[1], "-cp");
        assert!(cmd.args[2].ends_with(&format!("versions{sep}1.21{sep}1.21.jar")));
        assert_eq!(&cmd.args[3..], ["net.minecraft.client.main.Main", "--username", "Steve"]);

        let opts = LaunchOptions { resolution: Some((1280, 720)), ..opts };
        let cmd = build(&dir, &version, &opts, linux).unwrap();
        assert_eq!(&cmd.args[cmd.args.len() - 2..], ["--width", "1280"]);
    }
}
