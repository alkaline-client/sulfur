package dev.sulfur.feature.download

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sulfur.ffi.InstallRequest
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.LoaderVersion
import dev.sulfur.ffi.cleanroomInstallerVersion
import dev.sulfur.resources.action_choose_file
import dev.sulfur.resources.install_cleanroom_hint
import dev.sulfur.resources.install_cleanroom_open_page
import dev.sulfur.resources.install_cleanroom_page_hint
import dev.sulfur.resources.install_cleanroom_picked
import dev.sulfur.resources.install_more_loaders
import dev.sulfur.resources.install_rift_hint
import dev.sulfur.util.chooseFile
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_cancel
import dev.sulfur.resources.action_install
import dev.sulfur.resources.install_dialog_title
import dev.sulfur.resources.install_fabric_api
import dev.sulfur.resources.install_fabric_api_hint
import dev.sulfur.resources.install_loader
import dev.sulfur.resources.install_loader_none
import dev.sulfur.resources.install_loader_version
import dev.sulfur.resources.install_name
import dev.sulfur.resources.install_name_hint
import dev.sulfur.resources.install_no_loader_versions
import dev.sulfur.resources.install_optifine_hint
import dev.sulfur.resources.install_vanilla_hint
import dev.sulfur.resources.loader_recommended
import dev.sulfur.resources.loader_unstable
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.loaderName
import org.jetbrains.compose.resources.stringResource

private sealed interface Versions {
    data object Loading : Versions
    data class Loaded(val list: List<LoaderVersion>) : Versions
    data class Failed(val message: String) : Versions
}

/** Install options for one game version: vanilla, or a mod loader on top. */
@Composable
fun InstallDialog(
    gameVersion: String,
    installed: Boolean,
    /** Cleanroom straight from its repository instead of through its download page. */
    cleanroomDirect: Boolean,
    loadVersions: suspend (LoaderKind, String) -> List<LoaderVersion>,
    onDismiss: () -> Unit,
    onInstall: (InstallRequest) -> Unit,
) {
    // Opened on an installed version, the dialog is there to add a loader.
    var loader by remember { mutableStateOf(if (installed) LoaderKind.FABRIC else null) }
    var versions by remember { mutableStateOf<Versions>(Versions.Loading) }
    var chosen by remember { mutableStateOf<String?>(null) }
    var fabricApi by remember { mutableStateOf(true) }
    var name by remember { mutableStateOf("") }
    var nameEdited by remember { mutableStateOf(false) }
    // Cleanroom through its download page: the installer the user picked there.
    val viaPage = loader == LoaderKind.CLEANROOM && !cleanroomDirect
    var installer by remember { mutableStateOf<String?>(null) }
    var pageOpen by remember { mutableStateOf(false) }

    LaunchedEffect(loader) {
        val kind = loader ?: return@LaunchedEffect
        if (viaPage) {
            versions = Versions.Loaded(emptyList())
            chosen = installer?.let(::cleanroomInstallerVersion)
            return@LaunchedEffect
        }
        versions = Versions.Loading
        chosen = null
        versions = runCatching { loadVersions(kind, gameVersion) }
            .fold({ Versions.Loaded(it) }, { Versions.Failed(it.message ?: it.toString()) })
        val list = (versions as? Versions.Loaded)?.list.orEmpty()
        chosen = (list.firstOrNull { it.stable } ?: list.firstOrNull())?.version
    }
    LaunchedEffect(loader, chosen) {
        if (!nameEdited) name = suggestedName(gameVersion, loader, chosen)
    }

    val canInstall = loader == null || (chosen != null && name.isNotBlank())
    val chooseInstallerTitle = stringResource(Res.string.action_choose_file)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(Res.string.install_dialog_title, gameVersion)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(Res.string.install_loader), style = MaterialTheme.typography.labelLarge)
                ConnectedChoice(
                    options = listOf(null, LoaderKind.FABRIC, LoaderKind.QUILT, LoaderKind.FORGE, LoaderKind.NEO_FORGE, LoaderKind.OPTI_FINE),
                    selected = loader,
                    label = { it?.let(::loaderName) ?: stringResource(Res.string.install_loader_none) },
                    onSelect = { loader = it },
                    fill = false,
                )
                Text(stringResource(Res.string.install_more_loaders), style = MaterialTheme.typography.labelLarge)
                ConnectedChoice(
                    options = moreLoaders(gameVersion),
                    selected = loader,
                    label = { loaderName(it!!) },
                    onSelect = { loader = it },
                    fill = false,
                )
                when (val kind = loader) {
                    null -> Hint(stringResource(Res.string.install_vanilla_hint))
                    else -> {
                        if (viaPage) {
                            Hint(stringResource(Res.string.install_cleanroom_page_hint))
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                FilledTonalButton(onClick = { pageOpen = true }, shapes = ButtonDefaults.shapes()) {
                                    Text(stringResource(Res.string.install_cleanroom_open_page))
                                }
                                TextButton(onClick = {
                                    chooseFile(chooseInstallerTitle, listOf("jar"))?.let { path ->
                                        cleanroomInstallerVersion(path)?.let { v ->
                                            installer = path
                                            chosen = v
                                        }
                                    }
                                }) { Text(chooseInstallerTitle) }
                            }
                            chosen?.let { Text(stringResource(Res.string.install_cleanroom_picked, it), style = MaterialTheme.typography.titleSmall) }
                        } else {
                            LoaderVersionPicker(versions, chosen, onChoose = { chosen = it })
                        }
                        OutlinedTextField(
                            value = name,
                            onValueChange = {
                                name = it.filterNot { c -> c in "/\\:" }
                                nameEdited = true
                            },
                            label = { Text(stringResource(Res.string.install_name)) },
                            supportingText = { Text(stringResource(Res.string.install_name_hint)) },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        if (kind == LoaderKind.FABRIC) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Column(Modifier.weight(1f)) {
                                    Text(stringResource(Res.string.install_fabric_api), style = MaterialTheme.typography.titleSmall)
                                    Hint(stringResource(Res.string.install_fabric_api_hint))
                                }
                                Switch(fabricApi, onCheckedChange = { fabricApi = it })
                            }
                        }
                        when (kind) {
                            LoaderKind.OPTI_FINE -> Hint(stringResource(Res.string.install_optifine_hint))
                            LoaderKind.RIFT -> Hint(stringResource(Res.string.install_rift_hint))
                            LoaderKind.CLEANROOM -> Hint(stringResource(Res.string.install_cleanroom_hint))
                            else -> {}
                        }
                    }
                }
            }
        },
        confirmButton = {
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
            ) { Text(stringResource(Res.string.action_install)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_cancel)) } },
    )
    if (pageOpen) {
        CleanroomPageWindow(
            onPicked = { url ->
                cleanroomInstallerVersion(url)?.let { v ->
                    installer = url
                    chosen = v
                    pageOpen = false
                }
            },
            onClose = { pageOpen = false },
        )
    }
}

/** Loaders for few or old game versions, offered only where they apply. */
private fun moreLoaders(gameVersion: String): List<LoaderKind?> = buildList {
    add(LoaderKind.LEGACY_FABRIC)
    add(LoaderKind.ORNITHE)
    if (gameVersion == "1.12.2") add(LoaderKind.CLEANROOM)
    if (gameVersion == "1.13.2") add(LoaderKind.RIFT)
}

@Composable
private fun LoaderVersionPicker(versions: Versions, chosen: String?, onChoose: (String) -> Unit) {
    when (versions) {
        Versions.Loading -> Box(Modifier.fillMaxWidth().height(56.dp), contentAlignment = Alignment.Center) {
            LoadingIndicator(Modifier.size(40.dp))
        }
        is Versions.Failed -> Text(versions.message, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
        is Versions.Loaded -> if (versions.list.isEmpty()) {
            Text(stringResource(Res.string.install_no_loader_versions), color = MaterialTheme.colorScheme.error)
        } else {
            var open by remember { mutableStateOf(false) }
            val current = versions.list.firstOrNull { it.version == chosen }
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
                    versions.list.take(200).forEach { v ->
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

private fun suggestedName(game: String, loader: LoaderKind?, version: String?): String {
    if (loader == null || version == null) return ""
    // Forge versions repeat the game version ("1.21-51.0.33").
    return "$game-${loaderName(loader)} ${version.removePrefix("$game-")}"
}
