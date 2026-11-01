package dev.sulfur.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import com.materialkolor.Contrast
import com.materialkolor.DynamicMaterialExpressiveTheme
import com.materialkolor.PaletteStyle
import com.materialkolor.dynamiccolor.ColorSpec
import dev.sulfur.ffi.CornerStyle
import dev.sulfur.ffi.ThemeMode
import dev.sulfur.resources.Res
import dev.sulfur.resources.google_sans_flex
import org.jetbrains.compose.resources.Font
import androidx.compose.ui.text.platform.Font as ClasspathFont

/** Seed colors offered in Settings; the first is Sulfur's own. */
val SeedColors: List<Long> = listOf(
    0xE8B53B, 0xF06B3C, 0xE5486F, 0xA05AE0, 0x5A6EE8, 0x2F9BD8, 0x26A69A, 0x6BAA3A,
)

fun seedColor(rgb: Long): Color = Color(0xFF000000 or (rgb and 0xFFFFFF))

/**
 * Material 3 Expressive theme: a dynamic color scheme generated from the seed
 * (2025 spec, Expressive palette style), the expressive motion scheme and the
 * Google Sans Flex type scale. Color changes animate. In Chinese and Japanese
 * the type scale uses MiSans instead (when the build embedded it), whose Latin
 * letters match its Han and kana. Compose desktop asks the system's fonts first
 * for characters the main font lacks, so mixed text elsewhere keeps the system's
 * CJK font.
 */
@Composable
fun SulfurTheme(
    theme: ThemeMode,
    seed: Long,
    highContrast: Boolean,
    corners: CornerStyle,
    content: @Composable () -> Unit,
) {
    val dark = when (theme) {
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
    }
    DynamicMaterialExpressiveTheme(
        seedColor = seedColor(seed),
        motionScheme = MotionScheme.expressive(),
        isDark = dark,
        style = PaletteStyle.Expressive,
        contrastLevel = if (highContrast) Contrast.High.value else Contrast.Default.value,
        specVersion = ColorSpec.SpecVersion.SPEC_2025,
        typography = sulfurTypography(),
        shapes = sulfurShapes(corners),
        animate = true,
    ) {
        CompositionLocalProvider(LocalCornerStyle provides corners, content = content)
    }
}

/** The corner roundness picked in Settings → Appearance. */
val LocalCornerStyle = staticCompositionLocalOf { CornerStyle.DEFAULT }

/** How far [style] scales Material's corner radii. */
fun cornerScale(style: CornerStyle): Float = when (style) {
    CornerStyle.SQUARE -> 0f
    CornerStyle.SMALL -> 0.5f
    CornerStyle.DEFAULT -> 1f
    CornerStyle.ROUND -> 1.5f
}

/** Material's shape scale with every radius scaled by [style]. */
private fun sulfurShapes(style: CornerStyle): Shapes {
    val scale = cornerScale(style)
    if (scale == 1f) return Shapes()
    fun r(dp: Int) = RoundedCornerShape((dp * scale).dp)
    return Shapes(
        extraSmall = r(4),
        small = r(8),
        medium = r(12),
        large = r(16),
        extraLarge = r(28),
        largeIncreased = r(20),
        extraLargeIncreased = r(32),
        extraExtraLarge = r(48),
    )
}

/**
 * The one place that says which corner each kind of surface has, so every
 * page rounds alike and the Corners setting reaches all of them. Pills and
 * circles (buttons, chips, search fields) stay as Material draws them.
 */
object Corners {
    /** A page, and what floats over one: dialogs, sheets, opened cards. */
    val page: Shape @Composable @ReadOnlyComposable get() = MaterialTheme.shapes.extraLarge

    /** A card or banner within a page. */
    val card: Shape @Composable @ReadOnlyComposable get() = MaterialTheme.shapes.large

    /** A row of a list, a thumbnail, a box within a card. */
    val item: Shape @Composable @ReadOnlyComposable get() = MaterialTheme.shapes.medium

    /** A small picture or code block within a row. */
    val inset: Shape @Composable @ReadOnlyComposable get() = MaterialTheme.shapes.small

    /** A product logo of [size], rounded like an app icon. */
    @Composable
    @ReadOnlyComposable
    fun logo(size: Dp): Shape = RoundedCornerShape(size / 5 * cornerScale(LocalCornerStyle.current))
}

/** Weights the type scale uses. */
private val TypeWeights = listOf(400, 500, 600, 700, 800)

/** Embedded by the `fetchCjkFont` Gradle task; absent in builds made without it. */
private const val CjkFontResource = "fonts/MiSansVF.ttf"

private val cjkFontFamily: FontFamily? by lazy {
    if (Thread.currentThread().contextClassLoader?.getResource(CjkFontResource) == null) null
    else FontFamily(
        TypeWeights.map { weight ->
            ClasspathFont(
                CjkFontResource,
                weight = FontWeight(weight),
                variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
            )
        }
    )
}

/** Whether the UI language is set in MiSans (see [LocalizedApp]). */
private fun usesCjkFont(): Boolean = java.util.Locale.getDefault().language in setOf("zh", "ja")

@Composable
private fun sulfurFontFamily(): FontFamily = cjkFontFamily?.takeIf { usesCjkFont() } ?: FontFamily(
    TypeWeights.map { weight ->
        Font(
            Res.font.google_sans_flex,
            weight = FontWeight(weight),
            variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
        )
    }
)

@Composable
private fun sulfurTypography(): Typography {
    val family = sulfurFontFamily()
    val base = Typography()
    fun TextStyle.f() = copy(fontFamily = family)
    return base.copy(
        displayLarge = base.displayLarge.f(),
        displayMedium = base.displayMedium.f(),
        displaySmall = base.displaySmall.f(),
        headlineLarge = base.headlineLarge.f(),
        headlineMedium = base.headlineMedium.f(),
        headlineSmall = base.headlineSmall.f(),
        titleLarge = base.titleLarge.f(),
        titleMedium = base.titleMedium.f(),
        titleSmall = base.titleSmall.f(),
        bodyLarge = base.bodyLarge.f(),
        bodyMedium = base.bodyMedium.f(),
        bodySmall = base.bodySmall.f(),
        labelLarge = base.labelLarge.f(),
        labelMedium = base.labelMedium.f(),
        labelSmall = base.labelSmall.f(),
        displayLargeEmphasized = base.displayLargeEmphasized.f(),
        displayMediumEmphasized = base.displayMediumEmphasized.f(),
        displaySmallEmphasized = base.displaySmallEmphasized.f(),
        headlineLargeEmphasized = base.headlineLargeEmphasized.f(),
        headlineMediumEmphasized = base.headlineMediumEmphasized.f(),
        headlineSmallEmphasized = base.headlineSmallEmphasized.f(),
        titleLargeEmphasized = base.titleLargeEmphasized.f(),
        titleMediumEmphasized = base.titleMediumEmphasized.f(),
        titleSmallEmphasized = base.titleSmallEmphasized.f(),
        bodyLargeEmphasized = base.bodyLargeEmphasized.f(),
        bodyMediumEmphasized = base.bodyMediumEmphasized.f(),
        bodySmallEmphasized = base.bodySmallEmphasized.f(),
        labelLargeEmphasized = base.labelLargeEmphasized.f(),
        labelMediumEmphasized = base.labelMediumEmphasized.f(),
        labelSmallEmphasized = base.labelSmallEmphasized.f(),
    )
}
