package dev.sulfur.feature.download

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.SwingPanel
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogWindow
import androidx.compose.ui.window.rememberDialogState
import dev.sulfur.ffi.cleanroomDownloadPage
import dev.sulfur.ffi.cleanroomInstallerVersion
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_open_in_browser
import dev.sulfur.resources.cleanroom_page_hint
import dev.sulfur.resources.cleanroom_page_title
import dev.sulfur.resources.cleanroom_page_unavailable
import dev.sulfur.util.openUrl
import javafx.application.Platform
import javafx.concurrent.Worker
import javafx.embed.swing.JFXPanel
import javafx.scene.Scene
import javafx.scene.web.WebEngine
import javafx.scene.web.WebView
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.stringResource
import java.net.URI
import javax.swing.SwingUtilities

/**
 * Cleanroom's own download page in a window, so the project keeps its
 * visitors while Sulfur still does the install: when the user picks an
 * installer there, [onPicked] receives its link instead of a browser download.
 */
@Composable
fun CleanroomPageWindow(onPicked: (String) -> Unit, onClose: () -> Unit) {
    val url = remember { cleanroomDownloadPage() }
    val picked by rememberUpdatedState(onPicked)
    val scope = rememberCoroutineScope()
    DialogWindow(
        onCloseRequest = onClose,
        title = stringResource(Res.string.cleanroom_page_title),
        state = rememberDialogState(size = DpSize(1040.dp, 760.dp)),
    ) {
        val surface = MaterialTheme.colorScheme.surface
        Surface(color = surface) {
            Column(Modifier.fillMaxSize()) {
                var failed by remember { mutableStateOf(false) }
                var loading by remember { mutableStateOf(true) }
                var progress by remember { mutableFloatStateOf(0f) }
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        stringResource(if (failed) Res.string.cleanroom_page_unavailable else Res.string.cleanroom_page_hint),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = { scope.launch { openUrl(url) } }) {
                        Text(stringResource(Res.string.action_open_in_browser))
                    }
                }
                // Room for the progress bar is kept so the page does not jump when it goes.
                Box(Modifier.fillMaxWidth().height(4.dp)) {
                    if (!failed && loading) {
                        LinearWavyProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp))
                    }
                }
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    if (!failed) {
                        val page = remember {
                            CleanroomWebPage(
                                url = url,
                                dark = surface.luminance() < 0.5f,
                                background = surface,
                                onInstaller = { link -> SwingUtilities.invokeLater { picked(link) } },
                                onExternal = { link -> scope.launch { openUrl(link) } },
                                onProgress = { p -> SwingUtilities.invokeLater { progress = p } },
                                onLoading = { l -> SwingUtilities.invokeLater { loading = l } },
                                onFailed = { SwingUtilities.invokeLater { failed = true } },
                            )
                        }
                        DisposableEffect(page) { onDispose { page.close() } }
                        SwingPanel(
                            background = surface,
                            factory = { runCatching { page.panel() }.getOrElse { failed = true; javax.swing.JPanel() } },
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
            }
        }
    }
}

/**
 * A JavaFX WebView showing [url]. Links to Cleanroom installers (they open
 * in a new tab on the page) go to [onInstaller]; links that leave the site
 * go to [onExternal] for the system browser. The page follows the app's
 * light or dark theme, which WebKit in JavaFX cannot report to it. JavaFX
 * calls the callbacks on its own thread.
 */
private class CleanroomWebPage(
    private val url: String,
    private val dark: Boolean,
    private val background: Color,
    private val onInstaller: (String) -> Unit,
    private val onExternal: (String) -> Unit,
    private val onProgress: (Float) -> Unit,
    private val onLoading: (Boolean) -> Unit,
    private val onFailed: () -> Unit,
) {
    private val host = URI(url).host
    private var engine: WebEngine? = null

    fun panel(): JFXPanel {
        // Creating the panel starts JavaFX; keep it running when this window closes.
        val panel = JFXPanel()
        Platform.setImplicitExit(false)
        Platform.runLater { panel.scene = scene() }
        return panel
    }

    /** Stops the page (its scripts and timers) once the window is gone. */
    fun close() {
        Platform.runLater { engine?.load(null) }
    }

    private fun scene(): Scene {
        val fill = javafx.scene.paint.Color.rgb(
            (background.red * 255).toInt(), (background.green * 255).toInt(), (background.blue * 255).toInt(),
        )
        // In dark mode the page stays hidden until its dark rules apply, so it does not flash light.
        val view = WebView().apply {
            pageFill = fill
            if (dark) opacity = 0.0
        }
        val engine = view.engine.also { engine = it }
        engine.loadWorker.progressProperty().addListener { _, _, p -> onProgress(p.toFloat().coerceIn(0f, 1f)) }
        engine.loadWorker.stateProperty().addListener { _, _, state ->
            onLoading(state == Worker.State.SCHEDULED || state == Worker.State.RUNNING)
            when (state) {
                Worker.State.SUCCEEDED -> if (dark) {
                    engine.executeScript(DarkScheme)
                    view.opacity = 1.0
                }
                Worker.State.FAILED -> if (engine.location?.let(::isOwnPage) != false) onFailed()
                else -> {}
            }
        }
        engine.locationProperty().addListener { _, _, location ->
            if (location.isNullOrEmpty() || isOwnPage(location)) return@addListener
            if (cleanroomInstallerVersion(location) != null) onInstaller(location) else onExternal(location)
            // WebView cannot download, and other sites belong in the browser: stay on the page.
            Platform.runLater { engine.loadWorker.cancel() }
        }
        engine.setCreatePopupHandler {
            // A throwaway engine that only reports where the new tab was going.
            val popup = WebEngine()
            popup.locationProperty().addListener { _, _, location ->
                if (location.isNullOrEmpty()) return@addListener
                if (cleanroomInstallerVersion(location) != null) onInstaller(location) else onExternal(location)
                Platform.runLater { popup.load(null) }
            }
            popup
        }
        engine.load(url)
        return Scene(view, fill)
    }

    private fun isOwnPage(location: String): Boolean =
        runCatching { URI(location).host.equals(host, ignoreCase = true) }.getOrDefault(false)

    private companion object {
        /**
         * Applies the page's `prefers-color-scheme: dark` rules (and drops its
         * light-only ones): JavaFX's WebKit always reports a light scheme.
         */
        val DarkScheme = """
            (function () {
              function visit(rules) {
                for (const rule of rules) {
                  if (rule.media) {
                    const text = rule.media.mediaText;
                    if (/prefers-color-scheme:\s*dark/.test(text)) rule.media.mediaText = text.replace(/\(\s*prefers-color-scheme:\s*dark\s*\)/g, 'all');
                    else if (/prefers-color-scheme:\s*light/.test(text)) rule.media.mediaText = 'not all';
                  }
                  if (rule.cssRules) visit(rule.cssRules);
                }
              }
              for (const sheet of document.styleSheets) { try { visit(sheet.cssRules); } catch (e) {} }
              document.documentElement.style.colorScheme = 'dark';
            })();
        """.trimIndent()
    }
}
