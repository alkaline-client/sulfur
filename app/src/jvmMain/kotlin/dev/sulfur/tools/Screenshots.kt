package dev.sulfur.tools

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.unit.Density
import dev.sulfur.AppContainer
import dev.sulfur.feature.download.DownloadSection
import dev.sulfur.ffi.LaunchColumn
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.LaunchWidget
import dev.sulfur.ffi.NavigationPosition
import dev.sulfur.ffi.WidgetKind
import dev.sulfur.ffi.SulfurCore
import dev.sulfur.ffi.ThemeMode
import dev.sulfur.ui.AppShell
import dev.sulfur.feature.modpacks.PackTarget
import dev.sulfur.ui.Destination
import dev.sulfur.ui.theme.LocalizedApp
import dev.sulfur.ui.theme.SulfurTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.system.exitProcess

/**
 * Renders pages off-screen into PNGs, without a display.
 *
 * Usage: `./gradlew :app:screenshots -Pout=<dir> -Pportable=<data dir> [-Pshots=name,name]`
 *
 * Every frame is rasterized on the CPU, so the tool renders as few as it can:
 * it waits for the app's data before drawing anything, then advances the
 * animation clock in large steps (animations finish within a second).
 */
@OptIn(ExperimentalComposeUiApi::class)
fun main(args: Array<String>) {
    // A crash on the UI thread would otherwise leave the JVM idling forever.
    Thread.setDefaultUncaughtExceptionHandler { _, e ->
        e.printStackTrace()
        exitProcess(1)
    }
    runBlocking(Dispatchers.Main) { render(args) }
    exitProcess(0)
}

@OptIn(ExperimentalComposeUiApi::class)
private suspend fun render(args: Array<String>) {
    val out = File(args.getOrElse(0) { "screenshots" }).apply { mkdirs() }
    val only = args.getOrNull(2).orEmpty().split(',').map { it.trim() }.filter { it.isNotEmpty() }.toSet()
    val app = AppContainer(SulfurCore(args.getOrNull(1)))

    class Shot(
        val name: String,
        val page: Destination,
        val theme: ThemeMode,
        val after: () -> Unit = {},
        /** Time for loads the shot starts (loader versions) before its last frames. */
        val settleMillis: Long = 0,
        val width: Int = 1280,
        val setup: () -> Unit = {},
    )
    val newestRelease = { app.download.ui.value.groups.firstOrNull { it.releases.isNotEmpty() } }
    val navigation = app.repository.settings.value.navigation
    val launchPage = app.repository.settings.value.let { it.launchColumn to it.launchWidgets }
    // One card of each kind; the Markdown card reads a page written next to the shots.
    val sampleCards = File(out, "sample.md").apply {
        writeText("# Server rules\n\nBe **kind**, no griefing, and read the [wiki](https://minecraft.wiki).\n\n- Survival on `play.example.com`\n- Backups every night\n\n> Have fun!\n")
    }.let { page ->
        listOf(
            LaunchWidget(WidgetKind.NEWS, "", ""),
            LaunchWidget(WidgetKind.VERSIONS, "", ""),
            LaunchWidget(WidgetKind.NOTE, "Today", "Finish the redstone door, then visit the nether fortress."),
            LaunchWidget(WidgetKind.LINKS, "", "Minecraft Wiki | https://minecraft.wiki\nModrinth | https://modrinth.com"),
            LaunchWidget(WidgetKind.MARKDOWN, "", page.absolutePath),
        )
    }
    // Shots are in English unless they say otherwise.
    val language = "en"
    app.repository.updateSettings { it.copy(language = language) }
    fun inLanguage(name: String, page: Destination, code: String) = Shot(name, page, ThemeMode.LIGHT) {
        app.repository.updateSettings { it.copy(language = code) }
    }
    val shots = (Destination.entries.map { Shot("${it.name.lowercase()}-light", it, ThemeMode.LIGHT) } +
        Shot("launch-dark", Destination.Launch, ThemeMode.DARK) +
        Shot("download-dark", Destination.Download, ThemeMode.DARK) +
        Shot("settings-dark", Destination.Settings, ThemeMode.DARK) +
        Shot("launch-side-light", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(navigation = NavigationPosition.SIDE) }
        } +
        Shot("download-mods-light", Destination.Download, ThemeMode.LIGHT) {
            app.download.showSection(DownloadSection.Mods)
        } +
        // The floating card of the newest release group.
        Shot("download-card-light", Destination.Download, ThemeMode.LIGHT) {
            app.download.openGroup(app.download.ui.value.groups.firstOrNull { it.releases.isNotEmpty() }?.key)
        } +
        // A release's install page inside its update's card, with every loader asked.
        Shot("download-version-light", Destination.Download, ThemeMode.LIGHT, settleMillis = 6_000) {
            newestRelease()?.let { group ->
                app.download.openGroup(group.key)
                app.download.showInstallOptions(group.releases.first().id)
            }
        } +
        Shot("download-version-dark", Destination.Download, ThemeMode.DARK, settleMillis = 6_000) {
            newestRelease()?.let { group ->
                app.download.openGroup(group.key)
                app.download.showInstallOptions(group.releases.first().id)
            }
        } +
        // Fabric chosen: its version, Fabric API and the new version's name.
        Shot("download-version-fabric-light", Destination.Download, ThemeMode.LIGHT, settleMillis = 6_000) {
            newestRelease()?.let { group ->
                app.download.openGroup(group.key)
                app.download.showInstallOptions(group.releases.first().id, LoaderKind.FABRIC)
            }
        } +
        // An old version opened from search: Legacy Fabric, Ornithe and OptiFine instead of Fabric and Forge's newer loaders.
        Shot("download-version-search-light", Destination.Download, ThemeMode.LIGHT, settleMillis = 6_000) {
            app.download.setQuery("1.8.9")
            app.download.showInstallOptions("1.8.9")
        } +
        Shot("download-version-zh-cn", Destination.Download, ThemeMode.LIGHT, settleMillis = 6_000) {
            app.repository.updateSettings { it.copy(language = "zh-CN") }
            app.download.showInstallOptions("1.12.2")
        } +
        // A narrow window with a long loader version on Play.
        Shot("launch-narrow-dark", Destination.Launch, ThemeMode.DARK, width = 900) {
            app.repository.installed.value.maxByOrNull { it.id.length }?.let { app.launch.select(it.id) }
        } +
        // A dozen mods picked for a Forge version, in a narrow window.
        Shot("download-mods-list-narrow-dark", Destination.Download, ThemeMode.DARK, width = 900, settleMillis = 4_000, after = {
            app.mods.ui.value.hits.take(12).forEach(app.mods::toggleListed)
        }) {
            app.repository.installed.value.firstOrNull { it.loader == LoaderKind.FORGE }?.let { app.mods.selectVersion(it.id) }
            app.download.showSection(DownloadSection.Mods)
        } +
        Shot("misc-dark", Destination.Misc, ThemeMode.DARK) +
        // The oldest updates and April Fools, at the end of the grid.
        Shot("download-oldest-dark", Destination.Download, ThemeMode.DARK) {
            app.download.setQuery("")
            app.download.ui.value.groups.lastOrNull()?.let { app.download.openGroup(it.key) }
            app.download.openGroup(null)
        } +
        Shot("launch-version-mods-dark", Destination.Launch, ThemeMode.DARK) {
            app.repository.installed.value.firstOrNull { it.loader == LoaderKind.FORGE }?.let { app.launch.select(it.id) }
            app.launch.versionSettingsOpen.value = true
        } +
        Shot("download-archive-light", Destination.Download, ThemeMode.LIGHT) {
            app.download.showSection(DownloadSection.Archive)
        } +
        Shot("download-modpacks-light", Destination.Download, ThemeMode.LIGHT) {
            app.download.showSection(DownloadSection.Modpacks)
        } +
        // The install dialog of the first modpack found.
        Shot("download-modpack-dialog-light", Destination.Download, ThemeMode.LIGHT, after = {
            app.modpacks.ui.value.hits.firstOrNull()?.let(app.modpacks::open)
        }) {
            app.download.showSection(DownloadSection.Modpacks)
        } +
        // Finds 1.17 and 1.18 by their update's Chinese name.
        Shot("download-search-zh-cn", Destination.Download, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(language = "zh-CN") }
            app.download.setQuery("洞穴")
        } +
        Shot("launch-cards-light", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(launchWidgets = sampleCards) }
        } +
        Shot("launch-cards-right-dark", Destination.Launch, ThemeMode.DARK) {
            app.repository.updateSettings { it.copy(launchWidgets = sampleCards, launchColumn = LaunchColumn.RIGHT) }
        } +
        Shot("launch-cards-text-light", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(launchWidgets = sampleCards.reversed()) }
        } +
        Shot("launch-customize-light", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(launchWidgets = sampleCards) }
            app.launch.customizing.value = true
        } +
        Shot("launch-version-general-light", Destination.Launch, ThemeMode.LIGHT) {
            app.launch.versionSettingsOpen.value = true
        } +
        Shot("launch-version-general-zh-tw", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(language = "zh-TW") }
            app.launch.versionSettingsOpen.value = true
        } +
        Shot("launch-cards-zh-cn", Destination.Launch, ThemeMode.LIGHT) {
            app.repository.updateSettings { it.copy(launchWidgets = sampleCards, language = "zh-CN") }
        } +
        inLanguage("launch-zh-cn", Destination.Launch, "zh-CN") +
        inLanguage("download-zh-cn", Destination.Download, "zh-CN") +
        inLanguage("settings-zh-cn", Destination.Settings, "zh-CN") +
        inLanguage("download-zh-tw", Destination.Download, "zh-TW") +
        inLanguage("settings-zh-tw", Destination.Settings, "zh-TW") +
        Shot("download-modpack-dialog-zh-tw", Destination.Download, ThemeMode.LIGHT, after = {
            app.modpacks.ui.value.hits.firstOrNull()?.let(app.modpacks::open)
        }) {
            app.repository.updateSettings { it.copy(language = "zh-TW") }
            app.download.showSection(DownloadSection.Modpacks)
        }).filter { only.isEmpty() || it.name in only }

    // Version list and artwork load once for all shots; no frames are drawn meanwhile.
    withTimeoutOrNull(20_000) {
        while (app.download.ui.value.let { it.loading || it.groups.isEmpty() && it.error == null }) delay(100)
        while (app.download.ui.value.banners.isEmpty() && app.download.ui.value.error == null) delay(100)
    }

    for (shot in shots) {
        app.destination.value = shot.page
        val scene = ImageComposeScene(width = shot.width, height = 820, density = Density(1f)) {
            val settings by app.repository.settings.collectAsState()
            LocalizedApp(settings.language) {
                SulfurTheme(shot.theme, settings.seedColor.toLong(), settings.highContrast) {
                    AppShell(app)
                }
            }
        }
        var time = 0L
        suspend fun frames(count: Int) = repeat(count) {
            scene.render(time)
            time += FrameStep
            // Lets loads started by the frame (images, mod search) finish before the next one.
            delay(150)
        }
        frames(8)
        shot.setup()
        // The archive list comes from Omniarchive's folder pages on first use.
        withTimeoutOrNull(60_000) { while (app.download.ui.value.archive.loading) delay(100) }
        frames(4)
        withTimeoutOrNull(30_000) { while (app.modpacks.ui.value.loading) delay(100) }
        withTimeoutOrNull(30_000) { while (app.mods.ui.value.loading) delay(100) }
        shot.after()
        if (shot.settleMillis > 0) {
            frames(2)
            delay(shot.settleMillis)
        }
        withTimeoutOrNull(30_000) {
            while ((app.modpacks.ui.value.target as? PackTarget.Remote)?.versions == null && app.modpacks.ui.value.target != null) delay(100)
        }
        frames(8)
        val png = scene.render(time).encodeToData(EncodedImageFormat.PNG)!!.bytes
        File(out, "${shot.name}.png").writeBytes(png)
        println("wrote ${File(out, "${shot.name}.png")}")
        scene.close()
        app.download.showSection(DownloadSection.Game)
        app.download.closeCards()
        app.download.setQuery("")
        app.modpacks.close()
        app.launch.customizing.value = false
        app.launch.versionSettingsOpen.value = false
        app.repository.updateSettings {
            it.copy(navigation = navigation, language = language, launchColumn = launchPage.first, launchWidgets = launchPage.second)
        }
    }
    app.close()
}

/** Animation time between rendered frames: 0.4 s. */
private const val FrameStep = 400_000_000L
