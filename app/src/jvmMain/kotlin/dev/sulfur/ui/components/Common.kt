package dev.sulfur.ui.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.graphics.shapes.RoundedPolygon
import dev.sulfur.data.LocalImages
import dev.sulfur.data.rememberRemoteImage
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.VersionKind
import dev.sulfur.resources.Res
import dev.sulfur.resources.kind_alpha
import dev.sulfur.resources.kind_beta
import dev.sulfur.resources.kind_other
import dev.sulfur.resources.kind_release
import dev.sulfur.resources.kind_snapshot
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.text.CompactNumberFormat
import java.text.NumberFormat
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale

/** Horizontal padding of every page. */
val PagePadding = 20.dp

/** Gap between cards on a page. */
val CardSpacing = 12.dp

/** A titled group of content on a tonal surface. */
@Composable
fun SectionCard(
    title: String,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    trailing: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.heightIn(min = 32.dp)) {
                if (icon != null) {
                    ShapeIcon(icon, MaterialShapes.Cookie6Sided, size = 32.dp)
                    Spacer(Modifier.width(10.dp))
                }
                Text(
                    title,
                    style = MaterialTheme.typography.titleMediumEmphasized,
                    modifier = Modifier.weight(1f),
                )
                trailing()
            }
            content()
        }
    }
}

/** Page title with actions on the same line; actions wrap below on narrow windows. */
@Composable
fun PageHeader(
    title: String,
    modifier: Modifier = Modifier,
    actions: @Composable RowScope.() -> Unit = {},
) {
    Row(
        modifier.fillMaxWidth().heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(title, style = MaterialTheme.typography.headlineSmallEmphasized)
        Spacer(Modifier.weight(1f))
        actions()
    }
}

/**
 * Single choice as a connected button group. Buttons share the width when
 * [fill] is set; otherwise they size to their (possibly translated) labels and
 * wrap to a new line instead of truncating.
 */
@Composable
fun <T> ConnectedChoice(
    options: List<T>,
    selected: T,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    fill: Boolean = true,
    icon: ((T) -> DrawableResource)? = null,
) {
    @Composable
    fun Button(index: Int, option: T, buttonModifier: Modifier) {
        ToggleButton(
            checked = option == selected,
            onCheckedChange = { onSelect(option) },
            shapes = when (index) {
                0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
            },
            modifier = buttonModifier,
        ) {
            if (icon != null) {
                Icon(painterResource(icon(option)), null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
            }
            Text(label(option), maxLines = 1)
        }
    }
    if (fill) {
        Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
            options.forEachIndexed { index, option -> Button(index, option, Modifier.weight(1f)) }
        }
    } else {
        FlowRow(
            modifier,
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            options.forEachIndexed { index, option -> Button(index, option, Modifier) }
        }
    }
}

/** An icon inside one of the Material 3 Expressive shapes. */
@Composable
fun ShapeIcon(
    icon: Painter,
    shape: RoundedPolygon,
    modifier: Modifier = Modifier,
    size: Dp = 40.dp,
    container: Color = MaterialTheme.colorScheme.secondaryContainer,
    content: Color = MaterialTheme.colorScheme.onSecondaryContainer,
    rotating: Boolean = false,
) {
    // The angle is read only while drawing, so turning redraws the shape
    // without recomposing it every frame.
    val rotation = if (rotating) {
        rememberInfiniteTransition().animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(tween(12_000, easing = LinearEasing), RepeatMode.Restart),
        )
    } else null
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .matchParentSize()
                .graphicsLayer { rotationZ = rotation?.value ?: 0f }
                .clip(shape.toShape())
                .background(container)
        )
        Icon(icon, contentDescription = null, tint = content, modifier = Modifier.size(size * 0.55f))
    }
}

/**
 * A remote image cropped to fill its box, fading in once loaded. Without an
 * image (missing, offline, still loading) the box shows [fallback] on [placeholder].
 */
@Composable
fun RemoteImage(
    url: String?,
    modifier: Modifier = Modifier,
    placeholder: Color = MaterialTheme.colorScheme.surfaceContainerHighest,
    fallback: @Composable BoxScope.() -> Unit = {},
) {
    val image by rememberRemoteImage(url, LocalImages.current)
    val motion = MaterialTheme.motionScheme
    Box(modifier.background(placeholder), contentAlignment = Alignment.Center) {
        AnimatedContent(
            targetState = image,
            transitionSpec = { fadeIn(motion.slowEffectsSpec()) togetherWith fadeOut(motion.fastEffectsSpec()) },
            modifier = Modifier.matchParentSize(),
            contentAlignment = Alignment.Center,
            label = "image",
        ) { bitmap ->
            if (bitmap != null) {
                Image(bitmap, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            } else {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { fallback() }
            }
        }
    }
}

/** Shape used to mark a version's channel throughout the app. */
fun kindShape(kind: VersionKind): RoundedPolygon = when (kind) {
    VersionKind.RELEASE -> MaterialShapes.Cookie9Sided
    VersionKind.SNAPSHOT -> MaterialShapes.SoftBurst
    VersionKind.OLD_BETA -> MaterialShapes.Pill
    VersionKind.OLD_ALPHA -> MaterialShapes.Clover4Leaf
    VersionKind.OTHER -> MaterialShapes.Circle
}

@Composable
fun kindLabel(kind: VersionKind): String = stringResource(
    when (kind) {
        VersionKind.RELEASE -> Res.string.kind_release
        VersionKind.SNAPSHOT -> Res.string.kind_snapshot
        VersionKind.OLD_BETA -> Res.string.kind_beta
        VersionKind.OLD_ALPHA -> Res.string.kind_alpha
        VersionKind.OTHER -> Res.string.kind_other
    }
)

/** Product names, the same in every language. */
fun loaderName(kind: LoaderKind): String = when (kind) {
    LoaderKind.FABRIC -> "Fabric"
    LoaderKind.FORGE -> "Forge"
    LoaderKind.NEO_FORGE -> "NeoForge"
    LoaderKind.OPTI_FINE -> "OptiFine"
    LoaderKind.LEGACY_FABRIC -> "Legacy Fabric"
    LoaderKind.ORNITHE -> "Ornithe"
    LoaderKind.QUILT -> "Quilt"
    LoaderKind.RIFT -> "Rift"
    LoaderKind.CLEANROOM -> "Cleanroom"
}

fun formatBytes(bytes: Double): String {
    val units = listOf("B", "KiB", "MiB", "GiB")
    var value = bytes
    var unit = 0
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    return if (unit == 0) "${value.toInt()} ${units[unit]}" else String.format(Locale.getDefault(), "%.1f %s", value, units[unit])
}

fun formatBytes(bytes: ULong): String = formatBytes(bytes.toDouble())

/** Number formats by locale; the UI language can change while the app runs. Used on the UI thread only. */
private val compactCounts = HashMap<Locale, NumberFormat>()

/** `12345` → "12K" (or "1.2万" in Chinese), in the UI language. */
fun formatCount(count: ULong): String =
    compactCounts.getOrPut(Locale.getDefault()) {
        CompactNumberFormat.getCompactNumberInstance(Locale.getDefault(), NumberFormat.Style.SHORT).apply {
            maximumFractionDigits = 1
        }
    }.format(count.toLong())

/** An RFC 3339 timestamp as a date in the UI language. */
fun formatDate(timestamp: String): String = runCatching {
    OffsetDateTime.parse(timestamp).format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(Locale.getDefault()))
}.getOrDefault(timestamp.take(10))
