package dev.sulfur.data

import androidx.compose.ui.graphics.toComposeImageBitmap
import com.materialkolor.quantize.QuantizerCelebi
import com.materialkolor.score.Score
import dev.sulfur.ffi.ColorSource
import dev.sulfur.ffi.DesktopWallpaper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jetbrains.skia.Image
import org.jetbrains.skia.Rect
import org.jetbrains.skia.SamplingMode
import org.jetbrains.skia.Surface
import java.io.File

/**
 * The theme's seed color: the one picked in Settings, or one sampled from the
 * desktop wallpaper or the background picture with Material's quantizer and
 * scoring (what Android does for wallpaper colors). The wallpaper is looked
 * up again when the window gains focus, so a new one recolors the app.
 */
class ThemeColors(private val repository: SulfurRepository, private val scope: CoroutineScope) {
    private val _wallpaper = MutableStateFlow<DesktopWallpaper?>(null)
    /** The wallpaper found by the last lookup. */
    val wallpaper: StateFlow<DesktopWallpaper?> = _wallpaper.asStateFlow()

    private val wallpaperSeed = MutableStateFlow<Long?>(null)
    private val backgroundSeed = MutableStateFlow<Long?>(null)
    private var lastLookup = 0L

    /** The color sampled for each picture source, null until known or when nothing usable was found. */
    val sampled: StateFlow<Map<ColorSource, Long?>> = combine(wallpaperSeed, backgroundSeed) { w, b ->
        mapOf(ColorSource.WALLPAPER to w, ColorSource.BACKGROUND to b)
    }.stateIn(scope, SharingStarted.Eagerly, emptyMap())

    /** The seed to theme with; the picked color until a sample is ready. */
    val seed: StateFlow<Long> = combine(repository.settings, wallpaperSeed, backgroundSeed) { s, w, b ->
        when (s.colorSource) {
            ColorSource.SEED -> null
            ColorSource.WALLPAPER -> w
            ColorSource.BACKGROUND -> b
        } ?: s.seedColor.toLong()
    }.stateIn(scope, SharingStarted.Eagerly, repository.settings.value.seedColor.toLong())

    init {
        scope.launch {
            repository.settings.map { it.colorSource to it.backgroundImage }.distinctUntilChanged().collectLatest { (source, picture) ->
                when (source) {
                    ColorSource.BACKGROUND -> backgroundSeed.value = picture?.let { sample(it) }
                    ColorSource.WALLPAPER -> refreshWallpaper(force = true)
                    ColorSource.SEED -> {}
                }
            }
        }
    }

    /** Looks the wallpaper up again (at most every few seconds unless [force]d) when it is the color source. */
    fun refreshWallpaper(force: Boolean = false) {
        if (repository.settings.value.colorSource != ColorSource.WALLPAPER) return
        val now = System.currentTimeMillis()
        if (!force && now - lastLookup < 5_000) return
        lastLookup = now
        scope.launch {
            val found = runCatching { repository.desktopWallpaper() }.getOrNull()
            _wallpaper.value = found
            wallpaperSeed.value = found?.color?.toLong() ?: found?.path?.let { sample(it) }
        }
    }

    private suspend fun sample(path: String): Long? = withContext(Dispatchers.Default) {
        runCatching { dominantColor(File(path)) }.getOrNull()
    }
}

/** Longest side the picture is scaled to before quantizing; plenty for picking a color. */
private const val SampleSide = 128

/** The best theme color of a picture, as `0xRRGGBB`. */
private fun dominantColor(file: File): Long? {
    val image = Image.makeFromEncoded(file.readBytes())
    val scale = minOf(1f, SampleSide.toFloat() / maxOf(image.width, image.height))
    val (w, h) = (image.width * scale).toInt().coerceAtLeast(1) to (image.height * scale).toInt().coerceAtLeast(1)
    val surface = Surface.makeRasterN32Premul(w, h)
    surface.canvas.drawImageRect(
        image,
        Rect.makeWH(image.width.toFloat(), image.height.toFloat()),
        Rect.makeWH(w.toFloat(), h.toFloat()),
        SamplingMode.LINEAR,
        null,
        true,
    )
    image.close()
    val pixels = IntArray(w * h)
    surface.makeImageSnapshot().toComposeImageBitmap().readPixels(pixels)
    surface.close()
    val population = QuantizerCelebi.quantize(pixels, 128)
    // Scoring prefers colorful, common colors; a grey picture falls back to its commonest color.
    val best = Score.score(population, 1, null, true).firstOrNull()
        ?: population.maxByOrNull { it.value }?.key
        ?: return null
    return best.toLong() and 0xFFFFFF
}
