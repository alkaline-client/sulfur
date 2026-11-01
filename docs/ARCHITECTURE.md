# Architecture

Sulfur is two programs in one process: a **Rust core** that owns all I/O and
game logic, and a **Kotlin/Compose UI** that only presents state and forwards
user intent. They meet at one narrow, generated boundary.

```
┌──────────────────────── Kotlin (JVM) ────────────────────────┐
│ ui/ (AppShell, theme)   feature/{launch,download,settings,misc} │
│        │ StateFlow                     ▲ intents                 │
│ data/SulfurRepository  ── polls tasks / GameSession ────────┐   │
│        │ generated bindings (dev.sulfur.ffi, JNA)           │   │
└────────┼────────────────────────────────────────────────────┼───┘
         ▼  UniFFI                                            │
┌──────────────────────── Rust ────────────────────────────────┐
│ sulfur-ffi      SulfurCore object, records, error mapping     │
│   ├─ sulfur-minecraft   manifest · version · rules · library  │
│   │                     install · loader · groups · banners   │
│   │                     launch · java · process               │
│   ├─ sulfur-mods        Modrinth · CurseForge                 │
│   ├─ sulfur-download    Downloader · segmented · mirrors      │
│   └─ sulfur-core        AppDirs · Settings (TOML)             │
└───────────────────────────────────────────────────────────────┘
```

## Rust crates

| Crate | Responsibility | Depends on |
|---|---|---|
| `sulfur-core` | Platform directories, settings schema and persistence, saved accounts, settings import from HMCL and PCL (`import`). No networking. | – |
| `sulfur-download` | Generic download engine. Knows nothing about Minecraft. | – |
| `sulfur-minecraft` | Mojang metadata, rules, library resolution, installer, mod loaders (Fabric, Forge, NeoForge, OptiFine), update groups and artwork, Omniarchive versions, launch command, Java discovery, game processes, Microsoft sign-in (`msa`), the region rule for offline accounts. | core, download |
| `sulfur-mods` | Mod search and file selection on Modrinth and CurseForge (official API with a key, MCIM mirror without); reading Modrinth modpacks (`mrpack`). | download |
| `sulfur-ffi` | The only API Kotlin sees: one `SulfurCore` object, flat records, mapped errors. | all |
| `uniffi-bindgen` | Generates `dev.sulfur.ffi` Kotlin bindings from the compiled library. | – |
| `sulfur-bundle` | The single executable: `sulfur-stub` unpacks the app folder appended to it and starts it; `sulfur-pack` (feature `pack`) appends the folder. | – |

Rules that keep this maintainable:

- **Core crates never depend on UniFFI.** `sulfur-ffi/src/types.rs` is the
  only translation layer, so the core can serve a CLI or another UI later.
- **Long operations are objects, not callbacks.** `InstallTask` and
  `GameSession` expose cheap `snapshot()`/`logs_since()` calls that the UI polls
  at frame rate. No callbacks into the JVM, no cross-thread re-entrancy. The
  core keeps every task of the session, including the ones it starts itself
  for Play (a Java download, an Alkaline build), and the UI reads them all
  with `install_snapshots()` for its task list.
- **One corner scale.** `ui/theme/Theme.kt` builds Material's shape scale from
  the Corners setting, and `Corners` names the role of each surface (page,
  card, item, inset, logo). Pages use those roles rather than
  `MaterialTheme.shapes` or their own `RoundedCornerShape`, so one setting
  rounds the whole UI, the Windows 11 window frame included.
- **One Tokio runtime**, owned by `SulfurCore`. Async FFI methods spawn onto
  it and are awaited as Kotlin `suspend` functions.
- **Settings are owned by Rust** (`settings.toml`) so every frontend shares
  them; the UI edits a copy and the repository saves it debounced.

## Kotlin app

| Package | Contents |
|---|---|
| `dev.sulfur` | `Main.kt`: window, `AppContainer` (manual DI). |
| `dev.sulfur.data` | `SulfurRepository`: flows over the core, settings auto-save, install polling, snackbar messages. `ImageCache`: banners and mod icons. |
| `dev.sulfur.feature.*` | One package per page: a view model (plain class with `StateFlow`s) and its screen. |
| `dev.sulfur.ui` | `AppShell` (navigation rail, page transitions), `WindowChrome` (the window frame), theme, shared components. |
| `dev.sulfur.tools` | Developer tools, e.g. the off-screen screenshot renderer. |
| `dev.sulfur.ffi` | Generated at build time; never edited by hand. |

### Material 3 Expressive

Everything visual comes from `org.jetbrains.compose.material3` 1.13 (Google's
Material 3 Expressive implementation for Compose): `MaterialExpressiveTheme`
via MaterialKolor's `DynamicMaterialExpressiveTheme` (seed color, Expressive
palette style, 2025 color spec), `MotionScheme.expressive()`,
`WideNavigationRail`, `SplitButtonLayout`, connected `ToggleButton` groups,
shape-morphing `Button`s, `LinearWavyProgressIndicator`,
`CircularWavyProgressIndicator`, `LoadingIndicator`/`ContainedLoadingIndicator`
and `MaterialShapes`. Sulfur adds no custom components that imitate M3E; when
Google adds components, upgrade the `material3` version in
`gradle/libs.versions.toml`.

Motion guidelines used across pages:

- Spatial movement (slides, size, rotation): `motionScheme.*SpatialSpec()`.
- Fades and color: `motionScheme.*EffectsSpec()`.
- Page changes: fade + short vertical slide + slight scale, with direction
  following the rail order.

### Window frame

Sulfur draws its own title bar. On Windows and Linux the window is
undecorated: the top bar (or, with the side rail, a thin strip above the page)
drags the window and maximizes it on a double click, minimize, maximize and
close sit at its end, and 5 dp edges resize the window. On Windows Sulfur
maximizes to the screen's work area itself (a maximized undecorated window
would cover the taskbar) and asks DWM for Windows 11's rounded corners. On
macOS the window keeps its system frame with a transparent, full-size title
bar, so the traffic lights float over the app's own bar, which leaves room for
them on the left.

### Mod loaders

Every loader becomes a version that `inheritsFrom` the vanilla one, the same
layout the official launcher uses:

- **Fabric, Legacy Fabric, Ornithe, Quilt**: the version JSON comes from each
  project's meta server (`meta.fabricmc.net`, `meta.legacyfabric.net`,
  `meta.ornithemc.net` gen2, `meta.quiltmc.org`); libraries carry checksums.
  Ornithe names its parent `<game>-vanilla`, so `inheritsFrom` is rewritten.
  Fabric API is added from Modrinth into the version's mods folder.
- **Forge / NeoForge**: the official installer jar is downloaded, its
  `install_profile.json` read, bundled libraries extracted and the client
  *processors* (deobfuscation, binary patches) run with the game's Java.
  Pre-1.13 Forge installers carry the version JSON and universal jar directly.
- **Cleanroom** (Forge fork for 1.12.2): a processor-less installer of the same
  format; the version JSON gains `javaVersion` 21, which its metadata omits.
  The Cleanroom team asks launchers not to automate downloads (their download
  page carries their support), so by default Sulfur shows
  `download.cleanroommc.com` in a JavaFX WebView and installs the installer
  link the user clicks there (only links to the project's GitHub releases or
  Maven repository are accepted). A downloaded installer file works too.
  *Download Cleanroom automatically* (off by default) uses the Maven repository.
- **Rift** (1.13.2 only, archived): the jar from GitHub releases carries a
  `profile.json` whose repositories are gone; Sulfur writes the version JSON
  with Mixin from SpongePowered's repository and ASM from Maven Central.
- **OptiFine**: `optifine.Patcher` turns the client jar into the OptiFine
  library; LaunchWrapper loads it through `optifine.OptiFineTweaker`.

Version lists come from the loaders' Maven metadata (Forge marks the
recommended build via `promotions_slim.json`), the meta servers, BMCLAPI for
OptiFine and a fixed list for Rift.

### Java runtimes

`java::discover_in` probes the usual install locations plus runtimes Sulfur
downloaded into `<data>/java/<major>-<arch>/`. For each launch (and each
loader installer) `State::find_java` takes, in order: the Java set for the
version, else the one set in Settings; an installed runtime of exactly the needed major version, or a
newer one for games needing Java 16+ (older games and their loaders often
break on newer Java); a fresh download of that version when *Download Java
automatically* is on (Eclipse Temurin via the Adoptium API, Azul Zulu where
Temurin has no build, e.g. Java 8 on Apple silicon); finally the closest newer
runtime. Downloads run as install tasks named `Java <major>`, so their progress
shows with the game installs.

### Launch settings

`GameSettings` holds the global values and `version_settings`, one
`VersionSettings` of optional overrides per version id (empty overrides are
dropped). `GameSettings::launch_settings(id)` merges them into a
`LaunchSettings`; blank text follows the global value. From that, `launch.rs`
builds the command:

- `-Xms`/`-Xmx`, then `gc_args(gc, java_major)`, then the user's JVM
  arguments, so the user can still override any GC flag. G1 uses the official
  launcher's tuning; ZGC adds `-XX:+ZGenerational` on Java 21–22 (the default
  from 23) and needs Java 17+; Shenandoah needs Java 12+. A collector the Java
  lacks falls back to the JVM's own choice.
- Extra game arguments, and a server to join: through Quick Play when the
  version supports it (`quickPlayMultiplayer`), otherwise `--server`/`--port`.
- A wrapper command (`gamemoderun`, `prime-run`, `mangohud`…) goes in front of
  `java`; environment variables (`KEY=value` lines) are added to the process.
- A pre-launch command runs first in the game folder through `sh -c` (or
  `cmd /C`) with `INST_NAME`, `INST_ID`, `INST_DIR`, `INST_MC_DIR` and
  `INST_JAVA` set (the names MultiMC and Prism use); a failure stops the launch.

Arguments are split like a shell does with quotes (`split_args`), so
`-Dname="two words"` stays one argument.

### Theme color and wallpaper

The theme's seed is the picked color, or one sampled from the desktop
wallpaper or the background picture (`ThemeColors`): the picture is scaled to
128 px and run through Material's Celebi quantizer and scoring, as Android
does for wallpaper colors. The wallpaper is looked up again when the window
gains focus, at most every five seconds.

`sulfur_core::wallpaper::detect` finds the wallpaper per platform:

- **Windows:** `HKCU\Control Panel\Desktop\WallPaper`, else the
  `TranscodedWallpaper` copy, or the solid `Colors\Background` color.
- **macOS:** `NSWorkspace.desktopImageURLForScreen` through JXA, else the
  Dock's `desktoppicture.db`.
- **Linux:** probes ordered by `XDG_CURRENT_DESKTOP`: COSMIC's background
  state, KDE Plasma's `plasma-org.kde.plasma.desktop-appletsrc`, Xfce's
  `xfce4-desktop.xml`, GNOME/Cinnamon/MATE/Budgie through `gsettings` (with
  the dark-style picture and XML slideshows), LXQt and LXDE config, then
  wallpaper programs used by Hyprland, Niri, Sway and others: hyprpaper,
  swww/awww, wpaperd, swaybg, wbg and xwallpaper (from their command lines in
  `/proc`), waypaper, feh and nitrogen.

Pictures Skia cannot decode (HEIC, JPEG XL, AVIF, TIFF, SVG) are converted to
PNG in the cache with whichever of `sips`, `magick`, `convert`, `djxl`,
`rsvg-convert` or Quick Look is there. Every external command has a three-second
time limit.

### Launch page

A version's settings open as a page in place of the launch layout (the
button beside Play; Back or Esc returns). The page is laid out like Settings,
with General, Java and memory, and Advanced sections; changes save half a
second after the last edit and when the page closes by any route.

`LaunchPageSettings` stores the Play column's side and a list of
`LaunchWidget`s (kind, title, content). Cards: Minecraft news (the official
launcher's feed, `launchercontent.mojang.com/v2/news.json`, Java entries only),
the latest release and snapshot with an Install button, a note, links
(`label | url` lines) and a Markdown page read by `SulfurCore::read_document`
from a file or an https address (at most 512 KiB). `ui/components/Markdown.kt`
renders the common subset: headings, paragraphs, lists, quotes, code, rules,
bold, italic, inline code and links.

### Log4Shell

Mojang's version JSON names a patched Log4j configuration (`logging.client`),
downloaded at install into `assets/log_configs/`. For versions whose
`log4j-core` is older than 2.17, launches add that configuration and, for
Log4j 2.10+, `-Dlog4j2.formatMsgNoLookups=true`. Settings → Java can turn it off.

### Mods and other projects

The Mods & resources section browses mods, resource packs, data packs and
shaders on Modrinth, CurseForge or both. With both, each source is searched
with the same page offset and `sulfur_mods::merge` interleaves the results by
rank, folding a project found on both sites into one hit (same URL name or
title, and similar author names) with summed downloads; later pages merge into
earlier ones the same way (`merge_mod_hits`). Installing tries the hit's
sources in order until one has a file for the game version (and loader, for
mods). Files go to `mods/`, `resourcepacks/`, `shaderpacks/` or
`saves/<world>/datapacks/`.

Ctrl-click (⌘-click on macOS) on a result or its Install button starts a
**download list**; while it is open, clicks add or remove results, across the
Mods, Resource packs and Shaders tabs. *Install all* installs the list into the
selected version, three projects at a time; projects that fail stay listed.
Data packs are left out because they go into one world at a time.

### Modpacks

The Modpacks section searches Modrinth (`project_type:modpack`) and lists each
pack's releases with their `.mrpack` files. `SulfurCore::install_modpack` (or
`install_modpack_file` for a file on disk) runs as one install task named after
the new version:

1. Download the `.mrpack` into `<cache>/modpacks/` (checked by SHA-1).
2. Read `modrinth.index.json` (`mrpack::PackIndex`): the Minecraft version, at
   most one loader (`forge`, `neoforge`, `fabric-loader`, `quilt-loader`) and
   the files. Files the client does not use (`env.client: unsupported`) are
   skipped; every other file needs a relative path that stays inside the game
   folder, a SHA-1, and download links on the hosts the format allows
   (`cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`,
   `gitlab.com`), or the pack is refused.
3. Install the game and the loader as version `<name>` (packs name Forge builds
   without the game version, so `47.3.0` is matched to `1.20.1-47.3.0`). A pack
   without a loader gets a version that only inherits from the game.
4. Isolate the version, download the files into `versions/<name>/` through the
   usual mirrors, then copy `overrides/` and `client-overrides/` over them.

A failed install removes the version folder it created. CurseForge packs and
NeoForge for 1.20.1 (published under a different Maven name) are not supported
yet.

### Update cards

`groups::group` sorts the manifest into major updates (`1.21`, `26.3`); every
snapshot joins the first release after it, and snapshots after the latest
release form an "upcoming" group. April Fools versions (snapshots published on
April 1, plus 1.RV-Pre1) get a group of their own at the end. `banners::fetch` takes each group's artwork
from the official launcher's patch notes (1.13 and newer), falling back to the
lead image of the update's Minecraft Wiki page and finally to a color fill.
Artwork is cached in `<cache>/banners.json` and `<cache>/images/`.

### Omniarchive

Omniarchive retired its launcher manifest in 2026; it now serves plain client
jars from browsable folders (`omniarchive.net/archive/java/client/<era>/`).
`omniarchive::list` reads those pages (cached in `<cache>/omniarchive.json`)
and leaves out ids Mojang still offers. Installing one downloads the official
version JSON of the nearest version of the same era (`template_for`: `a1.0.1_01`
→ `a1.0.4`, indev → `inf-20100618`, `1.6-pre-…` → `1.6.4`, 2.0 → `1.5.1`),
swaps in the archived jar and continues like any install
(`Installer::install_json`). The Download page shows them in their own Archive
tab, by file name.

### Accounts and region

Offline accounts are allowed only when `region::Region::detect` says mainland
China, where Java Edition is not sold. Like HMCL, it trusts the system time
zone (`Asia/Shanghai`, `Asia/Urumqi` and their aliases) and only falls back to
the locale's region when the zone says nothing (`UTC`, `Etc/*`, missing).
`account::offline_allowed` is the single rule, checked by `SulfurCore::launch`
and shown by the Launch page; it deliberately has no override besides owning
the game: a signed-in Microsoft account (which only exists for owners) allows
offline accounts everywhere. Sign-in is described in
[MICROSOFT_SIGN_IN.md](MICROSOFT_SIGN_IN.md).

### Importing other launchers' settings

`sulfur_core::import::scan` looks in folders the user picks (where HMCL or PCL
is) and in user-wide locations (`%APPDATA%\PCLCE`, HMCL's user home):

| Launcher | Files |
|---|---|
| HMCL 3.16+ | `.hmcl/config/{game-directories,game-settings,launcher-settings,accounts}.json` |
| HMCL before 3.16 | `hmcl.json` / `.hmcl.json` |
| PCL CE | `PCL/config.v1.yml` (flat YAML) + `%APPDATA%\PCLCE\config.v1.json` |
| PCL2 | `PCL/Setup.ini` (`key:value` lines) + `HKCU\Software\PCL` |

Only settings with a Sulfur equivalent are read; accounts are not (PCL
encrypts them per machine).

### Languages

All UI text is in `app/src/commonMain/composeResources/values/strings.xml` and
read with `stringResource`/`pluralStringResource`; snackbar messages carry a
`StringResource` (`UiMessage`) and are resolved where they are shown. To add a
language, copy the file to `values-<lang>` (`values-zh-rCN`, `values-de`, …)
and translate it. Simplified (`values-zh-rCN`) and Traditional Chinese
(`values-zh-rTW`, also used for Hong Kong, Macau and `zh-Hant`) exist; update
names follow the Chinese Minecraft Wiki. The language follows the system or
Settings → Appearance; `LocalizedApp` swaps the default locale and recomposes.
Layout rules that keep translations fitting:

- No fixed widths around text; button groups wrap (`ConnectedChoice(fill = false)`)
  instead of truncating, and single-line labels ellipsize only where a full
  value is shown elsewhere.
- Numbers and dates go through `formatCount`/`formatDate`, which follow the locale
  (e.g. "1.2万" in Chinese).
- Fonts: Google Sans Flex first, in every language, and MiSans (Xiaomi,
  variable weight) for the characters Google Sans Flex lacks (Han, kana). The
  `fetchCjkFont` Gradle task takes `MiSansVF.ttf` out of Xiaomi's download with
  HTTP range requests, checks its SHA-256 and embeds it as a classpath resource;
  it is not committed because the MiSans license forbids redistributing the font
  files on their own, and it requires crediting MiSans in the app (Misc →
  Credits). Compose desktop gives Skia a list of fonts to try in order only for
  its generic families, so `Theme.kt` registers both fonts with Skia's font
  collection and puts them at the front of the sans-serif list (through two
  Compose internals, read by reflection). If that fails, or MiSans is missing,
  the type scale is Google Sans Flex alone and Skia uses the system's CJK font.
- Messages from the Rust core (errors) are English for now.

## Build pipeline

`app/build.gradle.kts` wires the Rust side into Gradle:

1. `cargoBuild`: `cargo build -p sulfur-ffi --profile release`.
2. `generateBindings`: runs `uniffi-bindgen` on the built library; the output
   becomes a Kotlin source directory.
3. `bundleNativeLib`: copies the library to `<os>-<arch>/` on the classpath,
   where JNA finds it at runtime (`linux-aarch64`, `win32-x86-64`, …).

Cross-compiling: `-Psulfur.rustTarget=<triple> -Psulfur.jnaPrefix=<os-arch>`.

### One executable

Compose Desktop can only make an app *folder* (a jlinked Java runtime, the
jars and the Rust core, about 210 MB); compiling Compose, Skiko, JavaFX and
JNA ahead of time into one native binary is not practical. So
`packageSingleExecutable` takes the folder from `createReleaseDistributable`
and `sulfur-pack` writes:

```text
sulfur-stub | app folder as tar.zst | {"id", "entry"} JSON | lengths | "SULFURPK"
```

`id` is the start of the payload's SHA-256. On start the stub reads its own
trailer and, unless `<cache>/app/<id>/.complete` exists, unpacks into a
temporary folder that is renamed into place once complete, then removes
other builds' folders. It `exec`s the app's launcher on Unix (so exit codes
and signals pass straight through) and spawns it on Windows, where the stub is
a GUI program with Sulfur's icon. `SULFUR_PORTABLE` moves the unpacked app
under the portable folder like the rest of the data.

The stub decompresses with `ruzstd` (pure Rust), so it builds for any target
without a C compiler; only the packer, which runs at build time, uses the C
zstd library. Compose packages for the host OS only, so each platform's
executable is built on that platform.

## On-disk layout

```
<data>/minecraft/                 game directory (configurable)
  versions/<id>/<id>.json|.jar    shared metadata and jars
  versions/<id>/{saves,mods,…}    per-version state when isolated (default)
  libraries/  assets/             shared by every version
<config>/settings.toml
<data>/logs/sulfur.log
```

The layout matches the official launcher's, so existing game directories can
be used directly or as reuse sources.
