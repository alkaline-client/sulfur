package dev.sulfur.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import java.util.Locale

/** The system's locale, captured before Sulfur replaces the default. */
private val systemLocale: Locale = Locale.getDefault()

/**
 * The locale for a language setting: `en`, `zh-CN`, `zh-TW`, or empty for
 * the system's. Chinese is either Simplified or Traditional (Taiwan, Hong
 * Kong, Macau and `zh-Hant` read Traditional), matching the translations.
 */
fun appLocale(language: String): Locale = when (language) {
    "en" -> Locale.ENGLISH
    "zh-CN" -> Locale.SIMPLIFIED_CHINESE
    "zh-TW" -> Locale.TRADITIONAL_CHINESE
    else -> when {
        systemLocale.language != "zh" -> systemLocale
        systemLocale.script == "Hant" || systemLocale.country in setOf("TW", "HK", "MO") -> Locale.TRADITIONAL_CHINESE
        else -> Locale.SIMPLIFIED_CHINESE
    }
}

/**
 * Shows [content] in [language]. String resources, number and date formats
 * and the CJK typeface follow `Locale.getDefault()`, so it is replaced and
 * everything below is composed afresh when the language changes.
 */
@Composable
fun LocalizedApp(language: String, content: @Composable () -> Unit) {
    val locale = appLocale(language)
    if (Locale.getDefault() != locale) Locale.setDefault(locale)
    key(locale) { content() }
}
