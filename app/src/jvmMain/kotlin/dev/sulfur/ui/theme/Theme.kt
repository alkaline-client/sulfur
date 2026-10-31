package dev.sulfur.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import com.materialkolor.Contrast
import com.materialkolor.DynamicMaterialExpressiveTheme
import com.materialkolor.PaletteStyle
import com.materialkolor.dynamiccolor.ColorSpec
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
        animate = true,
        content = content,
    )
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
