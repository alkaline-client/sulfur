package dev.sulfur.feature.settings

import androidx.compose.animation.core.animateDpAsState
import dev.sulfur.ui.theme.Corners
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.ThemeColors
import dev.sulfur.ffi.ColorSource
import dev.sulfur.ffi.GarbageCollector
import androidx.compose.ui.graphics.graphicsLayer
import dev.sulfur.resources.ic_science
import dev.sulfur.resources.settings_advanced
import dev.sulfur.resources.settings_background_use_wallpaper
import dev.sulfur.resources.settings_color_background_hint
import dev.sulfur.resources.settings_color_background_none
import dev.sulfur.resources.settings_color_source_seed
import dev.sulfur.resources.settings_color_source_wallpaper
import dev.sulfur.resources.settings_color_wallpaper_hint
import dev.sulfur.resources.settings_color_wallpaper_none
import dev.sulfur.resources.settings_game_args
import dev.sulfur.resources.settings_min_memory
import dev.sulfur.resources.settings_min_memory_hint
import dev.sulfur.feature.misc.MiscViewModel
import dev.sulfur.ffi.DownloadSource
import dev.sulfur.ffi.Isolation
import dev.sulfur.ffi.Settings
import dev.sulfur.ffi.ThemeMode
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_add
import dev.sulfur.resources.action_remove_folder
import dev.sulfur.resources.ic_brightness_auto
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.ic_dark_mode
import dev.sulfur.resources.ic_delete
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_extension
import dev.sulfur.resources.ic_light_mode
import dev.sulfur.resources.ic_link
import dev.sulfur.resources.ic_palette
import dev.sulfur.resources.ic_stack
import dev.sulfur.resources.settings_appearance
import dev.sulfur.resources.settings_color
import dev.sulfur.resources.settings_connections
import dev.sulfur.resources.settings_curseforge_key
import dev.sulfur.resources.settings_curseforge_key_hint
import dev.sulfur.resources.settings_download
import dev.sulfur.resources.settings_fullscreen
import dev.sulfur.resources.settings_game
import dev.sulfur.resources.settings_game_dir
import dev.sulfur.resources.settings_game_dir_default
import dev.sulfur.resources.settings_game_dir_hint
import dev.sulfur.resources.settings_height
import dev.sulfur.resources.settings_high_contrast
import dev.sulfur.resources.settings_high_contrast_hint
import dev.sulfur.resources.settings_isolation
import dev.sulfur.resources.settings_isolation_hint
import dev.sulfur.resources.settings_isolation_isolated
import dev.sulfur.resources.settings_isolation_shared
import dev.sulfur.resources.settings_java
import dev.sulfur.resources.settings_java_auto
import dev.sulfur.resources.settings_jvm_args
import dev.sulfur.resources.settings_memory
import dev.sulfur.resources.settings_reuse
import dev.sulfur.resources.settings_reuse_another
import dev.sulfur.resources.settings_reuse_hint
import dev.sulfur.resources.settings_reuse_official
import dev.sulfur.resources.settings_reuse_official_hint
import dev.sulfur.resources.settings_segments
import dev.sulfur.resources.settings_source
import dev.sulfur.resources.settings_source_auto
import dev.sulfur.resources.settings_source_auto_hint
import dev.sulfur.resources.settings_source_mirror
import dev.sulfur.resources.settings_source_mirror_hint
import dev.sulfur.resources.settings_source_official
import dev.sulfur.resources.settings_source_official_hint
import dev.sulfur.resources.settings_theme
import dev.sulfur.resources.settings_theme_dark
import dev.sulfur.resources.settings_theme_light
import dev.sulfur.resources.settings_theme_system
import dev.sulfur.resources.settings_title
import dev.sulfur.resources.settings_width
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.PageHeader
import dev.sulfur.ui.components.PagePadding
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.theme.SeedColors
import dev.sulfur.ui.theme.seedColor
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.util.Locale
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.LoadingIndicator
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import dev.sulfur.data.isRunning
import dev.sulfur.ui.components.SideNavItem
import dev.sulfur.data.rememberBackgroundImage
import dev.sulfur.ffi.NavigationPosition
import dev.sulfur.resources.ic_check
import dev.sulfur.resources.ic_deployed_code
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.ic_layers
import dev.sulfur.resources.ic_memory
import dev.sulfur.resources.ic_tune
import dev.sulfur.resources.ic_widgets
import dev.sulfur.resources.settings_background
import dev.sulfur.resources.settings_background_blur
import dev.sulfur.resources.settings_background_choose
import dev.sulfur.resources.settings_background_none
import dev.sulfur.resources.settings_background_opacity
import dev.sulfur.resources.settings_background_remove
import dev.sulfur.resources.settings_cleanroom
import dev.sulfur.resources.settings_cleanroom_direct
import dev.sulfur.resources.settings_cleanroom_direct_hint
import dev.sulfur.resources.settings_game_folder
import dev.sulfur.resources.settings_java_auto_download
import dev.sulfur.resources.settings_java_auto_download_hint
import dev.sulfur.resources.settings_java_download
import dev.sulfur.resources.settings_java_downloaded
import dev.sulfur.resources.settings_java_runtime
import dev.sulfur.resources.settings_jvm
import dev.sulfur.resources.settings_layout
import dev.sulfur.resources.settings_log4j
import dev.sulfur.resources.settings_log4j_hint
import dev.sulfur.resources.settings_mods_and_resources
import dev.sulfur.resources.settings_navigation
import dev.sulfur.resources.settings_navigation_side
import dev.sulfur.resources.settings_navigation_top
import dev.sulfur.resources.settings_theme_and_color
import dev.sulfur.resources.settings_window
import dev.sulfur.util.chooseFile
import dev.sulfur.util.chooseFolder
import dev.sulfur.data.UiMessage
import dev.sulfur.ffi.ForeignConfig
import dev.sulfur.ffi.ForeignLauncher
import androidx.compose.material3.Surface
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import dev.sulfur.resources.action_import
import dev.sulfur.resources.ic_history
import dev.sulfur.resources.ic_public
import dev.sulfur.resources.launch_offline_name
import dev.sulfur.resources.msg_imported
import dev.sulfur.resources.settings_import
import dev.sulfur.resources.settings_import_choose
import dev.sulfur.resources.settings_import_game_dirs
import dev.sulfur.resources.settings_import_hint
import dev.sulfur.resources.settings_import_none
import dev.sulfur.resources.settings_import_use_folder
import dev.sulfur.resources.settings_import_use_folder_hint
import dev.sulfur.resources.settings_import_window
import dev.sulfur.resources.settings_language
import dev.sulfur.resources.settings_language_system
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.StringResource
import java.io.File
import java.text.NumberFormat
import kotlin.math.roundToInt
import androidx.compose.material3.OutlinedButton
import dev.sulfur.ffi.CornerStyle
import dev.sulfur.resources.action_choose_folder
import dev.sulfur.resources.ic_bolt
import dev.sulfur.resources.settings_alkaline
import dev.sulfur.resources.settings_alkaline_choose
import dev.sulfur.resources.settings_alkaline_dir
import dev.sulfur.resources.settings_alkaline_found
import dev.sulfur.resources.settings_alkaline_missing
import dev.sulfur.resources.settings_corners
import dev.sulfur.resources.settings_corners_default
import dev.sulfur.resources.settings_corners_hint
import dev.sulfur.resources.settings_corners_round
import dev.sulfur.resources.settings_corners_small
import dev.sulfur.resources.settings_corners_square
import dev.sulfur.resources.settings_seasonal
import dev.sulfur.resources.settings_seasonal_hint
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import dev.sulfur.ffi.AlkalineRelease
import dev.sulfur.resources.action_install
import dev.sulfur.resources.settings_alkaline_check
import dev.sulfur.resources.settings_alkaline_check_failed
import dev.sulfur.resources.settings_alkaline_checking
import dev.sulfur.resources.settings_alkaline_feed
import dev.sulfur.resources.settings_alkaline_feed_default
import dev.sulfur.resources.settings_alkaline_installed
import dev.sulfur.resources.settings_alkaline_latest
import dev.sulfur.resources.settings_alkaline_no_download
import dev.sulfur.resources.settings_alkaline_no_release
import dev.sulfur.resources.settings_alkaline_not_installed
import dev.sulfur.resources.settings_alkaline_notes
import dev.sulfur.resources.settings_alkaline_update
import dev.sulfur.ui.components.RefreshButton
import dev.sulfur.util.openUrl
import kotlinx.coroutines.delay

private typealias Update = ((Settings) -> Settings) -> Unit

/** Settings are grouped into categories, listed on the left. */
private enum class Category(val title: StringResource, val icon: DrawableResource) {
    Appearance(Res.string.settings_appearance, Res.drawable.ic_palette),
    Game(Res.string.settings_game, Res.drawable.ic_stack),
    Java(Res.string.settings_java, Res.drawable.ic_coffee),
    Download(Res.string.settings_download, Res.drawable.ic_download),
    Mods(Res.string.settings_mods_and_resources, Res.drawable.ic_extension),
}

@Composable
fun SettingsScreen(repository: SulfurRepository, misc: MiscViewModel, themeColors: ThemeColors) {
    val settings by repository.settings.collectAsState()
    val update: Update = repository::updateSettings
    var category by rememberSaveable { mutableStateOf(Category.Appearance) }

    Row(Modifier.fillMaxSize().padding(horizontal = PagePadding, vertical = 12.dp)) {
        Column(Modifier.width(200.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            PageHeader(stringResource(Res.string.settings_title))
            Spacer(Modifier.height(8.dp))
            Category.entries.forEach { item ->
                SideNavItem(stringResource(item.title), item.icon, selected = item == category, onClick = { category = item })
            }
        }
        Spacer(Modifier.width(CardSpacing))
        val motion = MaterialTheme.motionScheme
        AnimatedContent(
            targetState = category,
            transitionSpec = { fadeIn(motion.defaultEffectsSpec()) togetherWith fadeOut(motion.fastEffectsSpec()) },
            modifier = Modifier.weight(1f).fillMaxHeight(),
            label = "category",
        ) { shown ->
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(top = 60.dp, bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(CardSpacing),
            ) {
                when (shown) {
                    Category.Appearance -> {
                        LanguageSection(settings, update)
                        ThemeSection(settings, update, themeColors)
                        LayoutSection(settings, update)
                        BackgroundSection(settings, update, repository)
                    }
                    Category.Game -> {
                        GameFolderSection(settings, update)
                        WindowSection(settings, update)
                        AlkalineSection(settings, update, repository)
                        ImportSection(repository)
                    }
                    Category.Java -> {
                        JavaRuntimeSection(settings, update, misc, repository)
                        JvmSection(settings, update)
                        AdvancedSection(settings, update)
                    }
                    Category.Download -> {
                        DownloadSection(settings, update)
                        ReuseSection(settings, update)
                    }
                    Category.Mods -> {
                        ModsSection(settings, update)
                        CleanroomSection(settings, update)
                    }
                }
            }
        }
    }
}

/** Languages with a translation, by code; names are written in their own language. */
private val Languages = listOf("en" to "English", "zh-CN" to "简体中文", "zh-TW" to "繁體中文")

@Composable
private fun LanguageSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_language), icon = painterResource(Res.drawable.ic_public)) {
        ConnectedChoice(
            options = listOf("") + Languages.map { it.first },
            selected = settings.language,
            label = { code -> Languages.firstOrNull { it.first == code }?.second ?: stringResource(Res.string.settings_language_system) },
            onSelect = { code -> update { it.copy(language = code) } },
        )
    }
}

@Composable
private fun ThemeSection(settings: Settings, update: Update, themeColors: ThemeColors) {
    SectionCard(stringResource(Res.string.settings_theme_and_color), icon = painterResource(Res.drawable.ic_palette)) {
        Label(stringResource(Res.string.settings_theme))
        ConnectedChoice(
            options = listOf(ThemeMode.SYSTEM, ThemeMode.LIGHT, ThemeMode.DARK),
            selected = settings.theme,
            label = { mode ->
                stringResource(
                    when (mode) {
                        ThemeMode.SYSTEM -> Res.string.settings_theme_system
                        ThemeMode.LIGHT -> Res.string.settings_theme_light
                        ThemeMode.DARK -> Res.string.settings_theme_dark
                    }
                )
            },
            icon = { mode ->
                when (mode) {
                    ThemeMode.SYSTEM -> Res.drawable.ic_brightness_auto
                    ThemeMode.LIGHT -> Res.drawable.ic_light_mode
                    ThemeMode.DARK -> Res.drawable.ic_dark_mode
                }
            },
            onSelect = { mode -> update { it.copy(theme = mode) } },
        )
        Label(stringResource(Res.string.settings_color))
        ConnectedChoice(
            options = ColorSource.entries,
            selected = settings.colorSource,
            label = {
                stringResource(
                    when (it) {
                        ColorSource.SEED -> Res.string.settings_color_source_seed
                        ColorSource.WALLPAPER -> Res.string.settings_color_source_wallpaper
                        ColorSource.BACKGROUND -> Res.string.settings_background
                    }
                )
            },
            onSelect = { source -> update { it.copy(colorSource = source) } },
            fill = false,
        )
        if (settings.colorSource == ColorSource.SEED) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SeedColors.forEach { rgb ->
                    val selected = settings.seedColor.toLong() == rgb
                    // The chosen swatch turns into a spinning expressive cookie shape.
                    val size by animateDpAsState(if (selected) 40.dp else 32.dp, MaterialTheme.motionScheme.fastSpatialSpec())
                    val turn by animateFloatAsState(if (selected) 90f else 0f, MaterialTheme.motionScheme.defaultSpatialSpec())
                    Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                        Box(
                            Modifier.size(size).graphicsLayer { rotationZ = turn }
                                .clip((if (selected) MaterialShapes.Cookie9Sided else MaterialShapes.Circle).toShape())
                                .background(seedColor(rgb))
                                .clickable { update { it.copy(seedColor = rgb.toUInt()) } }
                        )
                    }
                }
            }
        } else {
            val sampled by themeColors.sampled.collectAsState()
            val color = sampled[settings.colorSource]
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(
                    Modifier.size(40.dp).clip(MaterialShapes.Cookie9Sided.toShape())
                        .background(seedColor(color ?: settings.seedColor.toLong()))
                )
                Hint(
                    stringResource(
                        when {
                            settings.colorSource == ColorSource.WALLPAPER && color != null -> Res.string.settings_color_wallpaper_hint
                            settings.colorSource == ColorSource.WALLPAPER -> Res.string.settings_color_wallpaper_none
                            color != null -> Res.string.settings_color_background_hint
                            else -> Res.string.settings_color_background_none
                        }
                    )
                )
            }
        }
        SwitchRow(
            title = stringResource(Res.string.settings_high_contrast),
            subtitle = stringResource(Res.string.settings_high_contrast_hint),
            checked = settings.highContrast,
            onChange = { value -> update { it.copy(highContrast = value) } },
        )
    }
}

@Composable
private fun LayoutSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_layout), icon = painterResource(Res.drawable.ic_widgets)) {
        Label(stringResource(Res.string.settings_navigation))
        ConnectedChoice(
            options = listOf(NavigationPosition.TOP, NavigationPosition.SIDE),
            selected = settings.navigation,
            label = {
                stringResource(if (it == NavigationPosition.TOP) Res.string.settings_navigation_top else Res.string.settings_navigation_side)
            },
            onSelect = { value -> update { it.copy(navigation = value) } },
        )
        Label(stringResource(Res.string.settings_corners))
        ConnectedChoice(
            options = CornerStyle.entries,
            selected = settings.corners,
            label = {
                stringResource(
                    when (it) {
                        CornerStyle.SQUARE -> Res.string.settings_corners_square
                        CornerStyle.SMALL -> Res.string.settings_corners_small
                        CornerStyle.DEFAULT -> Res.string.settings_corners_default
                        CornerStyle.ROUND -> Res.string.settings_corners_round
                    }
                )
            },
            onSelect = { value -> update { it.copy(corners = value) } },
        )
        Hint(stringResource(Res.string.settings_corners_hint))
        SwitchRow(
            title = stringResource(Res.string.settings_seasonal),
            subtitle = stringResource(Res.string.settings_seasonal_hint),
            checked = settings.seasonal,
            onChange = { value -> update { it.copy(seasonal = value) } },
        )
    }
}

/**
 * Alkaline: its newest release, installed or updated from here (Play does
 * the same by itself), where releases come from, and another folder to use
 * instead (a client of its own or a source checkout).
 */
@Composable
private fun AlkalineSection(settings: Settings, update: Update, repository: SulfurRepository) {
    val installed by repository.installed.collectAsState()
    val installs by repository.installs.collectAsState()
    val found = installed.any { it.alkaline }
    val busy = installs.any { it.versionId == "Alkaline" && it.isRunning }
    val scope = rememberCoroutineScope()
    var checks by remember { mutableIntStateOf(0) }
    val initialFeed = remember { settings.alkalineFeed }
    // Asked again when the feed changes (once typing pauses), on Refresh and after an install.
    val release by produceState<Result<AlkalineRelease?>?>(null, settings.alkalineFeed, checks, busy) {
        if (settings.alkalineFeed != initialFeed) delay(600)
        value = null
        value = runCatching { repository.alkalineRelease(refresh = checks > 0 || settings.alkalineFeed != initialFeed) }
    }
    val chooseTitle = stringResource(Res.string.settings_alkaline_choose)
    SectionCard(
        stringResource(Res.string.settings_alkaline),
        icon = painterResource(Res.drawable.ic_bolt),
        trailing = {
            RefreshButton(release == null, stringResource(Res.string.settings_alkaline_check), onClick = { checks++ })
        },
    ) {
        val latest = release?.getOrNull()
        val current = remember(installed) { repository.alkalineInstalled() }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(Modifier.weight(1f)) {
                Text(
                    when {
                        current != null -> stringResource(Res.string.settings_alkaline_installed, current)
                        found -> stringResource(Res.string.settings_alkaline_found)
                        else -> stringResource(Res.string.settings_alkaline_not_installed)
                    },
                    style = MaterialTheme.typography.titleSmall,
                )
                Hint(
                    release.let { r ->
                        when {
                            r == null -> stringResource(Res.string.settings_alkaline_checking)
                            r.isFailure -> stringResource(Res.string.settings_alkaline_check_failed, r.exceptionOrNull()?.message.orEmpty())
                            latest == null -> stringResource(Res.string.settings_alkaline_no_release)
                            !latest.available -> stringResource(Res.string.settings_alkaline_no_download, latest.version)
                            else -> stringResource(Res.string.settings_alkaline_latest, latest.version)
                        }
                    }
                )
            }
            if (latest != null && latest.pageUrl.isNotBlank()) {
                TextButton(onClick = { scope.launch { openUrl(latest.pageUrl) } }) {
                    Text(stringResource(Res.string.settings_alkaline_notes))
                }
            }
            if (latest != null && latest.available && latest.version != current) {
                FilledTonalButton(onClick = repository::installAlkaline, enabled = !busy) {
                    Text(stringResource(if (current == null) Res.string.action_install else Res.string.settings_alkaline_update, latest.version))
                }
            }
        }
        OutlinedTextField(
            value = settings.alkalineFeed.orEmpty(),
            onValueChange = { feed -> update { it.copy(alkalineFeed = feed.ifBlank { null }) } },
            label = { Text(stringResource(Res.string.settings_alkaline_feed)) },
            placeholder = { Text(stringResource(Res.string.settings_alkaline_feed_default)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = settings.alkalineDir.orEmpty(),
                onValueChange = { dir -> update { it.copy(alkalineDir = dir.ifBlank { null }) } },
                label = { Text(stringResource(Res.string.settings_alkaline_dir)) },
                supportingText = if (settings.alkalineDir != null && !found) {
                    { Text(stringResource(Res.string.settings_alkaline_missing)) }
                } else null,
                isError = !found && settings.alkalineDir != null,
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(onClick = {
                chooseFolder(chooseTitle)?.let { dir -> update { it.copy(alkalineDir = dir) } }
            }) { Text(stringResource(Res.string.action_choose_folder)) }
        }
    }
}

@Composable
private fun BackgroundSection(settings: Settings, update: Update, repository: SulfurRepository) {
    val chooseTitle = stringResource(Res.string.settings_background_choose)
    val scope = rememberCoroutineScope()
    SectionCard(stringResource(Res.string.settings_background), icon = painterResource(Res.drawable.ic_layers)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            val preview by rememberBackgroundImage(settings.backgroundImage, 0f)
            Box(
                Modifier.size(width = 112.dp, height = 63.dp).clip(Corners.item)
                    .background(MaterialTheme.colorScheme.surfaceContainerHighest),
                contentAlignment = Alignment.Center,
            ) {
                preview?.let { Image(it, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
            }
            Column(Modifier.weight(1f)) {
                Text(
                    settings.backgroundImage?.let { File(it).name } ?: stringResource(Res.string.settings_background_none),
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilledTonalButton(
                        onClick = {
                            chooseFile(chooseTitle, listOf("png", "jpg", "jpeg", "webp", "bmp", "gif"))
                                ?.let { path -> update { it.copy(backgroundImage = path) } }
                        },
                        shapes = ButtonDefaults.shapes(),
                    ) { Text(chooseTitle) }
                    TextButton(onClick = {
                        scope.launch {
                            repository.desktopWallpaper()?.path?.let { path -> update { it.copy(backgroundImage = path) } }
                        }
                    }) { Text(stringResource(Res.string.settings_background_use_wallpaper)) }
                    if (settings.backgroundImage != null) {
                        TextButton(onClick = { update { it.copy(backgroundImage = null) } }) {
                            Text(stringResource(Res.string.settings_background_remove))
                        }
                    }
                }
            }
        }
        val enabled = settings.backgroundImage != null
        Label(stringResource(Res.string.settings_background_opacity, percent(settings.backgroundOpacity)))
        Slider(
            value = settings.backgroundOpacity,
            onValueChange = { v -> update { it.copy(backgroundOpacity = (v * 20).roundToInt() / 20f) } },
            valueRange = 0f..1f,
            enabled = enabled,
        )
        Label(stringResource(Res.string.settings_background_blur, settings.backgroundBlur.roundToInt().toString()))
        Slider(
            value = settings.backgroundBlur,
            onValueChange = { v -> update { it.copy(backgroundBlur = v.roundToInt().toFloat()) } },
            valueRange = 0f..48f,
            enabled = enabled,
        )
    }
}

private fun percent(value: Float): String = NumberFormat.getPercentInstance(Locale.getDefault()).format(value)

@Composable
private fun GameFolderSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_game_folder), icon = painterResource(Res.drawable.ic_folder_open)) {
        OutlinedTextField(
            value = settings.gameDir.orEmpty(),
            onValueChange = { dir -> update { it.copy(gameDir = dir.ifBlank { null }) } },
            label = { Text(stringResource(Res.string.settings_game_dir)) },
            placeholder = { Text(stringResource(Res.string.settings_game_dir_default)) },
            supportingText = { Text(stringResource(Res.string.settings_game_dir_hint)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Label(stringResource(Res.string.settings_isolation))
        ConnectedChoice(
            options = listOf(Isolation.ISOLATED, Isolation.SHARED),
            selected = settings.isolation,
            label = {
                stringResource(if (it == Isolation.ISOLATED) Res.string.settings_isolation_isolated else Res.string.settings_isolation_shared)
            },
            onSelect = { value -> update { it.copy(isolation = value) } },
        )
        Hint(stringResource(Res.string.settings_isolation_hint))
    }
}

@Composable
private fun WindowSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_window), icon = painterResource(Res.drawable.ic_memory)) {
        Label(stringResource(Res.string.settings_memory, String.format(Locale.getDefault(), "%.1f", settings.maxMemoryMib.toInt() / 1024.0)))
        Slider(
            value = settings.maxMemoryMib.toFloat(),
            onValueChange = { mib -> update { it.copy(maxMemoryMib = (mib / 512).toInt().times(512).toUInt()) } },
            valueRange = 1024f..16384f,
            steps = 29,
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp),
            itemVerticalAlignment = Alignment.CenterVertically) {
            NumberField(stringResource(Res.string.settings_width), settings.windowWidth, Modifier.width(120.dp)) { v -> update { it.copy(windowWidth = v) } }
            NumberField(stringResource(Res.string.settings_height), settings.windowHeight, Modifier.width(120.dp)) { v -> update { it.copy(windowHeight = v) } }
            OptionalNumberField(
                stringResource(Res.string.settings_min_memory),
                settings.minMemoryMib.takeIf { it > 0u },
                stringResource(Res.string.settings_min_memory_hint),
                Modifier.width(220.dp),
            ) { v -> update { it.copy(minMemoryMib = v ?: 0u) } }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(settings.fullscreen, onCheckedChange = { v -> update { it.copy(fullscreen = v) } })
                Spacer(Modifier.width(8.dp))
                Text(stringResource(Res.string.settings_fullscreen))
            }
        }
    }
}

@Composable
private fun JavaRuntimeSection(settings: Settings, update: Update, misc: MiscViewModel, repository: SulfurRepository) {
    val installs by repository.installs.collectAsState()
    val runtimes by misc.java.collectAsState()
    SectionCard(stringResource(Res.string.settings_java_runtime), icon = painterResource(Res.drawable.ic_coffee)) {
        JavaPicker(settings, update, misc)
        SwitchRow(
            title = stringResource(Res.string.settings_java_auto_download),
            subtitle = stringResource(Res.string.settings_java_auto_download_hint),
            checked = settings.autoJava,
            onChange = { v -> update { it.copy(autoJava = v) } },
        )
        Label(stringResource(Res.string.settings_java_download))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            repository.downloadableJava.forEach { major ->
                val task = installs.firstOrNull { it.versionId == "Java $major" }
                val have = runtimes.orEmpty().any { it.managed && it.major == major }
                val busy = task?.isRunning == true
                FilledTonalButton(
                    onClick = { repository.installJava(major) },
                    enabled = !busy,
                    shapes = ButtonDefaults.shapes(),
                ) {
                    when {
                        busy -> LoadingIndicator(Modifier.size(ButtonDefaults.IconSize))
                        have -> Icon(painterResource(Res.drawable.ic_check), null, Modifier.size(ButtonDefaults.IconSize))
                        else -> Icon(painterResource(Res.drawable.ic_download), null, Modifier.size(ButtonDefaults.IconSize))
                    }
                    Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                    Text("Java $major")
                }
            }
        }
        installs.firstOrNull { it.versionId.startsWith("Java ") && it.isRunning }?.let { task ->
            LinearWavyProgressIndicator(progress = { task.fraction.toFloat() }, modifier = Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun JvmSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_jvm), icon = painterResource(Res.drawable.ic_tune)) {
        OutlinedTextField(
            value = settings.jvmArgs,
            onValueChange = { args -> update { it.copy(jvmArgs = args) } },
            label = { Text(stringResource(Res.string.settings_jvm_args)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OptionField(settings.gameArgs, { args -> update { it.copy(gameArgs = args) } }, stringResource(Res.string.settings_game_args))
        SwitchRow(
            title = stringResource(Res.string.settings_log4j),
            subtitle = stringResource(Res.string.settings_log4j_hint),
            checked = settings.log4jFix,
            onChange = { v -> update { it.copy(log4jFix = v) } },
        )
    }
}

@Composable
private fun JavaPicker(settings: Settings, update: Update, misc: MiscViewModel) {
    val runtimes by misc.java.collectAsState()
    JavaChoice(runtimes.orEmpty(), settings.javaPath, stringResource(Res.string.settings_java_auto)) { path ->
        update { it.copy(javaPath = path) }
    }
}

/** Garbage collector and how the game process starts. */
@Composable
private fun AdvancedSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_advanced), icon = painterResource(Res.drawable.ic_science)) {
        GcChoice(settings.gc, inherited = null) { gc -> update { it.copy(gc = gc ?: GarbageCollector.AUTO) } }
        ProcessFields(
            wrapper = settings.wrapperCommand,
            preLaunch = settings.preLaunchCommand,
            env = settings.envVars,
            onWrapper = { v -> update { it.copy(wrapperCommand = v) } },
            onPreLaunch = { v -> update { it.copy(preLaunchCommand = v) } },
            onEnv = { v -> update { it.copy(envVars = v) } },
        )
    }
}

@Composable
private fun DownloadSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_download), icon = painterResource(Res.drawable.ic_download)) {
        Label(stringResource(Res.string.settings_source))
        ConnectedChoice(
            options = listOf(DownloadSource.AUTO, DownloadSource.OFFICIAL, DownloadSource.MIRROR),
            selected = settings.downloadSource,
            label = { stringResource(sourceLabel(it)) },
            onSelect = { value -> update { it.copy(downloadSource = value) } },
        )
        Hint(
            stringResource(
                when (settings.downloadSource) {
                    DownloadSource.AUTO -> Res.string.settings_source_auto_hint
                    DownloadSource.OFFICIAL -> Res.string.settings_source_official_hint
                    DownloadSource.MIRROR -> Res.string.settings_source_mirror_hint
                }
            )
        )
        Label(stringResource(Res.string.settings_connections, settings.maxConnections.toString()))
        Slider(
            value = settings.maxConnections.toFloat(),
            onValueChange = { v -> update { it.copy(maxConnections = (v / 16).toInt().times(16).coerceAtLeast(16).toUInt()) } },
            valueRange = 16f..512f,
            steps = 30,
        )
        Label(stringResource(Res.string.settings_segments, settings.segmentsPerFile.toString()))
        Slider(
            value = settings.segmentsPerFile.toFloat(),
            onValueChange = { v -> update { it.copy(segmentsPerFile = v.toInt().toUInt()) } },
            valueRange = 1f..16f,
            steps = 14,
        )
    }
}

@Composable
private fun ModsSection(settings: Settings, update: Update) {
    SectionCard("CurseForge", icon = painterResource(Res.drawable.ic_extension)) {
        OutlinedTextField(
            value = settings.curseforgeApiKey,
            onValueChange = { key -> update { it.copy(curseforgeApiKey = key.trim()) } },
            label = { Text(stringResource(Res.string.settings_curseforge_key)) },
            supportingText = { Text(stringResource(Res.string.settings_curseforge_key_hint)) },
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun CleanroomSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_cleanroom), icon = painterResource(Res.drawable.ic_deployed_code)) {
        SwitchRow(
            title = stringResource(Res.string.settings_cleanroom_direct),
            subtitle = stringResource(Res.string.settings_cleanroom_direct_hint),
            checked = settings.cleanroomDirect,
            onChange = { v -> update { it.copy(cleanroomDirect = v) } },
        )
    }
}

@Composable
private fun ReuseSection(settings: Settings, update: Update) {
    SectionCard(stringResource(Res.string.settings_reuse), icon = painterResource(Res.drawable.ic_link)) {
        Hint(stringResource(Res.string.settings_reuse_hint))
        SwitchRow(
            title = stringResource(Res.string.settings_reuse_official),
            subtitle = stringResource(Res.string.settings_reuse_official_hint),
            checked = settings.reuseOfficialDir,
            onChange = { v -> update { it.copy(reuseOfficialDir = v) } },
        )
        settings.reuseDirs.forEachIndexed { index, dir ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(dir, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                IconButton(onClick = { update { it.copy(reuseDirs = it.reuseDirs.filterIndexed { i, _ -> i != index }) } }) {
                    Icon(painterResource(Res.drawable.ic_delete), stringResource(Res.string.action_remove_folder))
                }
            }
        }
        var draft by remember { mutableStateOf("") }
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = draft,
                onValueChange = { draft = it },
                label = { Text(stringResource(Res.string.settings_reuse_another)) },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            TextButton(
                onClick = {
                    val dir = draft.trim()
                    if (dir.isNotEmpty()) update { it.copy(reuseDirs = it.reuseDirs + dir) }
                    draft = ""
                },
                enabled = draft.isNotBlank(),
            ) { Text(stringResource(Res.string.action_add)) }
        }
    }
}

/** Settings from HMCL, PCL CE or PCL2, found in folders the user picks and in their usual places. */
@Composable
private fun ImportSection(repository: SulfurRepository) {
    val scope = rememberCoroutineScope()
    var folders by rememberSaveable { mutableStateOf(listOf<String>()) }
    var found by remember { mutableStateOf<List<ForeignConfig>?>(null) }
    var useGameDir by rememberSaveable { mutableStateOf(true) }
    LaunchedEffect(folders) { found = repository.findLauncherConfigs(folders) }
    val chooseTitle = stringResource(Res.string.settings_import_choose)
    SectionCard(stringResource(Res.string.settings_import), icon = painterResource(Res.drawable.ic_history)) {
        Hint(stringResource(Res.string.settings_import_hint))
        FilledTonalButton(
            onClick = { chooseFolder(chooseTitle)?.let { dir -> if (dir !in folders) folders = folders + dir } },
            shapes = ButtonDefaults.shapes(),
        ) {
            Icon(painterResource(Res.drawable.ic_folder_open), null, Modifier.size(ButtonDefaults.IconSize))
            Spacer(Modifier.width(ButtonDefaults.IconSpacing))
            Text(chooseTitle)
        }
        val configs = found
        when {
            configs == null -> LoadingIndicator(Modifier.size(32.dp))
            configs.isEmpty() -> Hint(stringResource(Res.string.settings_import_none))
            else -> {
                SwitchRow(
                    title = stringResource(Res.string.settings_import_use_folder),
                    subtitle = stringResource(Res.string.settings_import_use_folder_hint),
                    checked = useGameDir,
                    onChange = { useGameDir = it },
                )
                configs.forEach { config ->
                    ForeignConfigCard(config) {
                        scope.launch {
                            runCatching { repository.importLauncherConfig(config, useGameDir) }
                                .onSuccess { repository.notify(UiMessage(Res.string.msg_imported, launcherName(config.launcher))) }
                                .onFailure(repository::notifyError)
                        }
                    }
                }
            }
        }
    }
}

private fun launcherName(launcher: ForeignLauncher): String = when (launcher) {
    ForeignLauncher.HMCL -> "HMCL"
    ForeignLauncher.PCL_CE -> "PCL CE"
    ForeignLauncher.PCL2 -> "PCL2"
}

/** What one launcher's settings would bring over. */
@Composable
private fun ForeignConfigCard(config: ForeignConfig, onImport: () -> Unit) {
    val lines = buildList {
        if (config.gameDirs.isNotEmpty()) add(stringResource(Res.string.settings_import_game_dirs, config.gameDirs.joinToString(", ")))
        config.maxMemoryMib?.let {
            add(stringResource(Res.string.settings_memory, String.format(Locale.getDefault(), "%.1f", it.toInt() / 1024.0)))
        }
        if (config.windowWidth != null && config.windowHeight != null) {
            add(stringResource(Res.string.settings_import_window, "${config.windowWidth} × ${config.windowHeight}"))
        }
        if (config.fullscreen == true) add(stringResource(Res.string.settings_fullscreen))
        config.javaPath?.let { add("${stringResource(Res.string.settings_java)}: $it") }
        config.jvmArgs?.let { add("${stringResource(Res.string.settings_jvm_args)}: $it") }
        config.downloadSource?.let {
            add("${stringResource(Res.string.settings_source)}: ${stringResource(sourceLabel(it))}")
        }
        config.maxConnections?.let { add(stringResource(Res.string.settings_connections, it.toString())) }
        config.isolation?.let {
            add(
                "${stringResource(Res.string.settings_isolation)}: " + stringResource(
                    if (it == Isolation.ISOLATED) Res.string.settings_isolation_isolated else Res.string.settings_isolation_shared
                )
            )
        }
        config.offlineName?.let { add("${stringResource(Res.string.launch_offline_name)}: $it") }
        config.backgroundImage?.let { add("${stringResource(Res.string.settings_background)}: ${File(it).name}") }
        config.language?.let { code -> Languages.firstOrNull { it.first == code }?.let { add("${stringResource(Res.string.settings_language)}: ${it.second}") } }
    }
    Surface(shape = Corners.card, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(launcherName(config.launcher), style = MaterialTheme.typography.titleSmall)
                    Text(config.location, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Spacer(Modifier.width(8.dp))
                FilledTonalButton(onClick = onImport, shapes = ButtonDefaults.shapes()) { Text(stringResource(Res.string.action_import)) }
            }
            lines.forEach { Text(it, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
    }
}

private fun sourceLabel(source: DownloadSource): StringResource = when (source) {
    DownloadSource.AUTO -> Res.string.settings_source_auto
    DownloadSource.OFFICIAL -> Res.string.settings_source_official
    DownloadSource.MIRROR -> Res.string.settings_source_mirror
}
