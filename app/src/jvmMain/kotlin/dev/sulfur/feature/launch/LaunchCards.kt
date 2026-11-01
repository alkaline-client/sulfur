package dev.sulfur.feature.launch

import androidx.compose.foundation.clickable
import dev.sulfur.ui.theme.Corners
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.isRunning
import dev.sulfur.feature.settings.Hint
import dev.sulfur.feature.settings.Label
import dev.sulfur.feature.settings.OptionField
import dev.sulfur.ffi.LaunchColumn
import dev.sulfur.ffi.LaunchWidget
import dev.sulfur.ffi.NewsEntry
import dev.sulfur.ffi.VersionList
import dev.sulfur.ffi.WidgetKind
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_cancel
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_save
import dev.sulfur.resources.ic_add
import dev.sulfur.resources.ic_arrow_downward
import dev.sulfur.resources.ic_arrow_upward
import dev.sulfur.resources.ic_check
import dev.sulfur.resources.ic_delete
import dev.sulfur.resources.ic_description
import dev.sulfur.resources.ic_edit
import dev.sulfur.resources.ic_history_edu
import dev.sulfur.resources.ic_layers
import dev.sulfur.resources.ic_link
import dev.sulfur.resources.ic_new_releases
import dev.sulfur.resources.ic_widgets
import dev.sulfur.resources.launch_add_card
import dev.sulfur.resources.launch_cards_hint
import dev.sulfur.resources.launch_column
import dev.sulfur.resources.launch_column_left
import dev.sulfur.resources.launch_column_right
import dev.sulfur.resources.launch_customize
import dev.sulfur.resources.launch_customize_done
import dev.sulfur.resources.widget_edit
import dev.sulfur.resources.widget_empty
import dev.sulfur.resources.widget_links
import dev.sulfur.resources.widget_links_text
import dev.sulfur.resources.widget_load_error
import dev.sulfur.resources.widget_markdown
import dev.sulfur.resources.widget_markdown_source
import dev.sulfur.resources.widget_move_down
import dev.sulfur.resources.widget_move_up
import dev.sulfur.resources.widget_news
import dev.sulfur.resources.widget_note
import dev.sulfur.resources.widget_remove
import dev.sulfur.resources.widget_text
import dev.sulfur.resources.widget_title
import dev.sulfur.resources.widget_title_hint
import dev.sulfur.resources.widget_versions
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.Markdown
import dev.sulfur.ui.components.RemoteImage
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.components.Logo
import dev.sulfur.ui.components.minecraftLogo
import dev.sulfur.ui.components.kindLabel
import dev.sulfur.util.openUrl
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale

private val WidgetKind.label: StringResource
    get() = when (this) {
        WidgetKind.NEWS -> Res.string.widget_news
        WidgetKind.VERSIONS -> Res.string.widget_versions
        WidgetKind.NOTE -> Res.string.widget_note
        WidgetKind.LINKS -> Res.string.widget_links
        WidgetKind.MARKDOWN -> Res.string.widget_markdown
    }

private val WidgetKind.icon: DrawableResource
    get() = when (this) {
        WidgetKind.NEWS -> Res.drawable.ic_new_releases
        WidgetKind.VERSIONS -> Res.drawable.ic_layers
        WidgetKind.NOTE -> Res.drawable.ic_history_edu
        WidgetKind.LINKS -> Res.drawable.ic_link
        WidgetKind.MARKDOWN -> Res.drawable.ic_description
    }

/** Kinds whose content the user writes; the others fill themselves. */
private val WidgetKind.editable: Boolean get() = this == WidgetKind.NOTE || this == WidgetKind.LINKS || this == WidgetKind.MARKDOWN

/**
 * The Launch page's cards beside the Play column, as arranged in edit mode:
 * the column's side, and cards added, moved, edited and removed in place.
 */
@Composable
internal fun LaunchCards(repository: SulfurRepository, customizing: MutableStateFlow<Boolean>, modifier: Modifier = Modifier) {
    val settings by repository.settings.collectAsState()
    val widgets = settings.launchWidgets
    val editing by customizing.collectAsState()
    var editingIndex by remember { mutableStateOf<Int?>(null) }
    fun setWidgets(list: List<LaunchWidget>) = repository.updateSettings { it.copy(launchWidgets = list) }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
        Toolbar(
            editing = editing,
            column = settings.launchColumn,
            onEditing = { customizing.value = it },
            onColumn = { c -> repository.updateSettings { it.copy(launchColumn = c) } },
            onAdd = { kind ->
                setWidgets(widgets + LaunchWidget(kind, "", ""))
                if (kind.editable) editingIndex = widgets.size
            },
        )
        if (widgets.isEmpty()) {
            if (editing) Hint(stringResource(Res.string.launch_cards_hint))
            return@Column
        }
        LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
            itemsIndexed(widgets) { index, widget ->
                WidgetCard(
                    widget = widget,
                    repository = repository,
                    editing = editing,
                    canMoveUp = index > 0,
                    canMoveDown = index < widgets.lastIndex,
                    onMove = { by ->
                        setWidgets(widgets.toMutableList().apply { add(index + by, removeAt(index)) })
                    },
                    onEdit = { editingIndex = index },
                    onRemove = { setWidgets(widgets.filterIndexed { i, _ -> i != index }) },
                )
            }
        }
    }
    editingIndex?.let { index ->
        widgets.getOrNull(index)?.let { widget ->
            WidgetEditor(
                widget = widget,
                onSave = { edited ->
                    setWidgets(widgets.toMutableList().apply { set(index, edited) })
                    editingIndex = null
                },
                onDismiss = { editingIndex = null },
            )
        }
    }
}

@Composable
private fun Toolbar(
    editing: Boolean,
    column: LaunchColumn,
    onEditing: (Boolean) -> Unit,
    onColumn: (LaunchColumn) -> Unit,
    onAdd: (WidgetKind) -> Unit,
) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
        if (!editing) {
            IconAction(Res.drawable.ic_widgets, stringResource(Res.string.launch_customize)) { onEditing(true) }
            return@Row
        }
        Label(stringResource(Res.string.launch_column))
        ConnectedChoice(
            options = LaunchColumn.entries,
            selected = column,
            label = { stringResource(if (it == LaunchColumn.LEFT) Res.string.launch_column_left else Res.string.launch_column_right) },
            onSelect = onColumn,
            fill = false,
        )
        Spacer(Modifier.width(8.dp))
        Box {
            var open by remember { mutableStateOf(false) }
            OutlinedButton(onClick = { open = true }, shapes = ButtonDefaults.shapes()) {
                Icon(painterResource(Res.drawable.ic_add), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(Res.string.launch_add_card))
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                WidgetKind.entries.forEach { kind ->
                    DropdownMenuItem(
                        text = { Text(stringResource(kind.label)) },
                        leadingIcon = { Icon(painterResource(kind.icon), null) },
                        onClick = {
                            open = false
                            onAdd(kind)
                        },
                    )
                }
            }
        }
        Button(onClick = { onEditing(false) }, shapes = ButtonDefaults.shapes()) {
            Icon(painterResource(Res.drawable.ic_check), null, Modifier.size(ButtonDefaults.IconSize))
            Spacer(Modifier.width(ButtonDefaults.IconSpacing))
            Text(stringResource(Res.string.launch_customize_done))
        }
    }
}

/** An icon button with its name as a tooltip. */
@Composable
private fun IconAction(icon: DrawableResource, label: String, enabled: Boolean = true, onClick: () -> Unit) {
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text(label) } },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = onClick, enabled = enabled) { Icon(painterResource(icon), label) }
    }
}

@Composable
private fun WidgetCard(
    widget: LaunchWidget,
    repository: SulfurRepository,
    editing: Boolean,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onMove: (Int) -> Unit,
    onEdit: () -> Unit,
    onRemove: () -> Unit,
) {
    SectionCard(
        title = widget.title.ifBlank { stringResource(widget.kind.label) },
        icon = painterResource(widget.kind.icon),
        trailing = {
            if (editing) {
                IconAction(Res.drawable.ic_arrow_upward, stringResource(Res.string.widget_move_up), canMoveUp) { onMove(-1) }
                IconAction(Res.drawable.ic_arrow_downward, stringResource(Res.string.widget_move_down), canMoveDown) { onMove(1) }
                IconAction(Res.drawable.ic_edit, stringResource(Res.string.widget_edit), onClick = onEdit)
                IconAction(Res.drawable.ic_delete, stringResource(Res.string.widget_remove), onClick = onRemove)
            }
        },
    ) {
        when (widget.kind) {
            WidgetKind.NEWS -> NewsContent(repository)
            WidgetKind.VERSIONS -> VersionsContent(repository)
            WidgetKind.NOTE ->
                if (widget.content.isBlank()) Hint(stringResource(Res.string.widget_empty))
                else Text(widget.content, style = MaterialTheme.typography.bodyMedium)
            WidgetKind.LINKS -> LinksContent(widget.content)
            WidgetKind.MARKDOWN -> MarkdownContent(repository, widget.content)
        }
    }
}

/** A loaded value, or why it could not be loaded; null while loading. */
private sealed interface Loaded<out T> {
    data class Ok<T>(val value: T) : Loaded<T>
    data class Failed(val message: String) : Loaded<Nothing>
}

/**
 * What cards loaded earlier this session, by key: a card coming back with the
 * page shows it at once instead of a loading indicator and a second layout
 * pass, and refreshes it quietly.
 */
private val loadedCards = HashMap<String, Loaded<*>>()

@Composable
private fun <T> rememberLoaded(key: String, load: suspend () -> T): Loaded<T>? {
    @Suppress("UNCHECKED_CAST")
    var state by remember(key) { mutableStateOf(loadedCards[key] as Loaded<T>?) }
    LaunchedEffect(key) {
        val loaded = runCatching { load() }.fold({ Loaded.Ok(it) }, { Loaded.Failed(it.message ?: it.toString()) })
        if (loaded is Loaded.Ok || state !is Loaded.Ok) {
            state = loaded
            loadedCards[key] = loaded
        }
    }
    return state
}

@Composable
private fun <T> LoadedContent(state: Loaded<T>?, content: @Composable (T) -> Unit) {
    when (state) {
        null -> Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.Center) { LoadingIndicator(Modifier.size(32.dp)) }
        is Loaded.Failed -> Hint(stringResource(Res.string.widget_load_error, state.message))
        is Loaded.Ok -> content(state.value)
    }
}

private val NewsDate = DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)

@Composable
private fun NewsContent(repository: SulfurRepository) {
    val state = rememberLoaded("news") { repository.launcherNews(false) }
    LoadedContent(state) { news ->
        if (news.isEmpty()) Hint(stringResource(Res.string.widget_empty))
        news.take(4).forEach { NewsRow(it) }
    }
}

@Composable
private fun NewsRow(entry: NewsEntry) {
    val scope = rememberCoroutineScope()
    val date = remember(entry.date) {
        runCatching { LocalDate.parse(entry.date).format(NewsDate.withLocale(Locale.getDefault())) }.getOrDefault(entry.date)
    }
    Row(
        Modifier.fillMaxWidth().clip(Corners.item).clickable(enabled = entry.link.isNotBlank()) {
            scope.launch { openUrl(entry.link) }
        }.padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RemoteImage(entry.imageUrl, Modifier.size(width = 120.dp, height = 68.dp).clip(Corners.item))
        Column(Modifier.weight(1f)) {
            Text(entry.title, style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(date, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (entry.text.isNotBlank()) {
                Text(entry.text, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun VersionsContent(repository: SulfurRepository) {
    val state = rememberLoaded("versions") { repository.versionList(false) }
    val installed by repository.installed.collectAsState()
    val installs by repository.installs.collectAsState()
    LoadedContent<VersionList>(state) { list ->
        listOf(list.latestRelease, list.latestSnapshot).distinct().forEach { id ->
            val remote = list.versions.firstOrNull { it.id == id } ?: return@forEach
            val task = installs.firstOrNull { it.versionId == id && it.isRunning }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Logo(minecraftLogo(remote.kind), 36.dp)
                Column(Modifier.weight(1f)) {
                    Text(id, style = MaterialTheme.typography.titleSmall)
                    Text(kindLabel(remote.kind), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    task?.let { LinearWavyProgressIndicator(progress = { it.fraction.toFloat() }, modifier = Modifier.fillMaxWidth().padding(top = 4.dp)) }
                }
                if (installed.any { it.id == id }) {
                    Icon(painterResource(Res.drawable.ic_check), null, tint = MaterialTheme.colorScheme.primary)
                } else {
                    FilledTonalButton(onClick = { repository.install(id) }, enabled = task == null, shapes = ButtonDefaults.shapes()) {
                        Text(stringResource(Res.string.action_install))
                    }
                }
            }
        }
    }
}

/** `label | url` lines; a line without a label shows the address. */
internal fun parseLinks(text: String): List<Pair<String, String>> = text.lines().mapNotNull { line ->
    val parts = line.split('|', limit = 2).map { it.trim() }
    when {
        parts.all { it.isEmpty() } -> null
        parts.size == 2 && parts[1].isNotEmpty() -> parts[0].ifEmpty { parts[1] } to parts[1]
        else -> parts[0] to parts[0]
    }
}

@Composable
private fun LinksContent(text: String) {
    val links = remember(text) { parseLinks(text) }
    val scope = rememberCoroutineScope()
    if (links.isEmpty()) {
        Hint(stringResource(Res.string.widget_empty))
        return
    }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        links.forEach { (label, url) ->
            AssistChip(
                onClick = { scope.launch { openUrl(url) } },
                label = { Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                leadingIcon = { Icon(painterResource(Res.drawable.ic_link), null, Modifier.size(18.dp)) },
                modifier = Modifier.widthIn(max = 280.dp),
            )
        }
    }
}

@Composable
private fun MarkdownContent(repository: SulfurRepository, location: String) {
    if (location.isBlank()) {
        Hint(stringResource(Res.string.widget_empty))
        return
    }
    val state = rememberLoaded("document:$location") { repository.readDocument(location.trim()) }
    LoadedContent(state) { text -> Markdown(text, Modifier.fillMaxWidth()) }
}

@Composable
private fun WidgetEditor(widget: LaunchWidget, onSave: (LaunchWidget) -> Unit, onDismiss: () -> Unit) {
    var title by remember { mutableStateOf(widget.title) }
    var content by remember { mutableStateOf(widget.content) }
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(painterResource(widget.kind.icon), null) },
        title = { Text(stringResource(widget.kind.label)) },
        text = {
            Column(Modifier.width(480.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OptionField(title, { title = it }, stringResource(Res.string.widget_title), stringResource(Res.string.widget_title_hint))
                when (widget.kind) {
                    WidgetKind.NOTE -> OptionField(content, { content = it }, stringResource(Res.string.widget_text), singleLine = false)
                    WidgetKind.LINKS -> OptionField(content, { content = it }, stringResource(Res.string.widget_links_text),
                        placeholder = "Minecraft Wiki | https://minecraft.wiki", singleLine = false, monospace = true)
                    WidgetKind.MARKDOWN -> OptionField(content, { content = it }, stringResource(Res.string.widget_markdown_source),
                        placeholder = "https://…/README.md")
                    WidgetKind.NEWS, WidgetKind.VERSIONS -> {}
                }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_cancel)) } },
        confirmButton = {
            TextButton(onClick = { onSave(widget.copy(title = title.trim(), content = content)) }) {
                Text(stringResource(Res.string.action_save))
            }
        },
    )
}
