package dev.sulfur.feature.modpacks

import dev.sulfur.ui.components.SelectField
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sulfur.feature.mods.ModRow
import dev.sulfur.feature.mods.SortMenu
import dev.sulfur.ffi.ModpackVersion
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_cancel
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_load_more
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.install_dialog_title
import dev.sulfur.resources.install_name
import dev.sulfur.resources.modpack_name_hint
import dev.sulfur.resources.modpack_no_loader
import dev.sulfur.resources.modpack_no_versions
import dev.sulfur.resources.modpack_unstable
import dev.sulfur.resources.modpack_version
import dev.sulfur.resources.modpack_version_label
import dev.sulfur.resources.modpacks_from_file
import dev.sulfur.resources.modpacks_search
import dev.sulfur.resources.n_files
import dev.sulfur.resources.no_results
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.SearchToolbar
import dev.sulfur.ui.components.loaderName
import dev.sulfur.util.chooseFile
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.pluralStringResource
import org.jetbrains.compose.resources.stringResource

/** Modrinth modpacks, each installed as a version of its own. */
@Composable
fun ModpacksPane(viewModel: ModpacksViewModel, modifier: Modifier = Modifier) {
    val ui by viewModel.ui.collectAsState()
    LaunchedEffect(Unit) { viewModel.ensureLoaded() }
    val chooseTitle = stringResource(Res.string.modpacks_from_file)

    Column(modifier, verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
        SearchToolbar(ui.query, viewModel::setQuery, stringResource(Res.string.modpacks_search)) {
            SortMenu(ui.sort, viewModel::setSort)
            FilledTonalButton(
                onClick = { chooseFile(chooseTitle, listOf("mrpack"))?.let(viewModel::openFile) },
                shapes = ButtonDefaults.shapes(),
            ) {
                Icon(painterResource(Res.drawable.ic_folder_open), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(chooseTitle)
            }
        }
        LazyColumn(
            Modifier.fillMaxWidth().weight(1f),
            verticalArrangement = Arrangement.spacedBy(6.dp),
            contentPadding = PaddingValues(bottom = 16.dp),
        ) {
            ui.error?.let { error ->
                item(key = "error") {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(error, color = MaterialTheme.colorScheme.error, modifier = Modifier.weight(1f))
                        TextButton(onClick = { viewModel.search() }) { Text(stringResource(Res.string.action_retry)) }
                    }
                }
            }
            items(ui.hits, key = { it.id }) { hit ->
                ModRow(hit, installing = false, canInstall = true, onInstall = { viewModel.open(hit) })
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

    when (val target = ui.target) {
        is PackTarget.Remote -> RemotePackDialog(target, viewModel)
        is PackTarget.Local -> {
            val info = target.info
            PackDialog(
                title = info.name,
                defaultName = "${info.name} ${info.version}",
                onDismiss = viewModel::close,
                onInstall = { name -> viewModel.installFile(target.path, name) },
            ) {
                val loader = info.loader?.let { "${loaderName(it)} ${info.loaderVersion.orEmpty()}".trim() }
                    ?: stringResource(Res.string.modpack_no_loader)
                Text(
                    listOf("Minecraft ${info.gameVersion}", loader, pluralStringResource(Res.plurals.n_files, info.files.toInt(), info.files.toInt()))
                        .joinToString(" · "),
                    style = MaterialTheme.typography.titleSmall,
                )
                info.summary?.takeIf { it.isNotBlank() }?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        null -> {}
    }
}

@Composable
private fun RemotePackDialog(target: PackTarget.Remote, viewModel: ModpacksViewModel) {
    val versions = target.versions
    var chosen by remember(versions) { mutableStateOf(versions?.let { list -> list.firstOrNull { it.stable } ?: list.firstOrNull() }) }
    val version = chosen
    PackDialog(
        title = target.hit.title,
        defaultName = version?.let { "${target.hit.title} ${it.versionNumber}" },
        onDismiss = viewModel::close,
        onInstall = version?.let { v -> { name -> viewModel.install(v, name) } },
    ) {
        when {
            versions == null -> Box(Modifier.fillMaxWidth().height(56.dp), contentAlignment = Alignment.Center) {
                LoadingIndicator(Modifier.size(40.dp))
            }
            target.error != null -> Text(target.error, color = MaterialTheme.colorScheme.error)
            versions.isEmpty() -> Text(stringResource(Res.string.modpack_no_versions), color = MaterialTheme.colorScheme.error)
            else -> PackVersionPicker(versions, version, onChoose = { chosen = it })
        }
        if (target.hit.description.isNotBlank()) {
            Text(target.hit.description, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** The install dialog shared by Modrinth packs and files: what the pack is, then the new version's name. */
@Composable
private fun PackDialog(
    title: String,
    /** Suggested version name; null until a release is chosen. */
    defaultName: String?,
    onDismiss: () -> Unit,
    /** Null while there is nothing to install. */
    onInstall: ((String) -> Unit)?,
    content: @Composable () -> Unit,
) {
    var name by remember { mutableStateOf("") }
    var edited by remember { mutableStateOf(false) }
    LaunchedEffect(defaultName) {
        if (!edited) name = defaultName.orEmpty().filterNot { it in "/\\:" }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(Res.string.install_dialog_title, title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                content()
                OutlinedTextField(
                    value = name,
                    onValueChange = {
                        name = it.filterNot { c -> c in "/\\:" }
                        edited = true
                    },
                    label = { Text(stringResource(Res.string.install_name)) },
                    supportingText = { Text(stringResource(Res.string.modpack_name_hint)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            Button(
                onClick = { onInstall?.invoke(name.trim()) },
                enabled = onInstall != null && name.isNotBlank(),
                shapes = ButtonDefaults.shapes(),
            ) { Text(stringResource(Res.string.action_install)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_cancel)) } },
    )
}

@Composable
private fun PackVersionPicker(versions: List<ModpackVersion>, chosen: ModpackVersion?, onChoose: (ModpackVersion) -> Unit) {
    SelectField(
        value = chosen?.let { versionLabel(it) }.orEmpty(),
        label = stringResource(Res.string.modpack_version),
        modifier = Modifier.fillMaxWidth(),
        supportingText = chosen?.let { v -> { Text(v.loaders.joinToString(" · ", transform = ::modrinthLoaderName)) } },
    ) { close ->
        versions.take(200).forEach { v ->
            DropdownMenuItem(
                text = { Text(versionLabel(v)) },
                onClick = {
                    onChoose(v)
                    close()
                },
            )
        }
    }
}

@Composable
private fun versionLabel(v: ModpackVersion): String {
    val label = stringResource(Res.string.modpack_version_label, v.versionNumber, v.gameVersions.joinToString(", "))
    return if (v.stable) label else stringResource(Res.string.modpack_unstable, label)
}

/** Modrinth's loader ids as product names. */
private fun modrinthLoaderName(id: String): String = when (id) {
    "fabric" -> "Fabric"
    "forge" -> "Forge"
    "neoforge" -> "NeoForge"
    "quilt" -> "Quilt"
    else -> id.replaceFirstChar { it.uppercase() }
}
