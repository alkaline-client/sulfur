package dev.sulfur.feature.download

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.SwingPanel
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
import javafx.embed.swing.JFXPanel
import javafx.scene.Scene
import javafx.scene.web.WebEngine
import javafx.scene.web.WebView
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.stringResource
import javax.swing.JComponent
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
        Surface(color = MaterialTheme.colorScheme.surface) {
            Column(Modifier.fillMaxSize()) {
                var failed by remember { mutableStateOf(false) }
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
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    if (!failed) {
                        SwingPanel(
                            factory = {
                                runCatching { cleanroomWebPanel(url) { link -> SwingUtilities.invokeLater { picked(link) } } }
                                    .getOrElse {
                                        failed = true
                                        javax.swing.JPanel()
                                    }
                            },
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
 * in a new tab on the page) go to [onInstaller]; other new-tab links open in
 * the system browser.
 */
fun cleanroomWebPanel(url: String, onInstaller: (String) -> Unit): JComponent {
    // Creating the panel starts JavaFX; keep it running when this window closes.
    val panel = JFXPanel()
    Platform.setImplicitExit(false)
    Platform.runLater {
        val view = WebView()
        val engine = view.engine
        engine.locationProperty().addListener { _, _, location ->
            if (location != null && cleanroomInstallerVersion(location) != null) {
                onInstaller(location)
                // WebView cannot download; stay on the page.
                Platform.runLater { engine.loadWorker.cancel() }
            }
        }
        engine.setCreatePopupHandler {
            // A throwaway engine that only reports where the new tab was going.
            val popup = WebEngine()
            popup.locationProperty().addListener { _, _, location ->
                if (location.isNullOrEmpty()) return@addListener
                if (cleanroomInstallerVersion(location) != null) onInstaller(location)
                else java.awt.Desktop.getDesktop().takeIf { java.awt.Desktop.isDesktopSupported() }
                    ?.runCatching { browse(java.net.URI(location)) }
                Platform.runLater { popup.load(null) }
            }
            popup
        }
        engine.load(url)
        panel.scene = Scene(view)
    }
    return panel
}
