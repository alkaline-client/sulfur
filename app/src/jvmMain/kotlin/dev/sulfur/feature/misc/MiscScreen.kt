package dev.sulfur.feature.misc

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.data.SulfurRepository
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_open
import dev.sulfur.resources.action_reload
import dev.sulfur.resources.action_scan_again
import dev.sulfur.resources.app_tagline
import dev.sulfur.resources.app_version
import dev.sulfur.resources.credits
import dev.sulfur.resources.credits_list
import dev.sulfur.resources.folder_data
import dev.sulfur.resources.folder_game
import dev.sulfur.resources.folder_logs
import dev.sulfur.resources.folder_settings
import dev.sulfur.resources.folders
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.ic_description
import dev.sulfur.resources.ic_folder_open
import dev.sulfur.resources.ic_info
import dev.sulfur.resources.ic_rocket_launch_filled
import dev.sulfur.resources.java_looking
import dev.sulfur.resources.java_none
import dev.sulfur.resources.java_runtimes
import dev.sulfur.resources.launcher_log
import dev.sulfur.resources.launcher_log_empty
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.PagePadding
import dev.sulfur.ui.components.SectionCard
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.util.openFolder
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringArrayResource
import org.jetbrains.compose.resources.stringResource

@Composable
fun MiscScreen(viewModel: MiscViewModel, repository: SulfurRepository) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val twoColumns = maxWidth >= 900.dp
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = PagePadding, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(CardSpacing),
        ) {
            About(repository)
            val left: @Composable ColumnScope.() -> Unit = {
                Folders(repository)
                JavaSection(viewModel)
            }
            val right: @Composable ColumnScope.() -> Unit = {
                LauncherLog(viewModel)
                Credits()
            }
            if (twoColumns) {
                Row(horizontalArrangement = Arrangement.spacedBy(CardSpacing)) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(CardSpacing), content = left)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(CardSpacing), content = right)
                }
            } else {
                left()
                right()
            }
        }
    }
}

@Composable
private fun About(repository: SulfurRepository) {
    Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.tertiaryContainer, modifier = Modifier.fillMaxWidth()) {
        Row(Modifier.padding(20.dp), verticalAlignment = Alignment.CenterVertically) {
            ShapeIcon(
                painterResource(Res.drawable.ic_rocket_launch_filled),
                MaterialShapes.SoftBurst,
                size = 72.dp,
                container = MaterialTheme.colorScheme.tertiary,
                content = MaterialTheme.colorScheme.onTertiary,
                rotating = true,
            )
            Spacer(Modifier.width(20.dp))
            Column {
                Text("Sulfur", style = MaterialTheme.typography.headlineMediumEmphasized, color = MaterialTheme.colorScheme.onTertiaryContainer)
                Text(stringResource(Res.string.app_tagline), style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onTertiaryContainer)
                Text(stringResource(Res.string.app_version, repository.coreVersion), style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onTertiaryContainer)
            }
        }
    }
}

@Composable
private fun Folders(repository: SulfurRepository) {
    val paths = repository.paths
    val scope = rememberCoroutineScope()
    SectionCard(stringResource(Res.string.folders), icon = painterResource(Res.drawable.ic_folder_open)) {
        listOf(
            Res.string.folder_game to paths.gameDir,
            Res.string.folder_data to paths.dataDir,
            Res.string.folder_settings to paths.configDir,
            Res.string.folder_logs to paths.logsDir,
        ).forEach { (label, path) ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(label), style = MaterialTheme.typography.titleSmall)
                    Text(path, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2, overflow = TextOverflow.MiddleEllipsis)
                }
                Spacer(Modifier.width(8.dp))
                OutlinedButton(onClick = { scope.launch { openFolder(path) } }, shapes = ButtonDefaults.shapes()) {
                    Text(stringResource(Res.string.action_open))
                }
            }
        }
    }
}

@Composable
private fun JavaSection(viewModel: MiscViewModel) {
    val runtimes by viewModel.java.collectAsState()
    val scanning by viewModel.scanning.collectAsState()
    SectionCard(
        stringResource(Res.string.java_runtimes),
        icon = painterResource(Res.drawable.ic_coffee),
        trailing = {
            if (scanning) LoadingIndicator() else TextButton(onClick = viewModel::scanJava) { Text(stringResource(Res.string.action_scan_again)) }
        },
    ) {
        val found = runtimes
        when {
            found == null -> Text(stringResource(Res.string.java_looking))
            found.isEmpty() -> Text(stringResource(Res.string.java_none), color = MaterialTheme.colorScheme.error)
            else -> found.forEach { java ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Java ${java.major}", style = MaterialTheme.typography.titleSmall, modifier = Modifier.widthIn(min = 72.dp))
                    Spacer(Modifier.width(8.dp))
                    Column(Modifier.weight(1f)) {
                        Text(java.path, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
                        Text("${java.version} · ${java.vendor} · ${java.arch}", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
private fun LauncherLog(viewModel: MiscViewModel) {
    val lines by viewModel.launcherLog.collectAsState()
    LaunchedEffect(Unit) { viewModel.loadLauncherLog() }
    SectionCard(
        stringResource(Res.string.launcher_log),
        icon = painterResource(Res.drawable.ic_description),
        trailing = {
            FilledTonalButton(onClick = viewModel::loadLauncherLog, shapes = ButtonDefaults.shapes()) { Text(stringResource(Res.string.action_reload)) }
        },
    ) {
        Surface(shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.surfaceContainerHighest) {
            SelectionContainer {
                Column(Modifier.fillMaxWidth().heightIn(max = 220.dp).verticalScroll(rememberScrollState()).padding(12.dp)) {
                    if (lines.isEmpty()) Text(stringResource(Res.string.launcher_log_empty), style = MaterialTheme.typography.bodySmall)
                    lines.forEach {
                        Text(it, style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace))
                    }
                }
            }
        }
    }
}

@Composable
private fun Credits() {
    SectionCard(stringResource(Res.string.credits), icon = painterResource(Res.drawable.ic_info)) {
        stringArrayResource(Res.array.credits_list).forEach {
            Text("• $it", style = MaterialTheme.typography.bodySmall)
        }
    }
}
