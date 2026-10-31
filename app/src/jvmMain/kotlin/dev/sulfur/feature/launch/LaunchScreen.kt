package dev.sulfur.feature.launch

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ContainedLoadingIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
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
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import org.jetbrains.compose.resources.DrawableResource
import dev.sulfur.data.SulfurRepository
import dev.sulfur.ffi.InstalledVersion
import dev.sulfur.ffi.Isolation
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_browse_versions
import dev.sulfur.resources.action_choose_version
import dev.sulfur.resources.action_dismiss
import dev.sulfur.resources.action_play
import dev.sulfur.resources.action_close
import dev.sulfur.resources.action_stop
import dev.sulfur.resources.ic_arrow_drop_down
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_extension
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.ic_layers
import dev.sulfur.resources.ic_person
import dev.sulfur.resources.ic_play_arrow
import dev.sulfur.resources.ic_stop
import dev.sulfur.resources.ic_terminal
import dev.sulfur.resources.ic_tune
import dev.sulfur.resources.isolation_default
import dev.sulfur.resources.isolation_isolated
import dev.sulfur.resources.isolation_isolated_hint
import dev.sulfur.resources.isolation_shared
import dev.sulfur.resources.isolation_shared_hint
import dev.sulfur.resources.launch_empty_hint
import dev.sulfur.resources.launch_empty_title
import dev.sulfur.resources.launch_exited
import dev.sulfur.resources.launch_exited_code
import dev.sulfur.resources.launch_game_folder
import dev.sulfur.resources.launch_java
import dev.sulfur.resources.launch_log
import dev.sulfur.resources.launch_log_empty
import dev.sulfur.resources.launch_offline_hint
import dev.sulfur.resources.launch_offline_name
import dev.sulfur.resources.launch_running
import dev.sulfur.resources.launch_running_pid
import dev.sulfur.resources.launch_saves_mods
import dev.sulfur.resources.launch_starting
import dev.sulfur.resources.launch_version_settings
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.PagePadding
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.ui.components.loaderName
import dev.sulfur.util.openFolder
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

@Composable
fun LaunchScreen(
    viewModel: LaunchViewModel,
    repository: SulfurRepository,
    onBrowseVersions: () -> Unit,
) {
    val installed by repository.installed.collectAsState()
    val selectedId by viewModel.selected.collectAsState()
    val state by viewModel.state.collectAsState()
    val logs by viewModel.logs.collectAsState()
    val account by repository.currentAccount.collectAsState()
    val selected = installed.firstOrNull { it.id == selectedId }
    var showOutput by remember { mutableStateOf(false) }
    var showVersionSettings by remember { mutableStateOf(false) }

    if (installed.isEmpty()) {
        EmptyState(onBrowseVersions)
        return
    }
    // Only what starting the game needs, at the bottom left: who plays, Play
    // with the version, and the version's settings. The rest of the page is
    // kept free for later features.
    Box(Modifier.fillMaxSize().padding(PagePadding), contentAlignment = Alignment.BottomStart) {
        Column(Modifier.widthIn(max = LaunchColumnWidth), verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
            AccountCard(repository)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PlayButton(
                    installed = installed,
                    selected = selected,
                    state = state,
                    canPlay = account != null,
                    onSelect = viewModel::select,
                    onPlay = viewModel::play,
                    onStop = viewModel::stop,
                )
                val height = SplitButtonDefaults.LargeContainerHeight
                FilledTonalIconButton(
                    onClick = { showVersionSettings = true },
                    enabled = selected != null,
                    shapes = IconButtonDefaults.shapes(),
                    modifier = Modifier.size(height),
                ) {
                    Icon(
                        painterResource(Res.drawable.ic_tune),
                        stringResource(Res.string.launch_version_settings),
                        Modifier.size(ButtonDefaults.iconSizeFor(height)),
                    )
                }
            }
            StatusBanner(state, onDismiss = viewModel::dismiss, onShowOutput = { showOutput = true })
        }
    }
    if (showOutput) OutputDialog(logs, onDismiss = { showOutput = false })
    if (showVersionSettings && selected != null) {
        VersionSettingsDialog(selected, repository, onDismiss = { showVersionSettings = false })
    }
}

/** Width of the launch column; wide enough for translated labels. */
private val LaunchColumnWidth = 440.dp

/**
 * The launch button: Play (or Stop while the game runs) with the selected
 * version under it, and a version picker on the trailing half.
 */
@Composable
private fun PlayButton(
    installed: List<InstalledVersion>,
    selected: InstalledVersion?,
    state: GameState,
    canPlay: Boolean,
    onSelect: (String) -> Unit,
    onPlay: () -> Unit,
    onStop: () -> Unit,
) {
    val height = SplitButtonDefaults.LargeContainerHeight
    val version = selected?.let { v -> v.loader?.let { "${v.id} · ${loaderName(it)}" } ?: v.id }.orEmpty()
    if (state is GameState.Running) {
        Button(
            onClick = onStop,
            shapes = ButtonDefaults.shapesFor(height),
            modifier = Modifier.heightIn(height).widthIn(min = PlayButtonMinWidth),
            contentPadding = ButtonDefaults.contentPaddingFor(height),
            colors = ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.error,
                contentColor = MaterialTheme.colorScheme.onError,
            ),
        ) {
            ButtonLabel(Res.drawable.ic_stop, stringResource(Res.string.action_stop), version, height)
        }
        return
    }
    val enabled = state !is GameState.Launching
    var menuOpen by remember { mutableStateOf(false) }
    SplitButtonLayout(
        leadingButton = {
            SplitButtonDefaults.LeadingButton(
                onClick = onPlay,
                enabled = enabled && canPlay,
                shapes = SplitButtonDefaults.leadingButtonShapesFor(height),
                contentPadding = SplitButtonDefaults.leadingButtonContentPaddingFor(height),
                modifier = Modifier.heightIn(height).widthIn(min = PlayButtonMinWidth),
            ) {
                ButtonLabel(Res.drawable.ic_play_arrow, stringResource(Res.string.action_play), version, height)
            }
        },
        trailingButton = {
            Box {
                SplitButtonDefaults.TrailingButton(
                    checked = menuOpen,
                    onCheckedChange = { menuOpen = it },
                    enabled = enabled,
                    shapes = SplitButtonDefaults.trailingButtonShapesFor(height),
                    contentPadding = SplitButtonDefaults.trailingButtonContentPaddingFor(height),
                    modifier = Modifier.heightIn(height),
                ) {
                    val rotation by animateFloatAsState(if (menuOpen) 180f else 0f, MaterialTheme.motionScheme.defaultSpatialSpec())
                    Icon(
                        painterResource(Res.drawable.ic_arrow_drop_down),
                        contentDescription = stringResource(Res.string.action_choose_version),
                        modifier = Modifier.size(SplitButtonDefaults.trailingButtonIconSizeFor(height)).rotate(rotation),
                    )
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    installed.forEach { v ->
                        DropdownMenuItem(
                            text = { Text(v.loader?.let { "${v.id} · ${loaderName(it)}" } ?: v.id) },
                            onClick = {
                                onSelect(v.id)
                                menuOpen = false
                            },
                        )
                    }
                }
            }
        },
    )
}

private val PlayButtonMinWidth = 260.dp

@Composable
private fun ButtonLabel(icon: DrawableResource, action: String, version: String, height: Dp) {
    Icon(painterResource(icon), null, Modifier.size(ButtonDefaults.iconSizeFor(height)))
    Spacer(Modifier.width(16.dp))
    Column {
        Text(action, style = ButtonDefaults.textStyleFor(height), maxLines = 1)
        if (version.isNotEmpty()) {
            Text(version, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** Loader, Java and folder of the selected version. */
@Composable
private fun VersionFacts(version: InstalledVersion) {
    val scope = rememberCoroutineScope()
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        version.loader?.let { loader ->
            AssistChip(
                onClick = {},
                label = { Text("${loaderName(loader)} · ${version.gameVersion}") },
                leadingIcon = { Icon(painterResource(Res.drawable.ic_extension), null, Modifier.size(18.dp)) },
            )
        }
        AssistChip(
            onClick = {},
            label = { Text(stringResource(Res.string.launch_java, version.javaMajor.toString())) },
            leadingIcon = { Icon(painterResource(Res.drawable.ic_coffee), null, Modifier.size(18.dp)) },
        )
        AssistChip(
            onClick = { scope.launch { openFolder(version.runDir) } },
            label = { Text(stringResource(Res.string.launch_game_folder)) },
            leadingIcon = { Icon(painterResource(Res.drawable.ic_folder_open), null, Modifier.size(18.dp)) },
        )
    }
}

@Composable
private fun StatusBanner(state: GameState, onDismiss: () -> Unit, onShowOutput: () -> Unit) {
    val motion = MaterialTheme.motionScheme
    AnimatedContent(
        targetState = state,
        contentKey = { it::class },
        transitionSpec = {
            (fadeIn(motion.defaultEffectsSpec()) + expandVertically(motion.defaultSpatialSpec())) togetherWith
                (fadeOut(motion.fastEffectsSpec()) + shrinkVertically())
        },
        label = "status",
    ) { current ->
        when (current) {
            GameState.Idle -> Spacer(Modifier.fillMaxWidth())
            is GameState.Launching -> StatusRow(stringResource(Res.string.launch_starting, current.versionId), loading = true)
            is GameState.Running -> StatusRow(
                current.pid?.let { stringResource(Res.string.launch_running_pid, current.versionId, it.toString()) }
                    ?: stringResource(Res.string.launch_running, current.versionId),
                action = { TextButton(onClick = onShowOutput) { Text(stringResource(Res.string.launch_log)) } },
            )
            is GameState.Exited -> StatusRow(
                if (current.code == 0 || current.code == null) stringResource(Res.string.launch_exited, current.versionId)
                else stringResource(Res.string.launch_exited_code, current.versionId, current.code.toString()),
                action = {
                    TextButton(onClick = onShowOutput) { Text(stringResource(Res.string.launch_log)) }
                    TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_dismiss)) }
                },
            )
            is GameState.Failed -> StatusRow(
                current.message,
                error = true,
                action = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_dismiss)) } },
            )
        }
    }
}

@Composable
private fun StatusRow(
    text: String,
    loading: Boolean = false,
    error: Boolean = false,
    action: @Composable () -> Unit = {},
) {
    Surface(
        shape = MaterialTheme.shapes.large,
        color = if (error) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.secondaryContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            if (loading) {
                ContainedLoadingIndicator(Modifier.size(32.dp))
            } else if (!error) {
                LoadingIndicator(Modifier.size(32.dp))
            }
            Spacer(Modifier.width(12.dp))
            Text(
                text,
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f),
                color = if (error) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onSecondaryContainer,
            )
            action()
        }
    }
}

/** Settings of one version: its loader, Java and folder, and where it keeps saves and mods. */
@Composable
private fun VersionSettingsDialog(version: InstalledVersion, repository: SulfurRepository, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(painterResource(Res.drawable.ic_layers), null) },
        title = { Text(version.id) },
        text = {
            Column(Modifier.widthIn(max = 480.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                VersionFacts(version)
                Text(stringResource(Res.string.launch_saves_mods), style = MaterialTheme.typography.labelLarge)
                // Follow the global default, or override for this version.
                val current = if (version.isolationOverridden) version.isolation else null
                ConnectedChoice(
                    options = listOf(null, Isolation.ISOLATED, Isolation.SHARED),
                    selected = current,
                    label = {
                        stringResource(
                            when (it) {
                                null -> Res.string.isolation_default
                                Isolation.ISOLATED -> Res.string.isolation_isolated
                                Isolation.SHARED -> Res.string.isolation_shared
                            }
                        )
                    },
                    onSelect = { repository.setVersionIsolation(version.id, it) },
                )
                Text(
                    stringResource(if (version.isolation == Isolation.ISOLATED) Res.string.isolation_isolated_hint else Res.string.isolation_shared_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_close)) } },
    )
}

/** The game's recent output. */
@Composable
private fun OutputDialog(lines: List<String>, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(Res.string.launch_log)) },
        text = {
            val listState = rememberLazyListState()
            LaunchedEffect(lines.size) {
                if (lines.isNotEmpty()) listState.scrollToItem(lines.lastIndex)
            }
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = MaterialTheme.colorScheme.surfaceContainerHighest,
                modifier = Modifier.width(720.dp).height(420.dp),
            ) {
                if (lines.isEmpty()) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(stringResource(Res.string.launch_log_empty), style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    SelectionContainer {
                        LazyColumn(state = listState, modifier = Modifier.fillMaxSize().padding(12.dp)) {
                            items(lines) { line ->
                                Text(
                                    line,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                                    color = if ("ERROR" in line || "Exception" in line) MaterialTheme.colorScheme.error
                                    else MaterialTheme.colorScheme.onSurface,
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_close)) } },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    )
}

@Composable
private fun EmptyState(onBrowseVersions: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        ShapeIcon(
            painterResource(Res.drawable.ic_download),
            MaterialShapes.SoftBurst,
            size = 140.dp,
            container = MaterialTheme.colorScheme.primaryContainer,
            content = MaterialTheme.colorScheme.onPrimaryContainer,
            rotating = true,
        )
        Spacer(Modifier.height(24.dp))
        Text(stringResource(Res.string.launch_empty_title), style = MaterialTheme.typography.headlineMediumEmphasized)
        Spacer(Modifier.height(8.dp))
        Text(
            stringResource(Res.string.launch_empty_hint),
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(20.dp))
        FilledTonalButton(onClick = onBrowseVersions, shapes = ButtonDefaults.shapes()) {
            Text(stringResource(Res.string.action_browse_versions))
        }
    }
}
