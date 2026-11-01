package dev.sulfur.feature.download

import androidx.compose.animation.AnimatedContent
import dev.sulfur.ui.theme.Corners
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularWavyProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProgressIndicatorDefaults
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.isRunning
import dev.sulfur.ffi.Banner
import dev.sulfur.ffi.GroupKind
import dev.sulfur.ffi.InstallSnapshot
import dev.sulfur.ffi.RemoteVersion
import dev.sulfur.ffi.VersionGroup
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_close
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_install_options
import dev.sulfur.resources.action_refresh_versions
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.download_offline
import dev.sulfur.resources.download_search
import dev.sulfur.resources.download_title
import dev.sulfur.resources.group_alpha
import dev.sulfur.resources.group_beta
import dev.sulfur.resources.group_in_development
import dev.sulfur.resources.group_previews
import dev.sulfur.resources.group_releases
import dev.sulfur.resources.group_snapshots
import dev.sulfur.resources.group_title
import dev.sulfur.resources.ic_arrow_drop_down
import dev.sulfur.resources.ic_check_circle
import dev.sulfur.resources.ic_close
import dev.sulfur.resources.ic_cloud_off
import dev.sulfur.resources.ic_deployed_code
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_extension
import dev.sulfur.ffi.VersionKind
import dev.sulfur.resources.ic_inventory_2
import dev.sulfur.resources.download_section_modpacks
import dev.sulfur.feature.modpacks.ModpacksViewModel
import dev.sulfur.feature.modpacks.ModpacksPane
import dev.sulfur.resources.install_with_loader
import dev.sulfur.resources.installed
import dev.sulfur.resources.n_installed
import dev.sulfur.resources.n_versions
import dev.sulfur.resources.no_results
import dev.sulfur.feature.mods.ModsPane
import dev.sulfur.feature.mods.ModsViewModel
import dev.sulfur.resources.download_section_game
import dev.sulfur.resources.download_section_mods
import dev.sulfur.resources.download_section_archive
import dev.sulfur.resources.group_april_fools
import dev.sulfur.resources.ic_history_edu
import dev.sulfur.resources.update_1_0
import dev.sulfur.resources.update_1_4
import dev.sulfur.resources.update_1_5
import dev.sulfur.resources.update_1_6
import dev.sulfur.resources.update_1_7
import dev.sulfur.resources.update_1_8
import dev.sulfur.resources.update_1_9
import dev.sulfur.resources.update_1_10
import dev.sulfur.resources.update_1_11
import dev.sulfur.resources.update_1_12
import dev.sulfur.resources.update_1_13
import dev.sulfur.resources.update_1_14
import dev.sulfur.resources.update_1_15
import dev.sulfur.resources.update_1_16
import dev.sulfur.resources.update_1_17
import dev.sulfur.resources.update_1_18
import dev.sulfur.resources.update_1_19
import dev.sulfur.resources.update_1_20
import dev.sulfur.resources.update_1_21
import dev.sulfur.resources.update_26_1
import dev.sulfur.resources.update_26_2
import dev.sulfur.resources.update_26_3
import org.jetbrains.compose.resources.StringResource
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.PageHeader
import dev.sulfur.ui.components.PagePadding
import dev.sulfur.ui.components.SearchToolbar
import dev.sulfur.ui.components.RefreshButton
import dev.sulfur.ui.components.Notice
import dev.sulfur.ui.components.RemoteImage
import dev.sulfur.ui.components.Logo
import dev.sulfur.ui.components.minecraftLogo
import dev.sulfur.ui.components.formatDate
import dev.sulfur.ui.components.kindLabel
import dev.sulfur.ui.components.kindShape
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.pluralStringResource
import org.jetbrains.compose.resources.stringResource

/** What a version row or card needs to know about installs. */
internal class InstallState(
    val installedIds: Set<String>,
    val active: Map<String, InstallSnapshot>,
)

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun DownloadScreen(viewModel: DownloadViewModel, mods: ModsViewModel, modpacks: ModpacksViewModel, repository: SulfurRepository) {
    val ui by viewModel.ui.collectAsState()
    val installs by repository.installs.collectAsState()
    val installed by repository.installed.collectAsState()
    val state = remember(installed, installs) { InstallState(installed.map { it.id }.toSet(), installs.associateBy { it.versionId }) }
    val open = ui.openGroup
    val target = ui.installTarget
    val cardOpen = open != null || target != null
    // Keep the floating cards' content while they animate away.
    var shownGroup by remember { mutableStateOf<VersionGroup?>(null) }
    ui.groups.firstOrNull { it.key == open }?.let { shownGroup = it }
    var shownTarget by remember { mutableStateOf<String?>(null) }
    if (target != null) shownTarget = target
    val cleanroomDirect = repository.settings.collectAsState().value.cleanroomDirect
    val page = @Composable { version: String, closes: Boolean ->
        VersionInstallPage(
            gameVersion = version,
            listed = ui.versions.firstOrNull { it.id == version },
            installedIds = state.installedIds,
            cleanroomDirect = cleanroomDirect,
            closes = closes,
            initialLoader = ui.installLoader,
            loadVersions = viewModel::loaderVersions,
            onBack = { viewModel.showInstallOptions(null) },
            onInstall = viewModel::install,
        )
    }

    // Read in the layer below, so the animation redraws the page instead of recomposing it.
    val blur = animateDpAsState(if (cardOpen) 14.dp else 0.dp, MaterialTheme.motionScheme.defaultEffectsSpec())

    SharedTransitionLayout(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize()) {
            Column(
                Modifier.fillMaxSize().graphicsLayer {
                    val radius = blur.value.toPx()
                    renderEffect = if (radius > 0.5f) BlurEffect(radius, radius, TileMode.Clamp) else null
                    clip = radius > 0.5f
                }.padding(horizontal = PagePadding, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(CardSpacing),
            ) {
                PageHeader(stringResource(Res.string.download_title)) {
                    ConnectedChoice(
                        options = DownloadSection.entries,
                        selected = ui.section,
                        label = {
                            stringResource(
                                when (it) {
                                    DownloadSection.Game -> Res.string.download_section_game
                                    DownloadSection.Mods -> Res.string.download_section_mods
                                    DownloadSection.Modpacks -> Res.string.download_section_modpacks
                                    DownloadSection.Archive -> Res.string.download_section_archive
                                }
                            )
                        },
                        onSelect = viewModel::showSection,
                        fill = false,
                        icon = {
                            when (it) {
                                DownloadSection.Game -> Res.drawable.ic_deployed_code
                                DownloadSection.Mods -> Res.drawable.ic_extension
                                DownloadSection.Modpacks -> Res.drawable.ic_inventory_2
                                DownloadSection.Archive -> Res.drawable.ic_history_edu
                            }
                        },
                    )
                }

                // Every section below starts with the same toolbar (search, then its actions),
                // then its filters and notices, then its results.
                if (ui.section == DownloadSection.Mods) {
                    ModsPane(mods, repository, Modifier.fillMaxWidth().weight(1f))
                } else if (ui.section == DownloadSection.Modpacks) {
                    ModpacksPane(modpacks, Modifier.fillMaxWidth().weight(1f))
                } else if (ui.section == DownloadSection.Archive) {
                    ArchivePane(ui.archive, state, viewModel, Modifier.fillMaxWidth().weight(1f))
                } else {
                SearchToolbar(ui.query, viewModel::setQuery, stringResource(Res.string.download_search)) {
                    RefreshButton(ui.loading, stringResource(Res.string.action_refresh_versions), onClick = { viewModel.refresh() })
                }
                if (ui.error != null || ui.fromCache) {
                    OfflineBanner(ui.error, onRetry = { viewModel.refresh() })
                }

                if (ui.query.isBlank()) {
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(minSize = 196.dp),
                        modifier = Modifier.fillMaxWidth().weight(1f),
                        contentPadding = PaddingValues(bottom = 16.dp),
                        horizontalArrangement = Arrangement.spacedBy(CardSpacing),
                        verticalArrangement = Arrangement.spacedBy(CardSpacing),
                    ) {
                        items(ui.groups, key = { it.key }) { group ->
                            val installedCount = (group.releases + group.previews).count { it.id in state.installedIds }
                            // An invisible copy holds the slot while the card floats, so the grid does not reflow.
                            Box {
                                GroupCard(group, ui.banners[group.key], installedCount, onClick = {}, modifier = Modifier.alpha(0f))
                                androidx.compose.animation.AnimatedVisibility(
                                    visible = open != group.key,
                                    enter = fadeIn(MaterialTheme.motionScheme.defaultEffectsSpec()),
                                    exit = fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()),
                                ) {
                                    GroupCard(
                                        group = group,
                                        banner = ui.banners[group.key],
                                        installedCount = installedCount,
                                        onClick = { viewModel.openGroup(group.key) },
                                        modifier = Modifier.sharedBounds(
                                            rememberSharedContentState("group-${group.key}"),
                                            animatedVisibilityScope = this,
                                        ),
                                    )
                                }
                            }
                        }
                    }
                } else {
                    // Update and channel names in the UI language and in English, so either finds a version.
                    val groupLabels = ui.groups.associate { g ->
                        g.key to listOfNotNull(groupTitle(g), groupSubtitle(g), g.name, g.kind.name.replace('_', ' ')).joinToString(" ")
                    }
                    val kindLabels = VersionKind.entries.associateWith { "${kindLabel(it)} ${it.name.replace('_', ' ')}" }
                    val groupOf = remember(ui.groups) { ui.groups.flatMap { g -> (g.releases + g.previews).map { it.id to g.key } }.toMap() }
                    val results = remember(ui.query, ui.versions, groupLabels, kindLabels) {
                        searchVersions(ui.query, ui.versions) { v -> "${groupOf[v.id]?.let(groupLabels::get).orEmpty()} ${kindLabels[v.kind]}" }
                    }
                    LazyColumn(
                        Modifier.fillMaxWidth().weight(1f),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                        contentPadding = PaddingValues(bottom = 16.dp),
                    ) {
                        if (results.isEmpty()) {
                            item { Text(stringResource(Res.string.no_results), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        }
                        items(results, key = { it.id }) { version ->
                            VersionRow(version, state, viewModel, Modifier.animateItem())
                        }
                    }
                }
                }
            }

            // Scrim: dims the blurred page and closes the card when clicked.
            AnimatedVisibility(cardOpen, enter = fadeIn(), exit = fadeOut()) {
                Box(
                    Modifier.fillMaxSize()
                        .background(MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f))
                        .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {
                            viewModel.closeCards()
                        }
                )
            }
            AnimatedVisibility(
                visible = open != null,
                enter = fadeIn(MaterialTheme.motionScheme.defaultEffectsSpec()),
                exit = fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()),
                modifier = Modifier.align(Alignment.Center),
            ) {
                shownGroup?.let { group ->
                    GroupDetail(
                        group = group,
                        banner = ui.banners[group.key],
                        state = state,
                        viewModel = viewModel,
                        target = target,
                        page = { page(it, false) },
                        animatedVisibilityScope = this,
                    )
                }
            }
            // A version opened from search results, outside any update's card.
            AnimatedVisibility(
                visible = open == null && target != null,
                enter = fadeIn(MaterialTheme.motionScheme.defaultEffectsSpec()) + scaleIn(initialScale = 0.94f),
                exit = fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()) + scaleOut(targetScale = 0.94f),
                modifier = Modifier.align(Alignment.Center),
            ) {
                shownTarget?.let { version ->
                    FloatingCard(onEscape = { viewModel.showInstallOptions(null) }) { page(version, true) }
                }
            }
        }
    }
}

/** A card above the blurred page, closed with Esc. */
@Composable
private fun FloatingCard(onEscape: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Surface(
        modifier = modifier
            .widthIn(max = 760.dp)
            .fillMaxWidth(0.88f)
            .fillMaxHeight(0.9f)
            .focusRequester(focus)
            .focusable()
            .onKeyEvent { event ->
                if (event.key == Key.Escape) {
                    onEscape()
                    true
                } else false
            },
        shape = Corners.page,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shadowElevation = 8.dp,
    ) { content() }
}

@Composable
private fun OfflineBanner(error: String?, onRetry: () -> Unit) {
    Notice(
        painterResource(Res.drawable.ic_cloud_off),
        error ?: stringResource(Res.string.download_offline),
        container = MaterialTheme.colorScheme.errorContainer,
        content = MaterialTheme.colorScheme.onErrorContainer,
    ) {
        TextButton(onClick = onRetry) { Text(stringResource(Res.string.action_retry)) }
    }
}

@Composable
private fun groupTitle(group: VersionGroup): String = when {
    group.kind == GroupKind.BETA -> stringResource(Res.string.group_beta)
    group.kind == GroupKind.ALPHA -> stringResource(Res.string.group_alpha)
    group.kind == GroupKind.APRIL_FOOLS -> stringResource(Res.string.group_april_fools)
    group.key == "upcoming" -> stringResource(Res.string.group_snapshots)
    else -> stringResource(Res.string.group_title, group.key)
}

@Composable
private fun groupSubtitle(group: VersionGroup): String? = when (group.kind) {
    GroupKind.UPCOMING -> UpdateNames[group.key]?.let { stringResource(it) } ?: group.name ?: stringResource(Res.string.group_in_development)
    GroupKind.RELEASE -> UpdateNames[group.key]?.let { stringResource(it) } ?: group.name
    // Rust names these in English; the title already says it in the UI language.
    else -> null
}

/** Official names of major updates, translated; the core knows them in English only. */
private val UpdateNames: Map<String, StringResource> = mapOf(
    "26.3" to Res.string.update_26_3,
    "26.2" to Res.string.update_26_2,
    "26.1" to Res.string.update_26_1,
    "1.21" to Res.string.update_1_21,
    "1.20" to Res.string.update_1_20,
    "1.19" to Res.string.update_1_19,
    "1.18" to Res.string.update_1_18,
    "1.17" to Res.string.update_1_17,
    "1.16" to Res.string.update_1_16,
    "1.15" to Res.string.update_1_15,
    "1.14" to Res.string.update_1_14,
    "1.13" to Res.string.update_1_13,
    "1.12" to Res.string.update_1_12,
    "1.11" to Res.string.update_1_11,
    "1.10" to Res.string.update_1_10,
    "1.9" to Res.string.update_1_9,
    "1.8" to Res.string.update_1_8,
    "1.7" to Res.string.update_1_7,
    "1.6" to Res.string.update_1_6,
    "1.5" to Res.string.update_1_5,
    "1.4" to Res.string.update_1_4,
    "1.0" to Res.string.update_1_0,
)

/** Color pair for groups without artwork, varied by key so the grid stays lively. */
@Composable
private fun fillColors(key: String): Pair<Color, Color> {
    val c = MaterialTheme.colorScheme
    return when (Math.floorMod(key.hashCode(), 3)) {
        0 -> c.primaryContainer to c.onPrimaryContainer
        1 -> c.secondaryContainer to c.onSecondaryContainer
        else -> c.tertiaryContainer to c.onTertiaryContainer
    }
}

@Composable
private fun GroupCard(
    group: VersionGroup,
    banner: Banner?,
    installedCount: Int,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val (fill, onFill) = fillColors(group.key)
    Surface(
        onClick = onClick,
        modifier = modifier.fillMaxWidth(),
        shape = Corners.card,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Column {
            RemoteImage(
                url = banner?.previewUrl ?: banner?.imageUrl,
                placeholder = fill,
                modifier = Modifier.fillMaxWidth().aspectRatio(16f / 10f).clip(Corners.card),
            ) {
                Text(group.key.takeIf { group.kind == GroupKind.RELEASE || group.kind == GroupKind.UPCOMING } ?: groupTitle(group),
                    style = MaterialTheme.typography.displaySmallEmphasized, color = onFill, maxLines = 1)
            }
            Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(groupTitle(group), style = MaterialTheme.typography.titleMediumEmphasized, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        groupSubtitle(group) ?: pluralStringResource(Res.plurals.n_versions, group.releases.size + group.previews.size, group.releases.size + group.previews.size),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (installedCount > 0) {
                    Icon(
                        painterResource(Res.drawable.ic_check_circle),
                        contentDescription = pluralStringResource(Res.plurals.n_installed, installedCount, installedCount),
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
    }
}

/** The floating card: artwork, summary, and every version of the group. */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun SharedTransitionScope.GroupDetail(
    group: VersionGroup,
    banner: Banner?,
    state: InstallState,
    viewModel: DownloadViewModel,
    /** A version of this update opened to install; replaces the list. */
    target: String?,
    page: @Composable (String) -> Unit,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(group.key) { runCatching { focus.requestFocus() } }
    var showPreviews by rememberSaveable(group.key) { mutableStateOf(group.releases.isEmpty()) }
    val (fill, onFill) = fillColors(group.key)
    Surface(
        modifier = Modifier
            .sharedBounds(rememberSharedContentState("group-${group.key}"), animatedVisibilityScope)
            .widthIn(max = 760.dp)
            .fillMaxWidth(0.88f)
            .fillMaxHeight(0.9f)
            .focusRequester(focus)
            .focusable()
            .onKeyEvent { event ->
                if (event.key == Key.Escape) {
                    if (target != null) viewModel.showInstallOptions(null) else viewModel.openGroup(null)
                    true
                } else false
            },
        shape = Corners.page,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shadowElevation = 8.dp,
    ) {
      val motion = MaterialTheme.motionScheme
      AnimatedContent(
        targetState = target,
        transitionSpec = {
            val forward = targetState != null
            (fadeIn(motion.defaultEffectsSpec()) + slideInHorizontally(motion.defaultSpatialSpec()) { if (forward) it / 6 else -it / 6 }) togetherWith
                (fadeOut(motion.fastEffectsSpec()) + slideOutHorizontally(motion.defaultSpatialSpec()) { if (forward) -it / 6 else it / 6 })
        },
        label = "group-page",
      ) { shown ->
        if (shown != null) {
            page(shown)
            return@AnimatedContent
        }
        Column {
            Box(Modifier.fillMaxWidth().height(200.dp)) {
                RemoteImage(banner?.imageUrl, Modifier.matchParentSize(), placeholder = fill)
                // Keeps the title readable on any artwork.
                Box(
                    Modifier.matchParentSize().background(
                        Brush.verticalGradient(0.35f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.7f))
                    )
                )
                Column(Modifier.align(Alignment.BottomStart).padding(20.dp)) {
                    Text(groupTitle(group), style = MaterialTheme.typography.headlineMediumEmphasized, color = Color.White)
                    groupSubtitle(group)?.let {
                        Text(it, style = MaterialTheme.typography.titleMedium, color = Color.White.copy(alpha = 0.9f))
                    }
                }
                FilledTonalIconButton(
                    onClick = { viewModel.openGroup(null) },
                    modifier = Modifier.align(Alignment.TopEnd).padding(12.dp),
                ) {
                    Icon(painterResource(Res.drawable.ic_close), stringResource(Res.string.action_close))
                }
                if (banner?.imageUrl == null) {
                    Text(group.key, style = MaterialTheme.typography.displayMediumEmphasized, color = onFill.copy(alpha = 0.4f),
                        modifier = Modifier.align(Alignment.CenterEnd).padding(end = 32.dp))
                }
            }
            Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                banner?.summary?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                if (group.releases.isNotEmpty() && group.previews.isNotEmpty()) {
                    ConnectedChoice(
                        options = listOf(false, true),
                        selected = showPreviews,
                        label = { previews ->
                            if (previews) stringResource(Res.string.group_previews, group.previews.size)
                            else stringResource(Res.string.group_releases, group.releases.size)
                        },
                        onSelect = { showPreviews = it },
                        fill = false,
                    )
                }
            }
            LazyColumn(
                Modifier.fillMaxWidth().weight(1f),
                contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                items(if (showPreviews) group.previews else group.releases, key = { it.id }) { version ->
                    VersionRow(version, state, viewModel)
                }
            }
        }
      }
    }
}

@Composable
private fun VersionRow(
    version: RemoteVersion,
    state: InstallState,
    viewModel: DownloadViewModel,
    modifier: Modifier = Modifier,
) {
    // The row opens the version's install options; its button installs the game as it is.
    Surface(
        onClick = { viewModel.showInstallOptions(version.id) },
        modifier = modifier.fillMaxWidth(),
        shape = Corners.item,
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Logo(minecraftLogo(version.kind), 32.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(version.id, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    "${kindLabel(version.kind)} · ${formatDate(version.releaseTime)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            InstallAction(
                installed = version.id in state.installedIds,
                active = state.active[version.id],
                onInstall = { viewModel.install(version.id) },
                onOptions = { viewModel.showInstallOptions(version.id) },
            )
        }
    }
}

/** Install split button that morphs into progress, then into an "Installed" chip. */
@Composable
private fun InstallAction(
    installed: Boolean,
    active: InstallSnapshot?,
    onInstall: () -> Unit,
    onOptions: () -> Unit,
) {
    val target = when {
        active?.isRunning == true -> 1
        installed -> 2
        else -> 0
    }
    val motion = MaterialTheme.motionScheme
    AnimatedContent(
        targetState = target,
        transitionSpec = {
            (fadeIn(motion.defaultEffectsSpec()) + scaleIn(motion.fastSpatialSpec())) togetherWith
                (fadeOut(motion.fastEffectsSpec()) + scaleOut())
        },
        contentAlignment = Alignment.CenterEnd,
        label = "install",
    ) { s ->
        when (s) {
            1 -> Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                val progress by animateFloatAsState(
                    (active?.fraction ?: 0.0).toFloat(),
                    ProgressIndicatorDefaults.ProgressAnimationSpec,
                )
                CircularWavyProgressIndicator(progress = { progress }, modifier = Modifier.size(32.dp))
            }
            2 -> Row(verticalAlignment = Alignment.CenterVertically) {
                AssistChip(
                    onClick = {},
                    label = { Text(stringResource(Res.string.installed)) },
                    leadingIcon = {
                        Icon(painterResource(Res.drawable.ic_check_circle), null, Modifier.size(AssistChipDefaults.IconSize))
                    },
                )
                IconButton(onClick = onOptions) {
                    Icon(painterResource(Res.drawable.ic_extension), stringResource(Res.string.install_with_loader))
                }
            }
            else -> InstallSplitButton(onInstall, onOptions)
        }
    }
}

/** "Install" on the leading half; the trailing half offers mod loaders. */
@Composable
private fun InstallSplitButton(onInstall: () -> Unit, onOptions: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    SplitButtonLayout(
        leadingButton = {
            SplitButtonDefaults.TonalLeadingButton(onClick = onInstall) {
                Icon(painterResource(Res.drawable.ic_download), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(Res.string.action_install))
            }
        },
        trailingButton = {
            Box {
                SplitButtonDefaults.TonalTrailingButton(checked = menu, onCheckedChange = { menu = it }) {
                    val rotation by animateFloatAsState(if (menu) 180f else 0f, MaterialTheme.motionScheme.defaultSpatialSpec())
                    Icon(
                        painterResource(Res.drawable.ic_arrow_drop_down),
                        contentDescription = stringResource(Res.string.action_install_options),
                        modifier = Modifier.size(ButtonDefaults.IconSize).rotate(rotation),
                    )
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(Res.string.install_with_loader)) },
                        leadingIcon = { Icon(painterResource(Res.drawable.ic_extension), null) },
                        onClick = {
                            menu = false
                            onOptions()
                        },
                    )
                }
            }
        },
    )
}
