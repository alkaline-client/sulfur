package dev.sulfur.ui

import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.awt.ComposeWindow
import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.ptr.IntByReference
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_close
import dev.sulfur.resources.ic_close
import dev.sulfur.resources.ic_window_maximize
import dev.sulfur.resources.ic_window_minimize
import dev.sulfur.resources.ic_window_restore
import dev.sulfur.resources.window_maximize
import dev.sulfur.resources.window_minimize
import dev.sulfur.resources.window_restore
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.awt.Cursor
import java.awt.Frame
import java.awt.MouseInfo
import java.awt.Point
import java.awt.Rectangle
import java.awt.Toolkit
import java.awt.event.WindowAdapter
import java.awt.event.WindowEvent
import java.awt.event.WindowStateListener

private val hostOs = System.getProperty("os.name").lowercase()
val IsMac = hostOs.startsWith("mac")
private val IsWindows = hostOs.startsWith("windows")

/**
 * The window frame Sulfur draws itself. On Windows and Linux the window is
 * undecorated and the app draws the title bar, its buttons and the resize
 * edges; on macOS the system keeps its traffic lights over a transparent title
 * bar and the app only leaves room for them.
 */
class WindowChrome(private val window: ComposeWindow, private val onClose: () -> Unit) {
    /** True on macOS, where the system draws the window buttons. */
    val nativeButtons = IsMac

    var maximized by mutableStateOf(false)
        private set

    /** Bounds to go back to after a maximize that Sulfur did itself (Windows). */
    private var restoreBounds: Rectangle? = null

    private val stateListener = WindowStateListener { event ->
        if (restoreBounds == null) maximized = event.newState and Frame.MAXIMIZED_BOTH == Frame.MAXIMIZED_BOTH
    }

    init {
        if (IsMac) {
            window.rootPane.putClientProperty("apple.awt.fullWindowContent", true)
            window.rootPane.putClientProperty("apple.awt.transparentTitleBar", true)
            window.rootPane.putClientProperty("apple.awt.windowTitleVisible", false)
        }
        if (IsWindows) {
            // The native handle exists once the window is shown.
            if (window.isDisplayable) roundWindowsCorners(window)
            else window.addWindowListener(object : WindowAdapter() {
                override fun windowOpened(e: WindowEvent) = roundWindowsCorners(window)
            })
        }
        window.addWindowStateListener(stateListener)
    }

    fun minimize() {
        window.extendedState = window.extendedState or Frame.ICONIFIED
    }

    fun close() = onClose()

    fun toggleMaximize() = if (maximized) restore() else maximize()

    private fun maximize() {
        if (IsWindows) {
            // An undecorated window maximized by the system covers the taskbar,
            // so fill the screen's work area instead.
            restoreBounds = window.bounds
            window.bounds = workArea()
            maximized = true
        } else {
            window.extendedState = window.extendedState or Frame.MAXIMIZED_BOTH
        }
    }

    private fun restore() {
        val previous = restoreBounds
        if (previous != null) {
            restoreBounds = null
            window.bounds = previous
            maximized = false
        } else {
            window.extendedState = window.extendedState and Frame.MAXIMIZED_BOTH.inv()
        }
    }

    private fun workArea(): Rectangle {
        val config = window.graphicsConfiguration
        val screen = config.bounds
        val insets = Toolkit.getDefaultToolkit().getScreenInsets(config)
        return Rectangle(
            screen.x + insets.left,
            screen.y + insets.top,
            screen.width - insets.left - insets.right,
            screen.height - insets.top - insets.bottom,
        )
    }

    // Dragging by the title bar. The pointer position comes from AWT in screen
    // coordinates, so the window moving under the pointer does not feed back.
    private var dragStart: Point? = null
    private var windowStart: Point? = null

    internal fun startDrag() {
        val pointer = MouseInfo.getPointerInfo()?.location ?: return
        if (maximized) {
            // Leave the maximized state with the pointer at the same relative spot of the title bar.
            val ratio = (pointer.x - window.x).toDouble() / window.width.coerceAtLeast(1)
            restore()
            window.setLocation((pointer.x - window.width * ratio).toInt(), pointer.y - TitleGrab)
        }
        dragStart = pointer
        windowStart = window.location
    }

    internal fun drag() {
        val start = dragStart ?: return
        val origin = windowStart ?: return
        val pointer = MouseInfo.getPointerInfo()?.location ?: return
        window.setLocation(origin.x + pointer.x - start.x, origin.y + pointer.y - start.y)
    }

    internal fun endDrag() {
        dragStart = null
        windowStart = null
    }

    // Resizing from the edges of an undecorated window.
    private var resizeStart: Point? = null
    private var boundsStart: Rectangle? = null

    internal fun startResize() {
        resizeStart = MouseInfo.getPointerInfo()?.location
        boundsStart = window.bounds
    }

    internal fun resize(edge: Edge) {
        val start = resizeStart ?: return
        val from = boundsStart ?: return
        val pointer = MouseInfo.getPointerInfo()?.location ?: return
        val (dx, dy) = pointer.x - start.x to pointer.y - start.y
        val min = window.minimumSize
        var (x, y, w, h) = listOf(from.x, from.y, from.width, from.height)
        if (edge.left) {
            w = (from.width - dx).coerceAtLeast(min.width)
            x = from.x + from.width - w
        }
        if (edge.right) w = (from.width + dx).coerceAtLeast(min.width)
        if (edge.top) {
            h = (from.height - dy).coerceAtLeast(min.height)
            y = from.y + from.height - h
        }
        if (edge.bottom) h = (from.height + dy).coerceAtLeast(min.height)
        window.setBounds(x, y, w, h)
    }

    internal fun dispose() = window.removeWindowStateListener(stateListener)

    enum class Edge(val left: Boolean = false, val top: Boolean = false, val right: Boolean = false, val bottom: Boolean = false) {
        Left(left = true), Top(top = true), Right(right = true), Bottom(bottom = true),
        TopLeft(left = true, top = true), TopRight(top = true, right = true),
        BottomLeft(left = true, bottom = true), BottomRight(right = true, bottom = true),
    }

    private companion object {
        /** Where the pointer lands in a title bar restored by dragging, from its top. */
        const val TitleGrab = 24
    }
}

/** The window's chrome; null when the app is not in a real window (e.g. screenshots). */
val LocalWindowChrome = staticCompositionLocalOf<WindowChrome?> { null }

/**
 * The app's content inside the frame Sulfur draws: edges to resize the window
 * and a hairline border while it is not maximized.
 */
@Composable
fun WindowFrame(window: ComposeWindow, onClose: () -> Unit, content: @Composable () -> Unit) {
    val chrome = remember(window) { WindowChrome(window, onClose) }
    DisposableEffect(chrome) { onDispose { chrome.dispose() } }
    CompositionLocalProvider(LocalWindowChrome provides chrome) {
        Box(Modifier.fillMaxSize()) {
            content()
            if (!chrome.nativeButtons && !chrome.maximized) {
                Box(Modifier.fillMaxSize().border(1.dp, MaterialTheme.colorScheme.outlineVariant))
                ResizeEdges(chrome)
            }
        }
    }
}

@Composable
private fun BoxScope.ResizeEdges(chrome: WindowChrome) {
    @Composable
    fun Handle(edge: WindowChrome.Edge, alignment: Alignment, modifier: Modifier, cursor: Int) {
        Box(
            modifier
                .align(alignment)
                .pointerHoverIcon(PointerIcon(Cursor(cursor)))
                .pointerInput(edge) {
                    detectDragGestures(
                        onDragStart = { chrome.startResize() },
                        onDrag = { change, _ ->
                            change.consume()
                            chrome.resize(edge)
                        },
                    )
                },
        )
    }
    val e = ResizeEdge
    val c = ResizeCorner
    Handle(WindowChrome.Edge.Left, Alignment.CenterStart, Modifier.width(e).fillMaxHeight(), Cursor.W_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.Right, Alignment.CenterEnd, Modifier.width(e).fillMaxHeight(), Cursor.E_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.Top, Alignment.TopCenter, Modifier.height(e).fillMaxWidth(), Cursor.N_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.Bottom, Alignment.BottomCenter, Modifier.height(e).fillMaxWidth(), Cursor.S_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.TopLeft, Alignment.TopStart, Modifier.size(c), Cursor.NW_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.TopRight, Alignment.TopEnd, Modifier.size(c), Cursor.NE_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.BottomLeft, Alignment.BottomStart, Modifier.size(c), Cursor.SW_RESIZE_CURSOR)
    Handle(WindowChrome.Edge.BottomRight, Alignment.BottomEnd, Modifier.size(c), Cursor.SE_RESIZE_CURSOR)
}

private val ResizeEdge = 5.dp
private val ResizeCorner = 12.dp

/** Room the macOS traffic lights take at the start of the title bar. */
val TrafficLightsWidth = 76.dp

/** Height the macOS traffic lights take at the top of the window. */
val TrafficLightsHeight = 28.dp

/** Drags the window by this element and maximizes or restores it on a double click. */
fun Modifier.windowTitleBar(chrome: WindowChrome?): Modifier =
    if (chrome == null) this
    else this
        .pointerInput(chrome) { detectTapGestures(onDoubleTap = { chrome.toggleMaximize() }) }
        .pointerInput(chrome) {
            detectDragGestures(
                onDragStart = { chrome.startDrag() },
                onDrag = { change, _ ->
                    change.consume()
                    chrome.drag()
                },
                onDragEnd = { chrome.endDrag() },
                onDragCancel = { chrome.endDrag() },
            )
        }

/**
 * Minimize, maximize and close, at the end of the title bar. Nothing on macOS,
 * where the system's traffic lights sit at the start instead.
 */
@Composable
fun WindowButtons(modifier: Modifier = Modifier) {
    val chrome = LocalWindowChrome.current
    if (chrome?.nativeButtons == true) return
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
        WindowButton(
            painterResource(Res.drawable.ic_window_minimize),
            stringResource(Res.string.window_minimize),
            onClick = { chrome?.minimize() },
        )
        val maximized = chrome?.maximized == true
        WindowButton(
            painterResource(if (maximized) Res.drawable.ic_window_restore else Res.drawable.ic_window_maximize),
            stringResource(if (maximized) Res.string.window_restore else Res.string.window_maximize),
            onClick = { chrome?.toggleMaximize() },
        )
        WindowButton(
            painterResource(Res.drawable.ic_close),
            stringResource(Res.string.action_close),
            danger = true,
            onClick = { chrome?.close() },
        )
    }
}

@Composable
private fun WindowButton(
    icon: Painter,
    description: String,
    onClick: () -> Unit,
    danger: Boolean = false,
    size: Dp = 40.dp,
) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val colors = if (danger && hovered) {
        IconButtonDefaults.iconButtonColors(
            containerColor = MaterialTheme.colorScheme.error,
            contentColor = MaterialTheme.colorScheme.onError,
        )
    } else {
        IconButtonDefaults.iconButtonColors(contentColor = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    IconButton(
        onClick = onClick,
        colors = colors,
        shapes = IconButtonDefaults.shapes(),
        interactionSource = interaction,
        modifier = Modifier.size(size).hoverable(interaction),
    ) {
        Icon(icon, description, Modifier.size(18.dp))
    }
}

/** Rounded corners for the undecorated window on Windows 11 (ignored on Windows 10). */
private fun roundWindowsCorners(window: ComposeWindow) {
    runCatching {
        val hwnd = Native.getComponentPointer(window) ?: return
        val dwm = Native.load("dwmapi", Dwm::class.java)
        dwm.DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, IntByReference(DWMWCP_ROUND), 4)
    }
}

@Suppress("FunctionName")
private interface Dwm : Library {
    fun DwmSetWindowAttribute(hwnd: Pointer, attribute: Int, value: IntByReference, size: Int): Int
}

private const val DWMWA_WINDOW_CORNER_PREFERENCE = 33
private const val DWMWCP_ROUND = 2

/** A thin title bar for layouts without a top bar: drag area and window buttons. */
@Composable
fun TitleStrip(modifier: Modifier = Modifier) {
    val chrome = LocalWindowChrome.current
    Row(
        modifier.fillMaxWidth().height(40.dp).windowTitleBar(chrome),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.End,
    ) {
        WindowButtons()
    }
}
