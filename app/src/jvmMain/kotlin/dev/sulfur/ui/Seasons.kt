package dev.sulfur.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.semantics.clearAndSetSemantics
import kotlinx.coroutines.delay
import java.time.LocalDate
import java.time.MonthDay
import kotlin.math.sin
import kotlin.random.Random

/** What a special day adds to the window. */
enum class SeasonalEffect { Bats, Snow, Confetti }

/** A special day: what it draws, and whether Sulfur is spelled the other way. */
class Occasion(val effect: SeasonalEffect?, val brand: String = "Sulfur")

/** The first day of the Lunar New Year, which moves with the moon. */
private val LunarNewYears: Set<LocalDate> = listOf(
    "2027-02-06", "2028-01-26", "2029-02-13", "2030-02-03", "2031-01-23", "2032-02-11", "2033-01-31",
    "2034-02-19", "2035-02-08",
).map(LocalDate::parse).toSet()

/**
 * Today, or the day in `SULFUR_DATE` (or `-Dsulfur.date`), e.g. `2026-12-25`,
 * to preview an occasion during development.
 */
fun today(): LocalDate =
    (System.getProperty("sulfur.date") ?: System.getenv("SULFUR_DATE"))
        ?.let { runCatching { LocalDate.parse(it) }.getOrNull() }
        ?: LocalDate.now()

/** The occasion on [date], if any. Halloween is also Sulfur's birthday. */
fun occasionOn(date: LocalDate): Occasion? {
    val day = MonthDay.from(date)
    return when {
        day == MonthDay.of(10, 31) -> Occasion(SeasonalEffect.Bats)
        day >= MonthDay.of(12, 24) && day <= MonthDay.of(12, 26) -> Occasion(SeasonalEffect.Snow)
        day == MonthDay.of(12, 31) -> Occasion(SeasonalEffect.Snow)
        day == MonthDay.of(1, 1) -> Occasion(SeasonalEffect.Confetti)
        date in LunarNewYears -> Occasion(SeasonalEffect.Confetti)
        day == MonthDay.of(4, 1) -> Occasion(null, brand = "Sulphur")
        day == MonthDay.of(5, 17) -> Occasion(SeasonalEffect.Confetti)
        else -> null
    }
}

/** The day's effect behind the pages (bats, snow) or over them for a moment (confetti). */
@Composable
fun SeasonalSky(effect: SeasonalEffect, modifier: Modifier = Modifier) {
    when (effect) {
        SeasonalEffect.Snow -> Snow(modifier)
        SeasonalEffect.Bats -> Bats(modifier)
        SeasonalEffect.Confetti -> Confetti(modifier)
    }
}

/** Seconds since the effect started, advanced every frame while [running]. */
@Composable
private fun frameClock(running: Boolean = true): Float {
    var seconds by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(running) {
        if (!running) return@LaunchedEffect
        val start = withFrameNanos { it }
        while (true) withFrameNanos { seconds = (it - start) / 1e9f }
    }
    return seconds
}

private class Flake(val x: Float, val y: Float, val radius: Float, val fall: Float, val sway: Float, val phase: Float)

/** Snow drifting down past the title bar and the page's edges. */
@Composable
private fun Snow(modifier: Modifier) {
    val flakes = remember {
        val r = Random(2026)
        List(70) { Flake(r.nextFloat(), r.nextFloat(), 1.2f + r.nextFloat() * 2.4f, 18f + r.nextFloat() * 30f, 6f + r.nextFloat() * 14f, r.nextFloat() * 6.3f) }
    }
    val time = frameClock()
    val color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
    Canvas(modifier.fillMaxSize().clearAndSetSemantics {}) {
        for (f in flakes) {
            val y = (f.y * size.height + time * f.fall * density) % (size.height + 10f) - 5f
            val x = f.x * size.width + sin(time * 0.6f + f.phase) * f.sway * density
            drawCircle(color, f.radius * density, Offset(x, y))
        }
    }
}

/**
 * A few bats crossing the title bar now and then: a flight every 10 to 25
 * seconds, so the window is still most of the time.
 */
@Composable
private fun Bats(modifier: Modifier) {
    var flight by remember { mutableIntStateOf(0) }
    var flying by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(2_500)
        while (true) {
            flight++
            flying = true
            delay(FlightSeconds.toLong() * 1000)
            flying = false
            delay(Random.nextLong(10_000, 25_000))
        }
    }
    if (!flying) return
    val time = frameClock(flying)
    val color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.7f)
    val bats = remember(flight) {
        val r = Random(flight)
        val leftToRight = r.nextBoolean()
        List(2 + r.nextInt(3)) { Bat(r.nextFloat() * 0.8f, 14f + r.nextFloat() * 40f, 0.7f + r.nextFloat() * 0.5f, r.nextFloat() * 6f, leftToRight) }
    }
    Canvas(modifier.fillMaxSize().clearAndSetSemantics {}) {
        for (bat in bats) {
            val progress = (time / FlightSeconds - bat.delay * 0.3f) / 0.7f
            if (progress !in 0f..1f) continue
            val span = size.width + 120 * density
            val x = (if (bat.leftToRight) progress else 1 - progress) * span - 60 * density
            val y = (bat.height + sin(time * 3f + bat.phase) * 6f) * density
            drawBat(Offset(x, y), bat.scale * density, flap = sin(time * 18f + bat.phase), color, mirrored = !bat.leftToRight)
        }
    }
}

private const val FlightSeconds = 7f

private class Bat(val delay: Float, val height: Float, val scale: Float, val phase: Float, val leftToRight: Boolean)

/** A bat silhouette about 28 units wide; [flap] in -1..1 raises and lowers its wings. */
private fun DrawScope.drawBat(at: Offset, k: Float, flap: Float, color: Color, mirrored: Boolean) {
    translate(at.x, at.y) {
        rotate(if (mirrored) 4f else -4f, Offset.Zero) {
            val lift = flap * 7f * k
            val wing = Path().apply {
                moveTo(0f, 0f)
                quadraticTo(6f * k, -4f * k - lift, 14f * k, -2f * k - lift)
                quadraticTo(11f * k, 1f * k - lift / 2, 12f * k, 4f * k - lift / 3)
                quadraticTo(8f * k, 2f * k, 6f * k, 4f * k)
                quadraticTo(3f * k, 2f * k, 0f, 3f * k)
                close()
            }
            drawPath(wing, color)
            scale(-1f, 1f, Offset.Zero) { drawPath(wing, color) }
            drawOval(color, Offset(-2.2f * k, -2.5f * k), Size(4.4f * k, 6.5f * k))
            // Ears.
            drawPath(Path().apply {
                moveTo(-2f * k, -2f * k); lineTo(-1.6f * k, -4.6f * k); lineTo(-0.5f * k, -2.4f * k)
                moveTo(2f * k, -2f * k); lineTo(1.6f * k, -4.6f * k); lineTo(0.5f * k, -2.4f * k)
            }, color)
        }
    }
}

private class Piece(val x: Float, val speed: Float, val drift: Float, val spin: Float, val width: Float, val color: Int)

/** Confetti over the window for a few seconds after start, then nothing. */
@Composable
private fun Confetti(modifier: Modifier) {
    var done by remember { mutableStateOf(false) }
    if (done) return
    val colors = MaterialTheme.colorScheme
    val palette = listOf(colors.primary, colors.secondary, colors.tertiary, colors.primaryContainer, colors.tertiaryContainer)
    val pieces = remember {
        val r = Random(517)
        List(120) { Piece(r.nextFloat(), 0.6f + r.nextFloat() * 0.8f, r.nextFloat() * 2 - 1, r.nextFloat() * 720 - 360, 4f + r.nextFloat() * 5, r.nextInt(5)) }
    }
    val time = frameClock()
    LaunchedEffect(Unit) {
        delay(ConfettiSeconds.toLong() * 1000)
        done = true
    }
    Canvas(modifier.fillMaxSize().clearAndSetSemantics {}) {
        // Fades over the last second.
        val alpha = (ConfettiSeconds - time).coerceIn(0f, 1f)
        for (p in pieces) {
            val t = time - p.x * 0.6f
            if (t < 0) continue
            // The slowest pieces reach the bottom as the burst fades.
            val y = -20f + t * p.speed * size.height * 0.16f + 0.5f * 14f * t * t * density
            val x = p.x * size.width + sin(t * 2f + p.drift * 3f) * 30f * density + p.drift * t * 40f * density
            if (y > size.height) continue
            rotate(t * p.spin, Offset(x, y)) {
                drawRect(palette[p.color].copy(alpha = alpha), Offset(x, y), Size(p.width * density, p.width * density * 0.45f))
            }
        }
    }
}

private const val ConfettiSeconds = 5f
