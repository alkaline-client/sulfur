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
import androidx.compose.foundation.layout.Spacer
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
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.NativeLong
import com.sun.jna.Pointer
import com.sun.jna.ptr.IntByReference
import com.sun.jna.ptr.PointerByReference
import com.sun.jna.ptr.NativeLongByReference
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
import java.awt.Window
import java.awt.event.WindowAdapter
import java.awt.event.WindowEvent
import java.awt.event.WindowStateListener
import java.lang.reflect.Method

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

    /** True while the window manager moves or resizes the window (X11), so pointer moves are left to it. */
    private var systemMoving = false

    // Dragging by the title bar. Where the window manager can do it (X11), it
    // moves the window itself, in step with the pointer as with a native title
    // bar; otherwise the window follows the pointer, whose position comes from
    // AWT in screen coordinates, so the window moving under it does not feed back.
    private var dragStart: Point? = null
    private var windowStart: Point? = null

    internal fun startDrag() {
        if (X11MoveResize.start(window, X11MoveResize.MOVE)) {
            systemMoving = true
            return
        }
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
        if (systemMoving) return
        val start = dragStart ?: return
        val origin = windowStart ?: return
        val pointer = MouseInfo.getPointerInfo()?.location ?: return
        window.setLocation(origin.x + pointer.x - start.x, origin.y + pointer.y - start.y)
    }

    internal fun endDrag() {
        systemMoving = false
        dragStart = null
        windowStart = null
    }

    // Resizing from the edges of an undecorated window, by the window manager where it can.
    private var resizeStart: Point? = null
    private var boundsStart: Rectangle? = null

    internal fun startResize(edge: Edge) {
        if (X11MoveResize.start(window, edge.x11Direction)) {
            systemMoving = true
            return
        }
        resizeStart = MouseInfo.getPointerInfo()?.location
        boundsStart = window.bounds
    }

    internal fun resize(edge: Edge) {
        if (systemMoving) return
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

    /** [x11Direction]: the edge's `_NET_WM_MOVERESIZE` direction. */
    enum class Edge(
        val x11Direction: Int,
        val left: Boolean = false,
        val top: Boolean = false,
        val right: Boolean = false,
        val bottom: Boolean = false,
    ) {
        Left(7, left = true), Top(1, top = true), Right(3, right = true), Bottom(5, bottom = true),
        TopLeft(0, left = true, top = true), TopRight(2, top = true, right = true),
        BottomLeft(6, left = true, bottom = true), BottomRight(4, right = true, bottom = true),
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
                        onDragStart = { chrome.startResize(edge) },
                        onDrag = { change, _ ->
                            change.consume()
                            chrome.resize(edge)
                        },
                        onDragEnd = { chrome.endDrag() },
                        onDragCancel = { chrome.endDrag() },
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

/**
 * [content] over the window's drag area. The area lies behind the content, so
 * only spots without a control of their own move the window: a press on a
 * button that slips a few pixels, or two quick clicks on page entries, stay
 * clicks instead of moving or maximizing the window.
 */
@Composable
fun WindowTitleBar(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val chrome = LocalWindowChrome.current
    Box(modifier) {
        Spacer(Modifier.matchParentSize().windowTitleBar(chrome))
        content()
    }
}

/** Drags the window by this element and maximizes or restores it on a double click. */
private fun Modifier.windowTitleBar(chrome: WindowChrome?): Modifier =
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
    WindowTitleBar(modifier.fillMaxWidth().height(40.dp)) {
        Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.End) {
            WindowButtons()
        }
    }
}

/**
 * `_NET_WM_MOVERESIZE` for X11 (and XWayland): the window manager moves or
 * resizes the window itself, following the pointer at the display's pace, as
 * it does for native title bars. Reaches into AWT's X11 toolkit for its
 * display connection and the frame's X window, which needs
 * `--add-exports java.desktop/sun.awt=ALL-UNNAMED` and `.../sun.awt.X11=ALL-UNNAMED`
 * (set in the app's JVM arguments). Anything missing makes [start] return
 * false and the caller moves the window itself.
 */
private object X11MoveResize {
    const val MOVE = 8

    private interface Xlib : Library {
        fun XInternAtom(display: Pointer, name: String, onlyIfExists: Boolean): NativeLong
        fun XDefaultRootWindow(display: Pointer): NativeLong
        fun XUngrabPointer(display: Pointer, time: NativeLong): Int
        fun XSendEvent(display: Pointer, window: NativeLong, propagate: Boolean, mask: NativeLong, event: Pointer): Int
        fun XFlush(display: Pointer): Int
        fun XGetWindowProperty(
            display: Pointer, window: NativeLong, property: NativeLong, offset: NativeLong, length: NativeLong,
            delete: Boolean, type: NativeLong, actualType: NativeLongByReference, actualFormat: IntByReference,
            items: NativeLongByReference, bytesAfter: NativeLongByReference, data: PointerByReference,
        ): Int
        fun XFree(data: Pointer): Int
    }

    private class AwtX11(val xlib: Xlib, val display: Pointer, val lock: Method, val unlock: Method, val windowOf: (Window) -> Long)

    /** AWT's X11 toolkit, or null on other platforms or without access to it. */
    private val toolkit: AwtX11? by lazy {
        runCatching {
            if (java.awt.Toolkit.getDefaultToolkit().javaClass.name != "sun.awt.X11.XToolkit" || Native.LONG_SIZE != 8) return@lazy null
            val sunToolkit = Class.forName("sun.awt.SunToolkit")
            val display = Class.forName("sun.awt.X11.XToolkit").getMethod("getDisplay").invoke(null) as Long
            val accessor = Class.forName("sun.awt.AWTAccessor").getMethod("getComponentAccessor").invoke(null)
            val getPeer = Class.forName("sun.awt.AWTAccessor\$ComponentAccessor").getMethod("getPeer", java.awt.Component::class.java)
            val getWindow = Class.forName("sun.awt.X11.XBaseWindow").getMethod("getWindow")
            AwtX11(
                xlib = Native.load("X11", Xlib::class.java),
                display = Pointer(display),
                lock = sunToolkit.getMethod("awtLock"),
                unlock = sunToolkit.getMethod("awtUnlock"),
                // The frame's own X window (its "shell"), the one the window manager manages.
                windowOf = { window -> getWindow.invoke(getPeer.invoke(accessor, window)) as Long },
            )
        }.getOrNull()
    }

    /** Whether the window manager announces `_NET_WM_MOVERESIZE`; checked once. */
    private var supported: Boolean? = null

    /** Asks the window manager to move ([MOVE]) or resize (an edge's direction) [window] with the pointer. */
    fun start(window: Window, direction: Int): Boolean {
        if (supported == false) return false
        val tk = toolkit ?: return false
        return runCatching {
            val scale = window.graphicsConfiguration.defaultTransform.scaleX
            val pointer = MouseInfo.getPointerInfo()?.location ?: return false
            val shell = tk.windowOf(window).takeIf { it != 0L } ?: return false
            tk.lock.invoke(null)
            try {
                val x = tk.xlib
                val moveResize = x.XInternAtom(tk.display, "_NET_WM_MOVERESIZE", false)
                val root = x.XDefaultRootWindow(tk.display)
                if (supported == null) supported = announced(tk, root, moveResize)
                if (supported != true) return false
                // The press that started the drag holds an implicit pointer grab; the
                // window manager can only take the pointer once it is released.
                x.XUngrabPointer(tk.display, NativeLong(0))
                // An XClientMessageEvent in an XEvent-sized buffer (64-bit layout).
                val event = Memory(192).apply {
                    clear()
                    setInt(0, 33) // ClientMessage
                    setInt(16, 1) // send_event
                    setPointer(24, tk.display)
                    setLong(32, shell)
                    setLong(40, moveResize.toLong())
                    setInt(48, 32) // format
                    setLong(56, (pointer.x * scale).toLong())
                    setLong(64, (pointer.y * scale).toLong())
                    setLong(72, direction.toLong())
                    setLong(80, 1) // left button
                    setLong(88, 1) // source: an application
                }
                // SubstructureRedirectMask | SubstructureNotifyMask
                x.XSendEvent(tk.display, root, false, NativeLong((1L shl 20) or (1L shl 19)), event)
                x.XFlush(tk.display)
                true
            } finally {
                tk.unlock.invoke(null)
            }
        }.getOrDefault(false)
    }

    /** Whether `_NET_SUPPORTED` on the root window lists [atom]. Called under the AWT lock. */
    private fun announced(tk: AwtX11, root: NativeLong, atom: NativeLong): Boolean {
        val x = tk.xlib
        val type = NativeLongByReference()
        val format = IntByReference()
        val count = NativeLongByReference()
        val after = NativeLongByReference()
        val data = PointerByReference()
        val status = x.XGetWindowProperty(
            tk.display, root, x.XInternAtom(tk.display, "_NET_SUPPORTED", false), NativeLong(0), NativeLong(4096),
            false, NativeLong(4) /* XA_ATOM */, type, format, count, after, data,
        )
        val items = data.value ?: return false
        try {
            if (status != 0 || format.value != 32) return false
            // Format-32 properties come back as an array of C longs.
            return items.getLongArray(0, count.value.toInt()).any { it == atom.toLong() }
        } finally {
            x.XFree(items)
        }
    }
}
