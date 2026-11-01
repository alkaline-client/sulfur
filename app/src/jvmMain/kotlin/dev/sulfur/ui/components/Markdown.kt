package dev.sulfur.ui.components

import androidx.compose.foundation.background
import dev.sulfur.ui.theme.Corners
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp

/**
 * The common part of Markdown, enough for a notice board: headings, paragraphs,
 * lists, quotes, code blocks, rules, and bold, italic, code and links inline.
 */
@Composable
fun Markdown(text: String, modifier: Modifier = Modifier) {
    val blocks = remember(text) { parseBlocks(text) }
    val colors = MaterialTheme.colorScheme
    val inline = InlineColors(link = colors.primary, code = colors.surfaceContainerHighest)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        blocks.forEach { block ->
            when (block) {
                is Block.Heading -> Text(
                    inline(block.text, inline),
                    style = when (block.level) {
                        1 -> MaterialTheme.typography.titleLarge
                        2 -> MaterialTheme.typography.titleMedium
                        else -> MaterialTheme.typography.titleSmall
                    },
                )
                is Block.Paragraph -> Text(inline(block.text, inline), style = MaterialTheme.typography.bodyMedium)
                is Block.Items -> Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    block.items.forEachIndexed { index, item ->
                        Row {
                            Text(
                                if (block.ordered) "${index + 1}." else "•",
                                style = MaterialTheme.typography.bodyMedium,
                                color = colors.onSurfaceVariant,
                                modifier = Modifier.width(22.dp),
                            )
                            Text(inline(item, inline), style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                        }
                    }
                }
                is Block.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
                    Box(Modifier.width(3.dp).fillMaxHeight().background(colors.outlineVariant, MaterialTheme.shapes.extraSmall))
                    Text(
                        inline(block.text, inline),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.padding(start = 10.dp),
                    )
                }
                is Block.Code -> Surface(shape = Corners.inset, color = colors.surfaceContainerHighest, modifier = Modifier.fillMaxWidth()) {
                    Text(
                        block.text,
                        style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                        modifier = Modifier.padding(10.dp),
                    )
                }
                Block.Rule -> HorizontalDivider()
            }
        }
    }
}

private sealed interface Block {
    data class Heading(val level: Int, val text: String) : Block
    data class Paragraph(val text: String) : Block
    data class Items(val ordered: Boolean, val items: List<String>) : Block
    data class Quote(val text: String) : Block
    data class Code(val text: String) : Block
    data object Rule : Block
}

private val Bullet = Regex("""^\s*[-*+]\s+(.*)""")
private val Numbered = Regex("""^\s*\d+[.)]\s+(.*)""")
private val Heading = Regex("""^(#{1,6})\s+(.*?)\s*#*\s*$""")
private val RuleLine = Regex("""^\s*([-*_])(\s*\1){2,}\s*$""")

private fun parseBlocks(text: String): List<Block> {
    val blocks = mutableListOf<Block>()
    val lines = text.replace("\r\n", "\n").lines()
    var i = 0
    val paragraph = mutableListOf<String>()
    fun flush() {
        if (paragraph.isNotEmpty()) blocks += Block.Paragraph(paragraph.joinToString(" ") { it.trim() })
        paragraph.clear()
    }
    while (i < lines.size) {
        val line = lines[i]
        when {
            line.trimStart().startsWith("```") -> {
                flush()
                val code = mutableListOf<String>()
                i++
                while (i < lines.size && !lines[i].trimStart().startsWith("```")) code += lines[i++]
                blocks += Block.Code(code.joinToString("\n"))
            }
            line.isBlank() -> flush()
            RuleLine.matches(line) -> {
                flush()
                blocks += Block.Rule
            }
            Heading.matches(line) -> {
                flush()
                val match = Heading.find(line)!!
                blocks += Block.Heading(match.groupValues[1].length, match.groupValues[2])
            }
            line.trimStart().startsWith(">") -> {
                flush()
                val quote = mutableListOf<String>()
                while (i < lines.size && lines[i].trimStart().startsWith(">")) quote += lines[i++].trimStart().removePrefix(">").trim()
                blocks += Block.Quote(quote.joinToString(" "))
                continue
            }
            Bullet.matches(line) || Numbered.matches(line) -> {
                flush()
                val ordered = Numbered.matches(line)
                val pattern = if (ordered) Numbered else Bullet
                val items = mutableListOf<String>()
                while (i < lines.size) {
                    val match = pattern.find(lines[i])
                    when {
                        match != null -> items += match.groupValues[1]
                        // An indented line continues the item above.
                        lines[i].startsWith("  ") && items.isNotEmpty() -> items[items.lastIndex] += " " + lines[i].trim()
                        else -> break
                    }
                    i++
                }
                blocks += Block.Items(ordered, items)
                continue
            }
            else -> paragraph += line
        }
        i++
    }
    flush()
    return blocks
}

private class InlineColors(val link: Color, val code: Color)

private val InlineToken = Regex("""\*\*(.+?)\*\*|__(.+?)__|\*(.+?)\*|(?<!\w)_(.+?)_(?!\w)|`(.+?)`|\[(.+?)]\((\S+?)\)""")

/** Bold, italic, code and links; anything else stays as written. */
private fun inline(text: String, colors: InlineColors): AnnotatedString = buildAnnotatedString {
    var at = 0
    for (match in InlineToken.findAll(text)) {
        append(text.substring(at, match.range.first))
        val g = match.groupValues
        when {
            g[1].isNotEmpty() || g[2].isNotEmpty() -> withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) {
                append(inline(g[1].ifEmpty { g[2] }, colors))
            }
            g[3].isNotEmpty() || g[4].isNotEmpty() -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                append(inline(g[3].ifEmpty { g[4] }, colors))
            }
            g[5].isNotEmpty() -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = colors.code)) { append(g[5]) }
            else -> {
                val url = g[7]
                if (url.startsWith("https://") || url.startsWith("http://")) {
                    withLink(LinkAnnotation.Url(url, TextLinkStyles(SpanStyle(color = colors.link)))) { append(g[6]) }
                } else {
                    append(g[6])
                }
            }
        }
        at = match.range.last + 1
    }
    append(text.substring(at))
}
