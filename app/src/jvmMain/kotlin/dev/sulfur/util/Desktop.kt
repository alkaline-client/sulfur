package dev.sulfur.util

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.awt.Desktop
import java.awt.FileDialog
import java.awt.Frame
import java.io.File
import java.io.FilenameFilter
import java.net.URI
import javax.swing.JFileChooser

/** Opens a folder in the system file manager, creating it if needed. */
suspend fun openFolder(path: String): Boolean = withContext(Dispatchers.IO) {
    val dir = File(path).apply { mkdirs() }
    runCatching {
        if (Desktop.isDesktopSupported() && Desktop.getDesktop().isSupported(Desktop.Action.OPEN)) {
            Desktop.getDesktop().open(dir)
        } else {
            // Common on Linux desktops where AWT lacks GNOME integration.
            ProcessBuilder("xdg-open", dir.absolutePath).start()
        }
    }.isSuccess
}

suspend fun openUrl(url: String): Boolean = withContext(Dispatchers.IO) {
    runCatching {
        if (Desktop.isDesktopSupported() && Desktop.getDesktop().isSupported(Desktop.Action.BROWSE)) {
            Desktop.getDesktop().browse(URI(url))
        } else {
            ProcessBuilder("xdg-open", url).start()
        }
    }.isSuccess
}

/**
 * Asks for a file with the system's file dialog; returns its absolute path,
 * or null when cancelled. [extensions] (without dots) narrow the choice where
 * the platform supports it. Call on the UI thread; the dialog is modal.
 */
fun chooseFile(title: String, extensions: List<String>): String? {
    val dialog = FileDialog(null as Frame?, title, FileDialog.LOAD).apply {
        filenameFilter = FilenameFilter { _, name -> extensions.any { name.endsWith(".$it", ignoreCase = true) } }
        // Windows ignores the filter but honours a wildcard file name.
        if (System.getProperty("os.name").startsWith("Windows")) file = extensions.joinToString(";") { "*.$it" }
        isVisible = true
    }
    val name = dialog.file ?: return null
    return File(dialog.directory, name).absolutePath
}

/**
 * Asks for a folder; returns its absolute path, or null when cancelled.
 * AWT's native dialog cannot pick folders outside macOS, so this uses Swing's.
 */
fun chooseFolder(title: String): String? {
    val chooser = JFileChooser().apply {
        dialogTitle = title
        fileSelectionMode = JFileChooser.DIRECTORIES_ONLY
        isAcceptAllFileFilterUsed = false
    }
    return if (chooser.showOpenDialog(null) == JFileChooser.APPROVE_OPTION) chooser.selectedFile?.absolutePath else null
}
