package dev.sulfur.util

import java.awt.Image
import java.awt.Toolkit
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO

/**
 * How Linux docks and task bars recognise Sulfur. They find a window's name
 * and icon through a `.desktop` file whose `StartupWMClass` matches the
 * window's X11 class; Java names that class after a thread class
 * ("java-lang-Thread") and there is no `.desktop` file for a bare executable.
 */
private val IsLinux = System.getProperty("os.name").lowercase().startsWith("linux")

/** X11 class of Sulfur's windows, matched by `StartupWMClass` below. */
private const val WindowClass = "Sulfur"

/**
 * Names Sulfur's windows' X11 class. Must run before the first window opens;
 * needs `--add-opens=java.desktop/sun.awt.X11=ALL-UNNAMED` (set in the build).
 */
fun setWindowClass() {
    if (!IsLinux) return
    runCatching {
        val toolkit = Toolkit.getDefaultToolkit()
        val field = toolkit.javaClass.getDeclaredField("awtAppClassName")
        field.isAccessible = true
        field.set(toolkit, WindowClass)
    }
}

/**
 * When Sulfur runs as its single executable (the stub passes its path in
 * `SULFUR_EXECUTABLE`), adds it to the user's applications with [icon], so
 * docks show its name and logo and it can be started from the app menu.
 * Rewritten when the executable moves. Installed packages bring their own entry.
 */
fun installDesktopEntry(icon: Image) {
    if (!IsLinux) return
    val executable = System.getenv("SULFUR_EXECUTABLE")?.takeIf { File(it).isFile } ?: return
    val data = System.getenv("XDG_DATA_HOME")?.takeIf { it.isNotBlank() }?.let(::File)
        ?: File(System.getProperty("user.home"), ".local/share")
    val entry = File(data, "applications/sulfur.desktop")
    val iconFile = File(data, "icons/hicolor/256x256/apps/sulfur.png")
    val text = """
        [Desktop Entry]
        Type=Application
        Name=Sulfur
        Comment=A Minecraft launcher for everyone
        Exec="${executable.replace("\"", "\\\"")}"
        Icon=sulfur
        Terminal=false
        Categories=Game;
        StartupWMClass=$WindowClass
    """.trimIndent() + "\n"
    runCatching {
        if (!iconFile.isFile) {
            iconFile.parentFile.mkdirs()
            // Compose hands out a multi-resolution image; ImageIO needs plain pixels.
            val pixels = BufferedImage(256, 256, BufferedImage.TYPE_INT_ARGB)
            pixels.createGraphics().apply {
                drawImage(icon, 0, 0, 256, 256, null)
                dispose()
            }
            ImageIO.write(pixels, "png", iconFile)
        }
        if (!entry.isFile || entry.readText() != text) {
            entry.parentFile.mkdirs()
            entry.writeText(text)
        }
    }
}
