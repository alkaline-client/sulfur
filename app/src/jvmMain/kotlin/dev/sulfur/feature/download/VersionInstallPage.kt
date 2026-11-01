package dev.sulfur.feature.download

import androidx.compose.animation.AnimatedContent
import dev.sulfur.ui.theme.Corners
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.ffi.InstallRequest
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.LoaderVersion
import dev.sulfur.ffi.RemoteVersion
import dev.sulfur.ffi.VersionKind
import dev.sulfur.ffi.cleanroomInstallerVersion
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_back
import dev.sulfur.resources.action_choose_file
import dev.sulfur.resources.action_close
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.ic_arrow_back
import dev.sulfur.resources.ic_check_circle
import dev.sulfur.resources.ic_close
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.install_cleanroom_hint
import dev.sulfur.resources.install_cleanroom_open_page
import dev.sulfur.resources.install_cleanroom_page_hint
import dev.sulfur.resources.install_cleanroom_picked
import dev.sulfur.resources.install_fabric_api
import dev.sulfur.resources.install_fabric_api_hint
import dev.sulfur.resources.install_installed_hint
import dev.sulfur.resources.install_loader
import dev.sulfur.resources.install_loader_pick_hint
import dev.sulfur.resources.install_loader_version
import dev.sulfur.resources.install_name
import dev.sulfur.resources.install_name_hint
import dev.sulfur.resources.install_name_taken
import dev.sulfur.resources.install_optifine_hint
import dev.sulfur.resources.install_rift_hint
import dev.sulfur.resources.install_vanilla_hint
import dev.sulfur.resources.installed
import dev.sulfur.resources.loader_latest
import dev.sulfur.resources.loader_load_failed
import dev.sulfur.resources.loader_recommended
import dev.sulfur.resources.loader_unavailable
import dev.sulfur.resources.loader_unstable
import dev.sulfur.ui.components.Logo
import dev.sulfur.ui.components.formatDate
import dev.sulfur.ui.components.kindLabel
import dev.sulfur.ui.components.loaderLogo
import dev.sulfur.ui.components.loaderName
import dev.sulfur.ui.components.minecraftLogo
import dev.sulfur.util.chooseFile
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

/** Loaders in the order the page lists them, the most used first. */
private val PageLoaders = listOf(
    LoaderKind.FABRIC,
    LoaderKind.QUILT,
    LoaderKind.FORGE,
    LoaderKind.NEO_FORGE,
    LoaderKind.OPTI_FINE,
    LoaderKind.LEGACY_FABRIC,
    LoaderKind.ORNITHE,
    LoaderKind.CLEANROOM,
    LoaderKind.RIFT,
)

private sealed interface Versions {
    data object Loading : Versions
    data class Loaded(val list: List<LoaderVersion>) : Versions
    data class Failed(val message: String) : Versions
}

private val Versions.available get() = this is Versions.Loaded && list.isNotEmpty()

/**
 * One game version's install options: the game as it is, or one mod loader
 * on top of it. Every loader's versions are asked for at once, so loaders
 * that do not support this version show as unavailable.
 */
@Composable
fun VersionInstallPage(
    gameVersion: String,
    /** The version as listed (channel and date); null if it is not in the list. */
    listed: RemoteVersion?,
    installedIds: Set<String>,
    /** Cleanroom straight from its repository instead of through its download page. */
    cleanroomDirect: Boolean,
    /** Back goes to the update's card; without one, the page closes. */
    closes: Boolean,
    /** Chosen once its versions have loaded, if it supports this game version. */
    initialLoader: LoaderKind? = null,
    loadVersions: suspend (LoaderKind, String) -> List<LoaderVersion>,
    onBack: () -> Unit,
    onInstall: (InstallRequest) -> Unit,
    modifier: Modifier = Modifier,
) {
    val installed = gameVersion in installedIds
    val versions = remember(gameVersion) { mutableStateMapOf<LoaderKind, Versions>() }
    val scope = rememberCoroutineScope()
    fun load(kind: LoaderKind) {
        versions[kind] = Versions.Loading
        scope.launch {
            versions[kind] = runCatching { loadVersions(kind, gameVersion) }
                .fold({ Versions.Loaded(it) }, { Versions.Failed(it.message ?: it.toString()) })
        }
    }
    LaunchedEffect(gameVersion) { PageLoaders.forEach(::load) }

    var loader by remember(gameVersion) { mutableStateOf<LoaderKind?>(null) }
    var chosen by remember(gameVersion) { mutableStateOf<String?>(null) }
    var fabricApi by remember(gameVersion) { mutableStateOf(true) }
    var name by remember(gameVersion) { mutableStateOf("") }
    var nameEdited by remember(gameVersion) { mutableStateOf(false) }
    // Cleanroom through its download page: the installer the user picked there.
    val viaPage = loader == LoaderKind.CLEANROOM && !cleanroomDirect
    var installer by remember(gameVersion) { mutableStateOf<String?>(null) }
    var pageOpen by remember { mutableStateOf(false) }
    val picked = { path: String ->
        cleanroomInstallerVersion(path)?.let { v ->
            installer = path
            chosen = v
            pageOpen = false
        }
    }

    fun select(kind: LoaderKind?) {
        loader = kind
        val list = (versions[kind] as? Versions.Loaded)?.list.orEmpty()
        chosen = if (kind == LoaderKind.CLEANROOM && !cleanroomDirect) installer?.let(::cleanroomInstallerVersion)
        else (list.firstOrNull { it.stable } ?: list.firstOrNull())?.version
    }
    val initialVersions = initialLoader?.let { versions[it] }
    var initialApplied by remember(gameVersion) { mutableStateOf(false) }
    LaunchedEffect(initialVersions) {
        if (!initialApplied && initialLoader != null && initialVersions?.available == true) {
            initialApplied = true
            if (loader == null) select(initialLoader)
        }
    }
    LaunchedEffect(loader, chosen) {
        if (!nameEdited) name = suggestedName(gameVersion, loader, chosen)
    }

    val nameTaken = loader != null && name.trim() in installedIds
    val canInstall = if (loader == null) !installed else chosen != null && name.isNotBlank() && !nameTaken

    Column(modifier) {
        Row(Modifier.fillMaxWidth().padding(start = 8.dp, end = 16.dp, top = 12.dp, bottom = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                if (closes) Icon(painterResource(Res.drawable.ic_close), stringResource(Res.string.action_close))
                else Icon(painterResource(Res.drawable.ic_arrow_back), stringResource(Res.string.action_back))
            }
            Spacer(Modifier.width(8.dp))
            Logo(minecraftLogo(listed?.kind ?: VersionKind.RELEASE), 48.dp)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(gameVersion, style = MaterialTheme.typography.headlineSmallEmphasized, maxLines = 1, overflow = TextOverflow.Ellipsis)
                listed?.let {
                    Text(
                        "Minecraft · ${kindLabel(it.kind)} · ${formatDate(it.releaseTime)}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        HorizontalDivider()

        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column {
                Text(stringResource(Res.string.install_loader), style = MaterialTheme.typography.titleMediumEmphasized)
                Hint(stringResource(if (installed) Res.string.install_installed_hint else Res.string.install_loader_pick_hint))
            }
            LoaderGrid(
                gameVersion = gameVersion,
                versions = versions,
                selected = loader,
                onSelect = { kind -> select(kind.takeIf { it != loader }) },
                onRetry = ::load,
            )

            when (val kind = loader) {
                null -> if (!installed) Hint(stringResource(Res.string.install_vanilla_hint))
                else -> Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (viaPage) {
                        Hint(stringResource(Res.string.install_cleanroom_page_hint))
                        val chooseTitle = stringResource(Res.string.action_choose_file)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            FilledTonalButton(onClick = { pageOpen = true }, shapes = ButtonDefaults.shapes()) {
                                Text(stringResource(Res.string.install_cleanroom_open_page))
                            }
                            TextButton(onClick = { chooseFile(chooseTitle, listOf("jar"))?.let { picked(it) } }) { Text(chooseTitle) }
                        }
                        chosen?.let { Text(stringResource(Res.string.install_cleanroom_picked, it), style = MaterialTheme.typography.titleSmall) }
                    } else {
                        LoaderVersionPicker((versions[kind] as? Versions.Loaded)?.list.orEmpty(), chosen, onChoose = { chosen = it })
                    }
                    if (kind == LoaderKind.FABRIC) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(stringResource(Res.string.install_fabric_api), style = MaterialTheme.typography.titleSmall)
                                Hint(stringResource(Res.string.install_fabric_api_hint))
                            }
                            Switch(fabricApi, onCheckedChange = { fabricApi = it })
                        }
                    }
                    OutlinedTextField(
                        value = name,
                        onValueChange = {
                            name = it.filterNot { c -> c in "/\\:" }
                            nameEdited = true
                        },
                        label = { Text(stringResource(Res.string.install_name)) },
                        supportingText = {
                            Text(
                                if (nameTaken) stringResource(Res.string.install_name_taken, name.trim())
                                else stringResource(Res.string.install_name_hint)
                            )
                        },
                        isError = nameTaken,
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    when (kind) {
                        LoaderKind.OPTI_FINE -> Hint(stringResource(Res.string.install_optifine_hint))
                        LoaderKind.RIFT -> Hint(stringResource(Res.string.install_rift_hint))
                        LoaderKind.CLEANROOM -> Hint(stringResource(Res.string.install_cleanroom_hint))
                        else -> {}
                    }
                }
            }
        }

        HorizontalDivider()
        Row(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                listOfNotNull("Minecraft $gameVersion", loader?.let { k -> listOfNotNull(loaderName(k), chosen).joinToString(" ") })
                    .joinToString(" + "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(12.dp))
            Button(
                onClick = {
                    onInstall(
                        InstallRequest(
                            gameVersion = gameVersion,
                            loader = loader,
                            loaderVersion = chosen.takeIf { loader != null },
                            name = name.trim().takeIf { loader != null && it.isNotEmpty() },
                            fabricApi = loader == LoaderKind.FABRIC && fabricApi,
                            installerUrl = installer.takeIf { viaPage },
                        )
                    )
                },
                enabled = canInstall,
                shapes = ButtonDefaults.shapes(),
            ) {
                val done = installed && loader == null
                Icon(
                    painterResource(if (done) Res.drawable.ic_check_circle else Res.drawable.ic_download),
                    null,
                    Modifier.size(ButtonDefaults.IconSize),
                )
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(if (done) Res.string.installed else Res.string.action_install))
            }
        }
    }
    if (pageOpen) {
        CleanroomPageWindow(onPicked = { picked(it) }, onClose = { pageOpen = false })
    }
}

/** Loader tiles, as many per row as fit. */
@Composable
private fun LoaderGrid(
    gameVersion: String,
    versions: Map<LoaderKind, Versions>,
    selected: LoaderKind?,
    onSelect: (LoaderKind) -> Unit,
    onRetry: (LoaderKind) -> Unit,
) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val columns = (maxWidth / 220.dp).toInt().coerceIn(1, 3)
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            PageLoaders.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    row.forEach { kind ->
                        LoaderTile(
                            kind = kind,
                            gameVersion = gameVersion,
                            versions = versions[kind] ?: Versions.Loading,
                            selected = kind == selected,
                            onClick = { onSelect(kind) },
                            onRetry = { onRetry(kind) },
                            modifier = Modifier.weight(1f),
                        )
                    }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun LoaderTile(
    kind: LoaderKind,
    gameVersion: String,
    versions: Versions,
    selected: Boolean,
    onClick: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val available = versions.available
    val container by animateColorAsState(if (selected) colors.secondaryContainer else colors.surfaceContainer)
    Surface(
        onClick = onClick,
        enabled = available,
        shape = Corners.card,
        color = container,
        border = if (selected) BorderStroke(2.dp, colors.primary) else null,
        modifier = modifier,
    ) {
        Row(
            Modifier.height(64.dp).padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val unavailable = versions is Versions.Loaded && !available
            Logo(loaderLogo(kind), 40.dp, Modifier.alpha(if (unavailable) 0.4f else 1f))
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f).alpha(if (unavailable) 0.6f else 1f)) {
                Text(loaderName(kind), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                AnimatedContent(
                    targetState = versions,
                    transitionSpec = { fadeIn() togetherWith fadeOut() },
                    contentKey = { it::class },
                    label = "status",
                ) { state ->
                    val status = when (state) {
                        Versions.Loading -> null
                        is Versions.Failed -> stringResource(Res.string.loader_load_failed)
                        is Versions.Loaded -> (state.list.firstOrNull { it.stable } ?: state.list.firstOrNull())
                            ?.let { stringResource(Res.string.loader_latest, shortVersion(it.version, gameVersion)) }
                            ?: stringResource(Res.string.loader_unavailable)
                    }
                    Text(
                        status ?: "",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (state is Versions.Failed) colors.error else colors.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            when {
                versions == Versions.Loading -> LoadingIndicator(Modifier.size(28.dp))
                versions is Versions.Failed -> TextButton(onClick = onRetry) { Text(stringResource(Res.string.action_retry)) }
                selected -> Icon(painterResource(Res.drawable.ic_check_circle), null, tint = colors.primary, modifier = Modifier.size(22.dp))
                else -> Spacer(Modifier.size(22.dp))
            }
        }
    }
}

@Composable
private fun LoaderVersionPicker(versions: List<LoaderVersion>, chosen: String?, onChoose: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val current = versions.firstOrNull { it.version == chosen }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }) {
        OutlinedTextField(
            value = current?.let { versionLabel(it) } ?: "",
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text(stringResource(Res.string.install_loader_version)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            versions.take(200).forEach { v ->
                DropdownMenuItem(
                    text = { Text(versionLabel(v)) },
                    onClick = {
                        onChoose(v.version)
                        open = false
                    },
                )
            }
        }
    }
}

@Composable
private fun versionLabel(v: LoaderVersion): String =
    if (v.stable) stringResource(Res.string.loader_recommended, v.version)
    else stringResource(Res.string.loader_unstable, v.version)

@Composable
private fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** A loader version without the game version Forge repeats around it ("1.8.9-11.15.1.2318-1.8.9"). */
private fun shortVersion(version: String, game: String): String =
    version.removePrefix("$game-").removeSuffix("-$game")

private fun suggestedName(game: String, loader: LoaderKind?, version: String?): String {
    if (loader == null || version == null) return ""
    return "$game-${loaderName(loader)} ${shortVersion(version, game)}"
}
