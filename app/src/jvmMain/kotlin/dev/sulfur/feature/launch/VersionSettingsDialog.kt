package dev.sulfur.feature.launch

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Icon
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import dev.sulfur.data.SulfurRepository
import dev.sulfur.feature.settings.GcChoice
import dev.sulfur.feature.settings.Hint
import dev.sulfur.feature.settings.JavaChoice
import dev.sulfur.feature.settings.Label
import dev.sulfur.feature.settings.OptionField
import dev.sulfur.feature.settings.OptionalNumberField
import dev.sulfur.feature.settings.ProcessFields
import dev.sulfur.feature.settings.SwitchRow
import dev.sulfur.ffi.InstalledVersion
import dev.sulfur.ffi.Isolation
import dev.sulfur.ffi.JavaRuntime
import dev.sulfur.ffi.VersionSettings
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_close
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.ic_extension
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.ic_layers
import dev.sulfur.resources.isolation_isolated
import dev.sulfur.resources.isolation_isolated_hint
import dev.sulfur.resources.isolation_shared
import dev.sulfur.resources.isolation_shared_hint
import dev.sulfur.resources.launch_game_folder
import dev.sulfur.resources.launch_java
import dev.sulfur.resources.launch_saves_mods
import dev.sulfur.resources.settings_fullscreen
import dev.sulfur.resources.settings_game_args
import dev.sulfur.resources.settings_height
import dev.sulfur.resources.settings_jvm_args
import dev.sulfur.resources.settings_log4j
import dev.sulfur.resources.settings_log4j_hint
import dev.sulfur.resources.settings_memory
import dev.sulfur.resources.settings_min_memory
import dev.sulfur.resources.settings_min_memory_hint
import dev.sulfur.resources.settings_width
import dev.sulfur.resources.version_follow_global
import dev.sulfur.resources.version_global
import dev.sulfur.resources.version_memory_custom
import dev.sulfur.resources.version_memory_custom_hint
import dev.sulfur.resources.version_off
import dev.sulfur.resources.version_on
import dev.sulfur.resources.version_reset
import dev.sulfur.resources.version_server
import dev.sulfur.resources.version_server_hint
import dev.sulfur.resources.version_tab_advanced
import dev.sulfur.resources.version_tab_general
import dev.sulfur.resources.version_tab_java
import dev.sulfur.resources.version_window
import dev.sulfur.resources.version_windowed
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.loaderName
import dev.sulfur.util.openFolder
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.util.Locale

/** No overrides: the version follows the global settings everywhere. */
private val NoOverrides = VersionSettings(
    javaPath = null, maxMemoryMib = null, minMemoryMib = null, windowWidth = null, windowHeight = null,
    fullscreen = null, jvmArgs = null, gameArgs = null, gc = null, wrapperCommand = null,
    preLaunchCommand = null, envVars = null, log4jFix = null, serverAddress = null,
)

/** Blank text means "follow the global value". */
private fun String.orInherit(): String? = takeIf { it.isNotBlank() }

private enum class VersionTab(val title: StringResource) {
    General(Res.string.version_tab_general),
    Java(Res.string.version_tab_java),
    Advanced(Res.string.version_tab_advanced),
}

/**
 * Settings of one version. Every field starts empty or on "Default" and then
 * follows Settings; the global value shows as the placeholder. Changes are
 * saved shortly after typing stops and when the dialog closes.
 */
@Composable
internal fun VersionSettingsDialog(
    version: InstalledVersion,
    repository: SulfurRepository,
    runtimes: List<JavaRuntime>,
    onDismiss: () -> Unit,
) {
    val global by repository.settings.collectAsState()
    val loaded = remember(version.id) { runCatching { repository.versionSettings(version.id) }.getOrDefault(NoOverrides) }
    var draft by remember(version.id) { mutableStateOf(loaded) }
    var saved by remember(version.id) { mutableStateOf(loaded) }
    fun save() {
        if (draft != saved) {
            repository.setVersionSettings(version.id, draft)
            saved = draft
        }
    }
    LaunchedEffect(draft) {
        delay(500)
        save()
    }
    fun edit(transform: (VersionSettings) -> VersionSettings) {
        draft = transform(draft)
    }
    val close = {
        save()
        onDismiss()
    }
    var tab by remember { mutableIntStateOf(0) }

    AlertDialog(
        onDismissRequest = close,
        icon = { Icon(painterResource(Res.drawable.ic_layers), null) },
        title = { Text(version.id) },
        text = {
            Column(Modifier.width(600.dp)) {
                PrimaryTabRow(selectedTabIndex = tab, containerColor = Color.Transparent) {
                    VersionTab.entries.forEachIndexed { index, item ->
                        Tab(selected = tab == index, onClick = { tab = index }, text = { Text(stringResource(item.title)) })
                    }
                }
                Column(
                    Modifier.fillMaxWidth().height(440.dp).verticalScroll(rememberScrollState()).padding(top = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    when (VersionTab.entries[tab]) {
                        VersionTab.General -> GeneralTab(version, repository, draft, global, ::edit)
                        VersionTab.Java -> JavaTab(draft, global, runtimes, ::edit)
                        VersionTab.Advanced -> AdvancedTab(draft, global, ::edit)
                    }
                }
            }
        },
        dismissButton = {
            TextButton(onClick = { draft = NoOverrides }, enabled = draft != NoOverrides) {
                Text(stringResource(Res.string.version_reset))
            }
        },
        confirmButton = { TextButton(onClick = close) { Text(stringResource(Res.string.action_close)) } },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    )
}

private typealias Edit = ((VersionSettings) -> VersionSettings) -> Unit

@Composable
private fun GeneralTab(
    version: InstalledVersion,
    repository: SulfurRepository,
    draft: VersionSettings,
    global: dev.sulfur.ffi.Settings,
    edit: Edit,
) {
    VersionFacts(version)
    Label(stringResource(Res.string.launch_saves_mods))
    // Isolation is stored apart from the other overrides and applies at once.
    ConnectedChoice(
        options = listOf(null, Isolation.ISOLATED, Isolation.SHARED),
        selected = if (version.isolationOverridden) version.isolation else null,
        label = {
            stringResource(
                when (it) {
                    null -> Res.string.version_global
                    Isolation.ISOLATED -> Res.string.isolation_isolated
                    Isolation.SHARED -> Res.string.isolation_shared
                }
            )
        },
        onSelect = { repository.setVersionIsolation(version.id, it) },
    )
    Hint(stringResource(if (version.isolation == Isolation.ISOLATED) Res.string.isolation_isolated_hint else Res.string.isolation_shared_hint))

    Label(stringResource(Res.string.version_window))
    DefaultOrChoice(
        value = draft.fullscreen,
        on = stringResource(Res.string.settings_fullscreen),
        off = stringResource(Res.string.version_windowed),
    ) { v -> edit { it.copy(fullscreen = v) } }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OptionalNumberField(stringResource(Res.string.settings_width), draft.windowWidth, global.windowWidth.toString(), Modifier.width(140.dp)) { v ->
            edit { it.copy(windowWidth = v) }
        }
        OptionalNumberField(stringResource(Res.string.settings_height), draft.windowHeight, global.windowHeight.toString(), Modifier.width(140.dp)) { v ->
            edit { it.copy(windowHeight = v) }
        }
    }
    OptionField(
        draft.serverAddress.orEmpty(),
        { v -> edit { it.copy(serverAddress = v.orInherit()) } },
        stringResource(Res.string.version_server),
        stringResource(Res.string.version_server_hint),
    )
}

@Composable
private fun JavaTab(draft: VersionSettings, global: dev.sulfur.ffi.Settings, runtimes: List<JavaRuntime>, edit: Edit) {
    val followGlobal = stringResource(Res.string.version_follow_global)
    JavaChoice(runtimes, draft.javaPath, followGlobal) { path -> edit { it.copy(javaPath = path) } }
    SwitchRow(
        title = stringResource(Res.string.version_memory_custom),
        subtitle = stringResource(Res.string.version_memory_custom_hint),
        checked = draft.maxMemoryMib != null,
        onChange = { on -> edit { it.copy(maxMemoryMib = if (on) global.maxMemoryMib else null) } },
    )
    draft.maxMemoryMib?.let { mib ->
        Label(stringResource(Res.string.settings_memory, String.format(Locale.getDefault(), "%.1f", mib.toInt() / 1024.0)))
        Slider(
            value = mib.toFloat(),
            onValueChange = { v -> edit { it.copy(maxMemoryMib = (v / 512).toInt().times(512).toUInt()) } },
            valueRange = 1024f..16384f,
            steps = 29,
        )
    }
    OptionalNumberField(
        stringResource(Res.string.settings_min_memory),
        draft.minMemoryMib,
        global.minMemoryMib.takeIf { it > 0u }?.toString() ?: stringResource(Res.string.settings_min_memory_hint),
        Modifier.fillMaxWidth(),
    ) { v -> edit { it.copy(minMemoryMib = v) } }
    OptionField(
        draft.jvmArgs.orEmpty(),
        { v -> edit { it.copy(jvmArgs = v.orInherit()) } },
        stringResource(Res.string.settings_jvm_args),
        placeholder = global.jvmArgs.ifBlank { followGlobal },
        monospace = true,
    )
    OptionField(
        draft.gameArgs.orEmpty(),
        { v -> edit { it.copy(gameArgs = v.orInherit()) } },
        stringResource(Res.string.settings_game_args),
        placeholder = global.gameArgs.ifBlank { followGlobal },
        monospace = true,
    )
}

@Composable
private fun AdvancedTab(draft: VersionSettings, global: dev.sulfur.ffi.Settings, edit: Edit) {
    GcChoice(draft.gc, inherited = global.gc) { gc -> edit { it.copy(gc = gc) } }
    ProcessFields(
        wrapper = draft.wrapperCommand.orEmpty(),
        preLaunch = draft.preLaunchCommand.orEmpty(),
        env = draft.envVars.orEmpty(),
        onWrapper = { v -> edit { it.copy(wrapperCommand = v.orInherit()) } },
        onPreLaunch = { v -> edit { it.copy(preLaunchCommand = v.orInherit()) } },
        onEnv = { v -> edit { it.copy(envVars = v.orInherit()) } },
        inherited = Triple(global.wrapperCommand, global.preLaunchCommand, global.envVars),
    )
    Label(stringResource(Res.string.settings_log4j))
    DefaultOrChoice(
        value = draft.log4jFix,
        on = stringResource(Res.string.version_on),
        off = stringResource(Res.string.version_off),
    ) { v -> edit { it.copy(log4jFix = v) } }
    Hint(stringResource(Res.string.settings_log4j_hint))
}

/** Default (null, the global value), or an explicit on or off. */
@Composable
private fun DefaultOrChoice(value: Boolean?, on: String, off: String, onSelect: (Boolean?) -> Unit) {
    val default = stringResource(Res.string.version_global)
    ConnectedChoice(
        options = listOf(null, true, false),
        selected = value,
        label = { v -> when (v) { null -> default; true -> on; false -> off } },
        onSelect = onSelect,
    )
}

/** Loader, Java and folder of the version. */
@Composable
private fun VersionFacts(version: InstalledVersion) {
    val scope = rememberCoroutineScope()
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), itemVerticalAlignment = Alignment.CenterVertically) {
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
