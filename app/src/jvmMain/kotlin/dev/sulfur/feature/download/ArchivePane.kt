package dev.sulfur.feature.download

import androidx.compose.animation.AnimatedContent
import dev.sulfur.ui.theme.Corners
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularWavyProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProgressIndicatorDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sulfur.data.isRunning
import dev.sulfur.ffi.ArchivedVersion
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_install
import dev.sulfur.resources.action_open_in_browser
import dev.sulfur.resources.action_refresh_versions
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.archive_all
import dev.sulfur.resources.archive_alpha
import dev.sulfur.resources.archive_april_fools
import dev.sulfur.resources.archive_beta
import dev.sulfur.resources.archive_classic
import dev.sulfur.resources.archive_hint
import dev.sulfur.resources.archive_indev
import dev.sulfur.resources.archive_infdev
import dev.sulfur.resources.archive_loading
import dev.sulfur.resources.archive_preclassic
import dev.sulfur.resources.archive_release
import dev.sulfur.resources.ic_check_circle
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_history_edu
import dev.sulfur.resources.ic_open_in_new
import dev.sulfur.resources.installed
import dev.sulfur.resources.no_results
import dev.sulfur.ui.components.CardSpacing
import dev.sulfur.ui.components.SearchToolbar
import dev.sulfur.ui.components.RefreshButton
import dev.sulfur.ui.components.Notice
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.resources.archive_search
import dev.sulfur.util.openUrl
import androidx.compose.material3.MaterialShapes
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

/** Archive folders in the order the archive lists them, oldest era first. */
private val Categories: List<Pair<String, StringResource>> = listOf(
    "preclassic" to Res.string.archive_preclassic,
    "classic" to Res.string.archive_classic,
    "indev" to Res.string.archive_indev,
    "infdev" to Res.string.archive_infdev,
    "alpha" to Res.string.archive_alpha,
    "beta" to Res.string.archive_beta,
    "release" to Res.string.archive_release,
    "april-fools" to Res.string.archive_april_fools,
)

/**
 * Versions Mojang no longer offers, from Omniarchive. They are raw client
 * jars without launcher metadata, so they are listed by file name and kept
 * apart from the official version list.
 */
@Composable
internal fun ArchivePane(archive: ArchiveState, state: InstallState, viewModel: DownloadViewModel, modifier: Modifier = Modifier) {
    val scope = rememberCoroutineScope()
    Column(modifier, verticalArrangement = Arrangement.spacedBy(CardSpacing)) {
        SearchToolbar(archive.query, viewModel::setArchiveQuery, stringResource(Res.string.archive_search)) {
            RefreshButton(archive.loading, stringResource(Res.string.action_refresh_versions), onClick = { viewModel.loadArchive() })
        }
        Notice(
            painterResource(Res.drawable.ic_history_edu),
            stringResource(Res.string.archive_hint),
            container = MaterialTheme.colorScheme.surfaceContainerLow,
            content = MaterialTheme.colorScheme.onSurfaceVariant,
            iconTint = MaterialTheme.colorScheme.primary,
        ) {
            TextButton(onClick = { scope.launch { openUrl("https://omniarchive.net") } }) {
                Icon(painterResource(Res.drawable.ic_open_in_new), null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(stringResource(Res.string.action_open_in_browser))
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            FilterChip(
                selected = archive.category == null,
                onClick = { viewModel.showArchiveCategory(null) },
                label = { Text(stringResource(Res.string.archive_all)) },
            )
            Categories.filter { (key, _) -> archive.versions.any { it.category == key } }.forEach { (key, label) ->
                FilterChip(
                    selected = archive.category == key,
                    onClick = { viewModel.showArchiveCategory(key) },
                    label = { Text(stringResource(label)) },
                )
            }
        }
        when {
            archive.error != null && archive.versions.isEmpty() -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text(archive.error, color = MaterialTheme.colorScheme.error, modifier = Modifier.weight(1f))
                TextButton(onClick = { viewModel.loadArchive() }) { Text(stringResource(Res.string.action_retry)) }
            }
            archive.loading && archive.versions.isEmpty() ->
                Text(stringResource(Res.string.archive_loading), color = MaterialTheme.colorScheme.onSurfaceVariant)
            archive.shown.isEmpty() ->
                Text(stringResource(Res.string.no_results), color = MaterialTheme.colorScheme.onSurfaceVariant)
            else -> LazyVerticalGrid(
                columns = GridCells.Adaptive(minSize = 300.dp),
                modifier = Modifier.fillMaxWidth().weight(1f),
                contentPadding = PaddingValues(bottom = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                items(archive.shown, key = { it.id }) { jar ->
                    ArchiveRow(jar, state, onInstall = { viewModel.installArchived(jar.id) })
                }
            }
        }
    }
}

@Composable
private fun ArchiveRow(jar: ArchivedVersion, state: InstallState, onInstall: () -> Unit) {
    val category = Categories.firstOrNull { it.first == jar.category }?.second
    Surface(Modifier.fillMaxWidth(), shape = Corners.item, color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            ShapeIcon(painterResource(Res.drawable.ic_history_edu), MaterialShapes.Clover4Leaf, size = 32.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                // The raw file name: these builds have no official names.
                Text(jar.fileName, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOfNotNull(category?.let { stringResource(it) }, jar.size.ifBlank { null }).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            val active = state.active[jar.id]
            val target = when {
                active?.isRunning == true -> 1
                jar.id in state.installedIds || jar.installed -> 2
                else -> 0
            }
            val motion = MaterialTheme.motionScheme
            AnimatedContent(
                targetState = target,
                transitionSpec = { fadeIn(motion.defaultEffectsSpec()) togetherWith fadeOut(motion.fastEffectsSpec()) },
                contentAlignment = Alignment.CenterEnd,
                label = "archive-install",
            ) { s ->
                when (s) {
                    1 -> Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                        val progress by animateFloatAsState((active?.fraction ?: 0.0).toFloat(), ProgressIndicatorDefaults.ProgressAnimationSpec)
                        CircularWavyProgressIndicator(progress = { progress }, modifier = Modifier.size(32.dp))
                    }
                    2 -> AssistChip(
                        onClick = {},
                        label = { Text(stringResource(Res.string.installed)) },
                        leadingIcon = { Icon(painterResource(Res.drawable.ic_check_circle), null, Modifier.size(AssistChipDefaults.IconSize)) },
                    )
                    else -> FilledTonalButton(onClick = onInstall, shapes = ButtonDefaults.shapes()) {
                        Icon(painterResource(Res.drawable.ic_download), null, Modifier.size(ButtonDefaults.IconSize))
                        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                        Text(stringResource(Res.string.action_install))
                    }
                }
            }
        }
    }
}
