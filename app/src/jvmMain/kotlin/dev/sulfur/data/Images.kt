package dev.sulfur.data

import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jetbrains.skia.FilterTileMode
import org.jetbrains.skia.Image
import org.jetbrains.skia.ImageFilter
import org.jetbrains.skia.Paint
import org.jetbrains.skia.Rect
import org.jetbrains.skia.SamplingMode
import org.jetbrains.skia.Surface
import java.io.File

/**
 * Remote images (update banners, mod icons): downloaded once by the core into
 * its cache folder, decoded here and kept in a small in-memory LRU.
 */
class ImageCache(private val repository: SulfurRepository, private val capacity: Int = 96) {
    private val decoded = object : LinkedHashMap<String, ImageBitmap>(capacity, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, ImageBitmap>?) = size > capacity
    }

    fun peek(url: String): ImageBitmap? = synchronized(decoded) { decoded[url] }

    /** The image, or null if it cannot be downloaded or decoded. */
    suspend fun load(url: String): ImageBitmap? {
        peek(url)?.let { return it }
        val image = runCatching {
            val path = repository.cachedImage(url)
            withContext(Dispatchers.IO) {
                Image.makeFromEncoded(File(path).readBytes()).toComposeImageBitmap()
            }
        }.getOrNull() ?: return null
        synchronized(decoded) { decoded[url] = image }
        return image
    }
}

val LocalImages = staticCompositionLocalOf<ImageCache?> { null }

/** Loads [url] through [LocalImages]; null while loading, on failure, or without a URL. */
@Composable
fun rememberRemoteImage(url: String?, images: ImageCache?): State<ImageBitmap?> =
    produceState(initialValue = url?.let { images?.peek(it) }, url, images) {
        if (url != null && images != null && value == null) value = images.load(url)
    }

/** Longest side of a decoded background picture; larger photos are scaled down to save memory. */
private const val BackgroundMaxSide = 1920

/**
 * The background picture at [path], scaled down and blurred once here rather
 * than on every frame. Null without a path or when the file cannot be read.
 */
@Composable
fun rememberBackgroundImage(path: String?, blurRadius: Float): State<ImageBitmap?> {
    val base by produceState<Image?>(initialValue = null, path) {
        value = path?.let { withContext(Dispatchers.IO) { runCatching { decodeScaled(File(it)) }.getOrNull() } }
    }
    return produceState<ImageBitmap?>(initialValue = null, base, blurRadius) {
        val image = base ?: run { value = null; return@produceState }
        value = withContext(Dispatchers.Default) { blurred(image, blurRadius).toComposeImageBitmap() }
    }
}

private fun decodeScaled(file: File): Image {
    val image = Image.makeFromEncoded(file.readBytes())
    val scale = BackgroundMaxSide.toFloat() / maxOf(image.width, image.height)
    if (scale >= 1f) return image
    val (w, h) = (image.width * scale).toInt().coerceAtLeast(1) to (image.height * scale).toInt().coerceAtLeast(1)
    val surface = Surface.makeRasterN32Premul(w, h)
    val source = Rect.makeWH(image.width.toFloat(), image.height.toFloat())
    surface.canvas.drawImageRect(image, source, Rect.makeWH(w.toFloat(), h.toFloat()), SamplingMode.LINEAR, null, true)
    image.close()
    return surface.makeImageSnapshot()
}

/** [image] blurred by [radius] (in pixels of a 1920-pixel picture, close to dp on a typical window). */
private fun blurred(image: Image, radius: Float): Image {
    if (radius <= 0.5f) return image
    val surface = Surface.makeRasterN32Premul(image.width, image.height)
    val sigma = radius / 2f
    val paint = Paint().apply { imageFilter = ImageFilter.makeBlur(sigma, sigma, FilterTileMode.CLAMP) }
    surface.canvas.drawImage(image, 0f, 0f, paint)
    return surface.makeImageSnapshot()
}
