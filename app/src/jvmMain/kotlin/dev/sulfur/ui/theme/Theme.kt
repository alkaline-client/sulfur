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
import androidx.compose.runtime.remember
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
import androidx.compose.ui.platform.LocalFontFamilyResolver
import org.jetbrains.compose.resources.ExperimentalResourceApi
import org.jetbrains.skia.Data
import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontVariation as SkiaFontVariation
import org.jetbrains.skia.paragraph.FontCollection
import org.jetbrains.skia.paragraph.TypefaceFontProvider

/** Seed colors offered in Settings; the first is Sulfur's own. */
val SeedColors: List<Long> = listOf(
    0xE8B53B, 0xF06B3C, 0xE5486F, 0xA05AE0, 0x5A6EE8, 0x2F9BD8, 0x26A69A, 0x6BAA3A,
)

fun seedColor(rgb: Long): Color = Color(0xFF000000 or (rgb and 0xFFFFFF))

/**
 * Material 3 Expressive theme: a dynamic color scheme generated from the seed
 * (2025 spec, Expressive palette style), the expressive motion scheme and the
 * Google Sans Flex type scale, with MiSans (when the build embedded it) for
 * the characters Google Sans Flex lacks. Color changes animate.
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

/**
 * Material's shape scale with every radius scaled by [style]. Text fields,
 * menus and chips (extra small and small) round like the list rows they sit
 * among ([Corners.item]) rather than Material's near-square 4 and 8.
 */
private fun sulfurShapes(style: CornerStyle): Shapes {
    val scale = cornerScale(style)
    fun r(dp: Int) = RoundedCornerShape((dp * scale).dp)
    return Shapes(
        extraSmall = r(12),
        small = r(12),
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

/** The names the two bundled fonts are registered under (see [fallbackFontFamily]). */
private const val LatinFamilyName = "Sulfur Google Sans Flex"
private const val CjkFamilyName = "Sulfur MiSans"

@Composable
private fun googleSansFlex(): FontFamily = FontFamily(
    TypeWeights.map { weight ->
        Font(
            Res.font.google_sans_flex,
            weight = FontWeight(weight),
            variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
        )
    }
)

/**
 * Google Sans Flex for everything it has, MiSans for the characters it lacks
 * (Han, kana), the system's fonts after that. Compose gives the text engine a
 * list of fonts to try in order only for its generic families; a bundled
 * font family is one font, with the system's fonts behind it. So both fonts
 * are registered, at every weight of the type scale, with the text engine's
 * font collection, and put at the front of the sans-serif family's list.
 * Null when MiSans was not embedded, or when Compose's internals changed; the
 * type scale is then Google Sans Flex alone, with the system's CJK fonts
 * behind it.
 */
@Suppress("UNCHECKED_CAST")
@OptIn(ExperimentalResourceApi::class)
private fun fallbackFontFamily(resolver: FontFamily.Resolver): FontFamily? = runCatching {
    val loader = Thread.currentThread().contextClassLoader
    val cjk = loader?.getResource(CjkFontResource)?.readBytes() ?: return null
    val latin = java.net.URI(Res.getUri("font/google_sans_flex.ttf")).toURL().readBytes()
    // FontFamilyResolverImpl.platformFontLoader is internal; the adapter's backend (the Skia font loader) and
    // its font collection are public.
    val platformLoader = resolver.javaClass.getDeclaredField("platformFontLoader").apply { isAccessible = true }.get(resolver)
    val backend = platformLoader.javaClass.getMethod("getBackend").invoke(platformLoader)
    val fonts = backend.javaClass.getMethod("getFontCollection").invoke(backend) as FontCollection
    // The generic families' font lists (internal), by family name.
    val mapping = Class.forName("androidx.compose.ui.text.platform.PlatformFont_skikoKt")
        .getDeclaredField("GenericFontFamiliesMapping\$delegate").apply { isAccessible = true }
        .get(null).let { (it as Lazy<*>).value as MutableMap<String, List<String>> }
    val provider = TypefaceFontProvider()
    for ((bytes, name) in listOf(latin to LatinFamilyName, cjk to CjkFamilyName)) {
        val variable = FontMgr.default.makeFromData(Data.makeFromBytes(bytes)) ?: return null
        for (weight in TypeWeights) {
            provider.registerTypeface(variable.makeClone(arrayOf(SkiaFontVariation("wght", weight.toFloat()))), name)
        }
    }
    // Compose fills the default and asset font managers; the dynamic one is free.
    fonts.setDynamicFontManager(provider)
    val sansSerif = FontFamily.SansSerif.name
    mapping[sansSerif] = listOf(LatinFamilyName, CjkFamilyName) + mapping[sansSerif].orEmpty()
    fonts.paragraphCache.reset()
    FontFamily.SansSerif
}.onFailure { System.err.println("Sulfur: MiSans fallback unavailable: $it") }.getOrNull()

@Composable
private fun sulfurFontFamily(): FontFamily {
    val resolver = LocalFontFamilyResolver.current
    val fallback = remember(resolver) { fallbackFontFamily(resolver) }
    return fallback ?: googleSansFlex()
}

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
