package dev.sulfur.feature.mods

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.onClick
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.InputChip
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.input.pointer.isCtrlPressed
import androidx.compose.ui.input.pointer.isMetaPressed
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import dev.sulfur.resources.ic_close
import dev.sulfur.resources.list_add_tip
import dev.sulfur.resources.list_add_tip_mac
import dev.sulfur.resources.list_clear
import dev.sulfur.resources.list_hint
import dev.sulfur.resources.list_in
import dev.sulfur.resources.list_install_all
import dev.sulfur.resources.list_title
import dev.sulfur.resources.n_listed
import dev.sulfur.ui.IsMac
import org.jetbrains.compose.resources.pluralStringResource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import dev.sulfur.ffi.InstalledMod
import dev.sulfur.ffi.InstalledVersion
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.ModHit
import dev.sulfur.ffi.ModSort
import dev.sulfur.ffi.ModSource
import dev.sulfur.ffi.ProjectKind
import dev.sulfur.resources.kind_data_packs
import dev.sulfur.resources.kind_mods
import dev.sulfur.resources.kind_resource_packs
import dev.sulfur.resources.kind_shaders
import dev.sulfur.resources.mods_folder_data_packs
import dev.sulfur.resources.mods_folder_resource_packs
import dev.sulfur.resources.mods_folder_shaders
import dev.sulfur.resources.mods_no_world
import dev.sulfur.resources.mods_search_data_packs
import dev.sulfur.resources.mods_search_resource_packs
import dev.sulfur.resources.mods_search_shaders
import dev.sulfur.resources.mods_shaders_hint
import dev.sulfur.resources.mods_source_both
import dev.sulfur.resources.mods_world
import org.jetbrains.compose.resources.StringResource
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_load_more
import dev.sulfur.resources.action_open_page
import dev.sulfur.resources.action_remove
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.ic_check
import dev.sulfur.resources.ic_delete
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_extension
import dev.sulfur.resources.ic_open_in_new
import dev.sulfur.resources.ic_search
import dev.sulfur.resources.ic_sort
import dev.sulfur.resources.mods_by
import dev.sulfur.resources.mods_downloads
import dev.sulfur.resources.mods_folder
import dev.sulfur.resources.mods_folder_empty
import dev.sulfur.resources.mods_no_loader
import dev.sulfur.resources.mods_no_version
import dev.sulfur.resources.mods_search
import dev.sulfur.resources.mods_sort
import dev.sulfur.resources.mods_sort_downloads
import dev.sulfur.resources.mods_sort_relevance
import dev.sulfur.resources.mods_sort_updated
import dev.sulfur.resources.mods_version
import dev.sulfur.resources.no_results
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.RemoteImage
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.ui.components.formatBytes
import dev.sulfur.ui.components.formatCount
import dev.sulfur.ui.components.loaderName
import dev.sulfur.util.openUrl
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

/**
 * The Mods & resources section of the Download page: search, install and
 * manage mods, resource packs, data packs and shaders of one version.
 */
@Composable
fun ModsPane(viewModel: ModsViewModel, repository: SulfurRepository, modifier: Modifier = Modifier) {
    val ui by viewModel.ui.collectAsState()
    val installed by repository.installed.collectAsState()
    val version = installed.firstOrNull { it.id == ui.versionId }
    val hasModLoader = version?.loader?.let { it != LoaderKind.OPTI_FINE } == true
    val canInstall = when (ui.kind) {
        ProjectKind.MOD -> hasModLoader
        ProjectKind.DATA_PACK -> version != null && ui.world != null
        else -> version != null
    }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            VersionPicker(installed, version, onSelect = viewModel::selectVersion)
            if (ui.kind == ProjectKind.DATA_PACK && ui.worlds.isNotEmpty()) {
                WorldPicker(ui.worlds, ui.world, onSelect = viewModel::selectWorld)
            }
            ConnectedChoice(
                options = ProjectKind.entries,
                selected = ui.kind,
                label = { stringResource(kindTitle(it)) },
                onSelect = viewModel::setKind,
                fill = false,
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedTextField(
                value = ui.query,
                onValueChange = viewModel::setQuery,
                placeholder = { Text(stringResource(searchHint(ui.kind)), maxLines = 1) },
                leadingIcon = { Icon(painterResource(Res.drawable.ic_search), null) },
                singleLine = true,
                shape = CircleShape,
                modifier = Modifier.weight(1f),
            )
            ConnectedChoice(
                options = SourceChoice.entries,
                selected = ui.source,
                label = {
                    when (it) {
                        SourceChoice.Both -> stringResource(Res.string.mods_source_both)
                        SourceChoice.Modrinth -> "Modrinth"
                        SourceChoice.CurseForge -> "CurseForge"
                    }
                },
                onSelect = viewModel::setSource,
                fill = false,
            )
            SortMenu(ui.sort, viewModel::setSort)
        }
        val notice = when {
            version == null -> Res.string.mods_no_version
            ui.kind == ProjectKind.MOD && !hasModLoader -> Res.string.mods_no_loader
            ui.kind == ProjectKind.DATA_PACK && ui.world == null -> Res.string.mods_no_world
            ui.kind == ProjectKind.SHADER -> Res.string.mods_shaders_hint
            else -> null
        }
        if (notice != null) {
            Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.tertiaryContainer) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(painterResource(Res.drawable.ic_extension), null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
                    Spacer(Modifier.width(12.dp))
                    Text(
                        stringResource(notice),
                        color = MaterialTheme.colorScheme.onTertiaryContainer,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f)) {
            val wide = maxWidth >= 840.dp
            Row(horizontalArrangement = Arrangement.spacedBy(CardSpacing)) {
                Results(ui, canInstall, viewModel, Modifier.weight(1.7f).fillMaxHeight())
                if (wide && version != null) {
                    ModsFolder(version, ui.kind, ui.installedMods, viewModel, Modifier.weight(1f).fillMaxHeight())
                }
            }
        }
    }
}

@Composable
private fun VersionPicker(installed: List<InstalledVersion>, current: InstalledVersion?, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }, modifier = Modifier.widthIn(min = 200.dp, max = 280.dp)) {
        OutlinedTextField(
            value = current?.let { describe(it) }.orEmpty(),
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text(stringResource(Res.string.mods_version)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            installed.forEach { v ->
                DropdownMenuItem(
                    text = { Text(describe(v)) },
                    onClick = {
                        onSelect(v.id)
                        open = false
                    },
                )
            }
        }
    }
}

@Composable
private fun WorldPicker(worlds: List<String>, current: String?, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }, modifier = Modifier.widthIn(min = 160.dp, max = 240.dp)) {
        OutlinedTextField(
            value = current.orEmpty(),
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text(stringResource(Res.string.mods_world)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            worlds.forEach { world ->
                DropdownMenuItem(
                    text = { Text(world) },
                    onClick = {
                        onSelect(world)
                        open = false
                    },
                )
            }
        }
    }
}

private fun kindTitle(kind: ProjectKind): StringResource = when (kind) {
    ProjectKind.MOD -> Res.string.kind_mods
    ProjectKind.RESOURCE_PACK -> Res.string.kind_resource_packs
    ProjectKind.DATA_PACK -> Res.string.kind_data_packs
    ProjectKind.SHADER -> Res.string.kind_shaders
}

private fun searchHint(kind: ProjectKind): StringResource = when (kind) {
    ProjectKind.MOD -> Res.string.mods_search
    ProjectKind.RESOURCE_PACK -> Res.string.mods_search_resource_packs
    ProjectKind.DATA_PACK -> Res.string.mods_search_data_packs
    ProjectKind.SHADER -> Res.string.mods_search_shaders
}

private fun folderTitle(kind: ProjectKind): StringResource = when (kind) {
    ProjectKind.MOD -> Res.string.mods_folder
    ProjectKind.RESOURCE_PACK -> Res.string.mods_folder_resource_packs
    ProjectKind.DATA_PACK -> Res.string.mods_folder_data_packs
    ProjectKind.SHADER -> Res.string.mods_folder_shaders
}

private fun sourceName(source: ModSource): String = if (source == ModSource.MODRINTH) "Modrinth" else "CurseForge"

private fun describe(v: InstalledVersion): String =
    v.loader?.let { "${v.id} · ${loaderName(it)}" } ?: v.id

@Composable
internal fun SortMenu(sort: ModSort, onSort: (ModSort) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(painterResource(Res.drawable.ic_sort), stringResource(Res.string.mods_sort)) }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            listOf(
                ModSort.RELEVANCE to Res.string.mods_sort_relevance,
                ModSort.DOWNLOADS to Res.string.mods_sort_downloads,
                ModSort.UPDATED to Res.string.mods_sort_updated,
            ).forEach { (value, label) ->
                DropdownMenuItem(
                    text = { Text(stringResource(label)) },
                    onClick = {
                        onSort(value)
                        open = false
                    },
                    trailingIcon = if (value == sort) {
                        { Icon(painterResource(Res.drawable.ic_check), null) }
                    } else null,
                )
            }
        }
    }
}

@Composable
private fun Results(ui: ModsUiState, canInstall: Boolean, viewModel: ModsViewModel, modifier: Modifier) {
    // Data packs go into one world at a time, so they have no download list.
    val listable = ui.kind.listable && canInstall
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        AnimatedVisibility(
            visible = ui.picking,
            enter = expandVertically(MaterialTheme.motionScheme.defaultSpatialSpec()) + fadeIn(MaterialTheme.motionScheme.defaultEffectsSpec()),
            exit = shrinkVertically(MaterialTheme.motionScheme.fastSpatialSpec()) + fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()),
        ) {
            DownloadListBar(ui, canInstall = ui.versionId != null, viewModel)
        }
        LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp), contentPadding = PaddingValues(bottom = 16.dp)) {
            ui.error?.let { error ->
                item(key = "error") {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(error, color = MaterialTheme.colorScheme.error, modifier = Modifier.weight(1f))
                        TextButton(onClick = { viewModel.search() }) { Text(stringResource(Res.string.action_retry)) }
                    }
                }
            }
            items(ui.hits, key = { it.key }) { hit ->
                ModRow(
                    hit,
                    installing = hit.key in ui.installing,
                    canInstall = canInstall,
                    onInstall = { viewModel.install(hit) },
                    listed = ui.isListed(hit),
                    picking = ui.picking && listable,
                    onToggleList = if (listable) ({ viewModel.toggleListed(hit) }) else null,
                )
            }
            item(key = "footer") {
                Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.Center) {
                    when {
                        ui.loading -> LoadingIndicator(Modifier.size(40.dp))
                        ui.canLoadMore -> TextButton(onClick = { viewModel.search(append = true) }) {
                            Text(stringResource(Res.string.action_load_more))
                        }
                        ui.hits.isEmpty() && ui.error == null -> Text(
                            stringResource(Res.string.no_results),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

/** The projects picked with Ctrl-click, installed together into the selected version. */
@Composable
private fun DownloadListBar(ui: ModsUiState, canInstall: Boolean, viewModel: ModsViewModel) {
    val busy = ui.downloadList.any { it.hit.key in ui.installing }
    Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(start = 16.dp, end = 12.dp, top = 8.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(Res.string.list_title) + " · " + pluralStringResource(Res.plurals.n_listed, ui.downloadList.size, ui.downloadList.size),
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = viewModel::clearList, enabled = !busy) { Text(stringResource(Res.string.list_clear)) }
                if (busy) {
                    LoadingIndicator(Modifier.size(40.dp))
                } else {
                    Button(onClick = viewModel::installList, enabled = canInstall, shapes = ButtonDefaults.shapes()) {
                        Icon(painterResource(Res.drawable.ic_download), null, Modifier.size(ButtonDefaults.IconSize))
                        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                        Text(stringResource(Res.string.list_install_all))
                    }
                }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                ui.downloadList.forEach { entry ->
                    InputChip(
                        selected = false,
                        onClick = { viewModel.removeListed(entry) },
                        enabled = entry.hit.key !in ui.installing,
                        label = { Text("${entry.hit.title} · ${stringResource(kindTitle(entry.kind))}", maxLines = 1) },
                        trailingIcon = { Icon(painterResource(Res.drawable.ic_close), stringResource(Res.string.action_remove), Modifier.size(16.dp)) },
                    )
                }
            }
            Text(
                stringResource(Res.string.list_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }
    }
}

/**
 * One search result. With [onToggleList], Ctrl-click (⌘-click on macOS) on the
 * row or its Install button starts a download list; while [picking], a click
 * adds the project to the list or takes it out.
 */
@OptIn(ExperimentalFoundationApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun ModRow(
    hit: ModHit,
    installing: Boolean,
    canInstall: Boolean,
    onInstall: () -> Unit,
    listed: Boolean = false,
    picking: Boolean = false,
    onToggleList: (() -> Unit)? = null,
) {
    val scope = rememberCoroutineScope()
    val window = LocalWindowInfo.current
    val listClick = when {
        onToggleList == null -> Modifier
        picking -> Modifier.clickable(onClick = onToggleList)
        else -> Modifier.onClick(keyboardModifiers = { isCtrlPressed || isMetaPressed }, onClick = onToggleList)
    }
    val color by animateColorAsState(
        if (listed) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainerLow,
        MaterialTheme.motionScheme.defaultEffectsSpec(),
    )
    Surface(Modifier.fillMaxWidth().clip(MaterialTheme.shapes.medium).then(listClick), shape = MaterialTheme.shapes.medium, color = color) {
        Row(Modifier.padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
            RemoteImage(hit.iconUrl, Modifier.size(48.dp).clip(MaterialTheme.shapes.small)) {
                Icon(painterResource(Res.drawable.ic_extension), null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(hit.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false))
                    if (hit.author.isNotBlank()) {
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(Res.string.mods_by, hit.author), style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                    }
                }
                Text(hit.description, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                // Downloads (summed over sources), where the project is, and its first categories.
                val sources = hit.refs.joinToString(" + ") { sourceName(it.source) }
                Text(
                    (listOf(stringResource(Res.string.mods_downloads, formatCount(hit.downloads)), sources) + hit.categories.take(3))
                        .joinToString(" · "),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (hit.pageUrl.isNotBlank()) {
                IconButton(onClick = { scope.launch { openUrl(hit.pageUrl) } }) {
                    Icon(painterResource(Res.drawable.ic_open_in_new), stringResource(Res.string.action_open_page))
                }
            }
            Box(Modifier.widthIn(min = 48.dp), contentAlignment = Alignment.Center) {
                when {
                    installing -> LoadingIndicator(Modifier.size(36.dp))
                    picking && onToggleList != null -> {
                        val label = stringResource(Res.string.list_in)
                        Checkbox(
                            checked = listed,
                            onCheckedChange = { onToggleList() },
                            modifier = Modifier.semantics { contentDescription = label },
                        )
                    }
                    else -> InstallButton(
                        enabled = canInstall,
                        tip = onToggleList != null,
                        onClick = {
                            val keys = window.keyboardModifiers
                            if (onToggleList != null && (keys.isCtrlPressed || keys.isMetaPressed)) onToggleList() else onInstall()
                        },
                    )
                }
            }
        }
    }
}

/** Install, with a tooltip about download lists where they are possible. */
@Composable
private fun InstallButton(enabled: Boolean, tip: Boolean, onClick: () -> Unit) {
    val button = @Composable {
        FilledTonalButton(onClick = onClick, enabled = enabled, shapes = ButtonDefaults.shapes()) {
            Icon(painterResource(Res.drawable.ic_download), null, Modifier.size(ButtonDefaults.IconSize))
            Spacer(Modifier.width(ButtonDefaults.IconSpacing))
            Text(stringResource(Res.string.action_install))
        }
    }
    if (!tip || !enabled) return button()
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text(stringResource(if (IsMac) Res.string.list_add_tip_mac else Res.string.list_add_tip)) } },
        state = rememberTooltipState(),
    ) { button() }
}

@Composable
private fun ModsFolder(version: InstalledVersion, kind: ProjectKind, mods: List<InstalledMod>, viewModel: ModsViewModel, modifier: Modifier) {
    SectionCard(
        stringResource(folderTitle(kind), version.id),
        modifier,
        icon = painterResource(Res.drawable.ic_extension),
    ) {
        if (mods.isEmpty()) {
            Text(stringResource(Res.string.mods_folder_empty), style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items(mods, key = { it.fileName }) { mod ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ShapeIcon(painterResource(Res.drawable.ic_extension), MaterialShapes.Square, size = 28.dp)
                    Spacer(Modifier.width(8.dp))
                    Column(Modifier.weight(1f)) {
                        Text(mod.fileName.removeSuffix(".disabled"), style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(formatBytes(mod.size), style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(mod.enabled, onCheckedChange = { viewModel.setEnabled(mod, it) })
                    IconButton(onClick = { viewModel.remove(mod) }) {
                        Icon(painterResource(Res.drawable.ic_delete), stringResource(Res.string.action_remove))
                    }
                }
            }
        }
    }
}
