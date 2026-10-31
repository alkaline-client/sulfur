package dev.sulfur.tools

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.unit.Density
import dev.sulfur.AppContainer
import dev.sulfur.feature.download.DownloadSection
import dev.sulfur.ffi.NavigationPosition
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

    class Shot(val name: String, val page: Destination, val theme: ThemeMode, val after: () -> Unit = {}, val setup: () -> Unit = {})
    val navigation = app.repository.settings.value.navigation
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
        val scene = ImageComposeScene(width = 1280, height = 820, density = Density(1f)) {
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
        shot.after()
        withTimeoutOrNull(30_000) {
            while ((app.modpacks.ui.value.target as? PackTarget.Remote)?.versions == null && app.modpacks.ui.value.target != null) delay(100)
        }
        frames(8)
        val png = scene.render(time).encodeToData(EncodedImageFormat.PNG)!!.bytes
        File(out, "${shot.name}.png").writeBytes(png)
        println("wrote ${File(out, "${shot.name}.png")}")
        scene.close()
        app.download.showSection(DownloadSection.Game)
        app.download.setQuery("")
        app.modpacks.close()
        app.repository.updateSettings { it.copy(navigation = navigation, language = language) }
    }
    app.close()
}

/** Animation time between rendered frames: 0.4 s. */
private const val FrameStep = 400_000_000L
