package dev.sulfur.feature.launch

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.data.SulfurRepository
import dev.sulfur.ffi.InstalledMod
import dev.sulfur.ffi.InstalledVersion
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.ProjectKind
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_remove
import dev.sulfur.resources.ic_delete
import dev.sulfur.resources.ic_extension
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.ic_search
import dev.sulfur.resources.kind_mods
import dev.sulfur.resources.kind_resource_packs
import dev.sulfur.resources.kind_shaders
import dev.sulfur.resources.mods_folder_empty
import dev.sulfur.resources.mods_no_loader
import dev.sulfur.resources.version_mods_count
import dev.sulfur.resources.version_mods_find
import dev.sulfur.resources.version_mods_open_folder
import dev.sulfur.ui.components.ConnectedChoice
import dev.sulfur.ui.components.Notice
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.ui.components.formatBytes
import dev.sulfur.util.openFolder
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import java.io.File

/** What the version page manages; data packs belong to worlds and stay on the Download page. */
private val Kinds = listOf(ProjectKind.MOD, ProjectKind.RESOURCE_PACK, ProjectKind.SHADER)

private fun ProjectKind.title(): StringResource = when (this) {
    ProjectKind.RESOURCE_PACK -> Res.string.kind_resource_packs
    ProjectKind.SHADER -> Res.string.kind_shaders
    else -> Res.string.kind_mods
}

/** The folder the game reads this kind from, inside the version's game folder. */
private fun ProjectKind.folder(): String = when (this) {
    ProjectKind.RESOURCE_PACK -> "resourcepacks"
    ProjectKind.SHADER -> "shaderpacks"
    else -> "mods"
}

/**
 * A version's mods, resource packs and shaders: turn them on or off, remove
 * them, open their folder, or find more on the Download page.
 */
@Composable
internal fun VersionModsSection(
    version: InstalledVersion,
    repository: SulfurRepository,
    onFindMore: (ProjectKind) -> Unit,
) {
    var kind by rememberSaveable(version.id) { mutableStateOf(ProjectKind.MOD) }
    var items by remember(version.id, kind) { mutableStateOf<List<InstalledMod>?>(null) }
    val scope = rememberCoroutineScope()
    suspend fun reload() {
        items = repository.installedMods(version.id, kind, null)
    }
    LaunchedEffect(version.id, kind) { reload() }
    fun change(action: suspend () -> Unit) {
        scope.launch {
            runCatching { action() }.onFailure(repository::notifyError)
            reload()
        }
    }

    ConnectedChoice(
        options = Kinds,
        selected = kind,
        label = { stringResource(it.title()) },
        onSelect = { kind = it },
        fill = false,
    )
    val noLoader = kind == ProjectKind.MOD && version.loader.let { it == null || it == LoaderKind.OPTI_FINE }
    if (noLoader) {
        Notice(
            painterResource(Res.drawable.ic_extension),
            stringResource(Res.string.mods_no_loader),
            container = MaterialTheme.colorScheme.tertiaryContainer,
            content = MaterialTheme.colorScheme.onTertiaryContainer,
        )
    }
    val list = items.orEmpty()
    SectionCard(
        stringResource(kind.title()),
        icon = painterResource(Res.drawable.ic_extension),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                if (list.isEmpty()) stringResource(Res.string.mods_folder_empty)
                else stringResource(Res.string.version_mods_count, list.count { it.enabled }, list.size),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(
                onClick = { scope.launch { openFolder(File(version.runDir, kind.folder()).path) } },
                shapes = ButtonDefaults.shapes(),
            ) {
                Icon(painterResource(Res.drawable.ic_folder_open), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(Res.string.version_mods_open_folder))
            }
            FilledTonalButton(onClick = { onFindMore(kind) }, enabled = !noLoader, shapes = ButtonDefaults.shapes()) {
                Icon(painterResource(Res.drawable.ic_search), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(Res.string.version_mods_find))
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            list.forEach { mod ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ShapeIcon(painterResource(Res.drawable.ic_extension), MaterialShapes.Square, size = 32.dp)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f).alpha(if (mod.enabled) 1f else 0.6f)) {
                        Text(
                            mod.fileName.removeSuffix(".disabled"),
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            formatBytes(mod.size.toDouble()),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(mod.enabled, onCheckedChange = { on ->
                        change { repository.setModEnabled(version.id, kind, null, mod.fileName, on) }
                    })
                    IconButton(onClick = { change { repository.removeMod(version.id, kind, null, mod.fileName) } }) {
                        Icon(painterResource(Res.drawable.ic_delete), stringResource(Res.string.action_remove))
                    }
                }
            }
        }
    }
}
