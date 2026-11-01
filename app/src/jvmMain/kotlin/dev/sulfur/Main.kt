package dev.sulfur

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import dev.sulfur.data.ImageCache
import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.ThemeColors
import dev.sulfur.feature.download.DownloadViewModel
import dev.sulfur.feature.launch.LaunchViewModel
import dev.sulfur.feature.misc.MiscViewModel
import dev.sulfur.feature.modpacks.ModpacksViewModel
import dev.sulfur.feature.mods.ModsViewModel
import dev.sulfur.ffi.SulfurCore
import dev.sulfur.ui.AppShell
import dev.sulfur.ui.Destination
import dev.sulfur.ui.IsMac
import dev.sulfur.ui.WindowFrame
import dev.sulfur.resources.Res
import dev.sulfur.resources.sulfur_logo
import dev.sulfur.ui.theme.LocalizedApp
import dev.sulfur.ui.theme.SulfurTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.jetbrains.compose.resources.painterResource
import dev.sulfur.util.installDesktopEntry
import dev.sulfur.util.setWindowClass
import kotlinx.coroutines.withContext
import java.awt.Dimension
import javax.swing.JOptionPane
import kotlin.system.exitProcess

/** Everything the UI needs, created once at startup (manual dependency injection). */
class AppContainer(val core: SulfurCore) {
    /** The page shown. */
    val destination = mutableStateOf(Destination.Launch)
    /** Whether the task list is open over the page. */
    val tasksOpen = mutableStateOf(false)
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    val repository = SulfurRepository(core, scope)
    val launch = LaunchViewModel(repository, scope)
    val download = DownloadViewModel(repository, scope)
    val misc = MiscViewModel(repository, scope)
    val mods = ModsViewModel(repository, scope)
    val modpacks = ModpacksViewModel(repository, scope)
    val images = ImageCache(repository)
    val themeColors = ThemeColors(repository, scope)

    fun close() {
        scope.cancel()
        core.close()
    }
}

/** `SULFUR_START_PAGE=Download` opens another page first (useful for development). */
private fun startPage(): Destination =
    Destination.entries.firstOrNull { it.name.equals(System.getenv("SULFUR_START_PAGE"), ignoreCase = true) }
        ?: Destination.Launch

fun main() {
    val core = try {
        // -Dsulfur.portable=<dir> (or SULFUR_PORTABLE) keeps all data in one folder.
        SulfurCore(System.getProperty("sulfur.portable") ?: System.getenv("SULFUR_PORTABLE"))
    } catch (e: Throwable) {
        JOptionPane.showMessageDialog(null, "Sulfur could not start:\n${e.message}", "Sulfur", JOptionPane.ERROR_MESSAGE)
        exitProcess(1)
    }
    val app = AppContainer(core)
    app.destination.value = startPage()
    setWindowClass()

    application {
        val windowState = rememberWindowState(
            size = DpSize(1200.dp, 780.dp),
            position = WindowPosition(Alignment.Center),
        )
        val quit = {
            app.close()
            exitApplication()
        }
        Window(
            onCloseRequest = quit,
            state = windowState,
            title = "Sulfur",
            // Sulfur draws its own title bar; macOS keeps its window with a transparent title bar.
            undecorated = !IsMac,
        ) {
            window.minimumSize = Dimension(960, 640)
            // Vector painters need a window's composition, so the icon is set from inside it.
            val logo = painterResource(Res.drawable.sulfur_logo)
            val density = LocalDensity.current
            LaunchedEffect(logo) {
                val icon = logo.toAwtImage(density, LayoutDirection.Ltr, Size(256f, 256f))
                window.iconImage = icon
                withContext(Dispatchers.IO) { installDesktopEntry(icon) }
            }
            val settings by app.repository.settings.collectAsState()
            val seed by app.themeColors.seed.collectAsState()
            // A wallpaper changed while Sulfur was in the background recolors it on return.
            val focused = LocalWindowInfo.current.isWindowFocused
            LaunchedEffect(focused) { if (focused) app.themeColors.refreshWallpaper() }
            LocalizedApp(settings.language) {
                SulfurTheme(
                    theme = settings.theme,
                    seed = seed,
                    highContrast = settings.highContrast,
                    corners = settings.corners,
                ) {
                    WindowFrame(window, onClose = quit) {
                        AppShell(app)
                    }
                }
            }
        }
    }
}
