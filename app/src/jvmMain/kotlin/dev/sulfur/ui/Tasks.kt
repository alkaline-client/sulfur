package dev.sulfur.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.CircularWavyProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProgressIndicatorDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.sulfur.data.isRunning
import dev.sulfur.ffi.InstallSnapshot
import dev.sulfur.ffi.InstallStage
import dev.sulfur.ffi.TaskKind
import dev.sulfur.resources.Res
import dev.sulfur.resources.action_cancel_install
import dev.sulfur.resources.action_close
import dev.sulfur.resources.action_dismiss
import dev.sulfur.resources.ic_check
import dev.sulfur.resources.ic_close
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.ic_deployed_code
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_error
import dev.sulfur.resources.ic_inventory_2
import dev.sulfur.resources.ic_terminal
import dev.sulfur.resources.install_cancelled
import dev.sulfur.resources.install_done
import dev.sulfur.resources.install_failed
import dev.sulfur.resources.install_running
import dev.sulfur.resources.install_stage_building
import dev.sulfur.resources.install_stage_downloading
import dev.sulfur.resources.install_stage_extracting
import dev.sulfur.resources.install_stage_loader
import dev.sulfur.resources.install_stage_preparing
import dev.sulfur.resources.task_build_done
import dev.sulfur.resources.task_build_failed
import dev.sulfur.resources.task_build_running
import dev.sulfur.resources.task_bytes
import dev.sulfur.resources.task_eta
import dev.sulfur.resources.task_files
import dev.sulfur.resources.task_java_done
import dev.sulfur.resources.task_java_failed
import dev.sulfur.resources.task_java_running
import dev.sulfur.resources.task_reused
import dev.sulfur.resources.task_speed
import dev.sulfur.resources.tasks_clear_finished
import dev.sulfur.resources.tasks_empty
import dev.sulfur.resources.tasks_open
import dev.sulfur.resources.tasks_summary
import dev.sulfur.resources.tasks_title
import dev.sulfur.ui.components.ShapeIcon
import dev.sulfur.ui.components.formatBytes
import dev.sulfur.ui.theme.Corners
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

/**
 * The button for the task list, at the page's bottom end on every page while
 * there are tasks (like PCL's download button): it shows how far the running
 * ones are, or how the last ones ended.
 */
@Composable
fun BoxScope.TasksButton(tasks: List<InstallSnapshot>, onClick: () -> Unit) {
    val running = tasks.filter { it.isRunning }
    AnimatedVisibility(
        visible = tasks.isNotEmpty(),
        enter = scaleIn(MaterialTheme.motionScheme.fastSpatialSpec()) + fadeIn(MaterialTheme.motionScheme.fastEffectsSpec()),
        exit = scaleOut(MaterialTheme.motionScheme.fastSpatialSpec()) + fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()),
        modifier = Modifier.align(Alignment.BottomEnd).padding(end = 28.dp, bottom = 28.dp),
    ) {
        val failed = tasks.any { it.stage == InstallStage.FAILED }
        FloatingActionButton(
            onClick = onClick,
            containerColor = if (failed && running.isEmpty()) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.primaryContainer,
        ) {
            BadgedBox(badge = { if (running.size > 1) Badge { Text("${running.size}") } }) {
                Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                    if (running.isNotEmpty()) {
                        val progress by animateFloatAsState(overallFraction(running), ProgressIndicatorDefaults.ProgressAnimationSpec)
                        if (running.all { it.indeterminate }) {
                            CircularWavyProgressIndicator(Modifier.size(40.dp))
                        } else {
                            CircularWavyProgressIndicator(progress = { progress }, modifier = Modifier.size(40.dp))
                        }
                    }
                    Icon(
                        painterResource(
                            when {
                                running.isNotEmpty() -> Res.drawable.ic_download
                                failed -> Res.drawable.ic_error
                                else -> Res.drawable.ic_check
                            }
                        ),
                        contentDescription = stringResource(Res.string.tasks_open),
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
    }
}

/**
 * Every task of this session in a sheet over the page's end, below the
 * title bar ([top] high) so the window buttons stay reachable; Escape or a
 * click beside it closes it.
 */
@Composable
fun TasksSheet(
    open: Boolean,
    top: Dp,
    tasks: List<InstallSnapshot>,
    onCancel: (String) -> Unit,
    onDismiss: (String) -> Unit,
    onClearFinished: () -> Unit,
    onClose: () -> Unit,
) {
    val motion = MaterialTheme.motionScheme
    AnimatedVisibility(open, enter = fadeIn(motion.defaultEffectsSpec()), exit = fadeOut(motion.fastEffectsSpec())) {
        Box(
            Modifier.fillMaxSize()
                .padding(top = top)
                .background(MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f))
                .clickable(remember { MutableInteractionSource() }, indication = null, onClick = onClose)
        )
    }
    Box(Modifier.fillMaxSize().padding(top = top), contentAlignment = Alignment.CenterEnd) {
        AnimatedVisibility(
            open,
            enter = slideInHorizontally(motion.defaultSpatialSpec()) { it / 3 } + fadeIn(motion.defaultEffectsSpec()),
            exit = slideOutHorizontally(motion.fastSpatialSpec()) { it / 3 } + fadeOut(motion.fastEffectsSpec()),
        ) {
            val focus = remember { FocusRequester() }
            LaunchedEffect(Unit) { focus.requestFocus() }
            Surface(
                shape = Corners.page,
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                shadowElevation = 8.dp,
                modifier = Modifier
                    .fillMaxHeight()
                    .width(420.dp)
                    .padding(end = 16.dp, bottom = 16.dp)
                    .focusRequester(focus)
                    .focusable()
                    .onKeyEvent { event -> if (event.key == Key.Escape) { onClose(); true } else false }
                    // Clicks inside the sheet do not reach the scrim.
                    .clickable(remember { MutableInteractionSource() }, indication = null) {},
            ) {
                Column(Modifier.padding(top = 16.dp)) {
                    Row(Modifier.padding(start = 20.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(stringResource(Res.string.tasks_title), style = MaterialTheme.typography.titleLargeEmphasized)
                            val running = tasks.filter { it.isRunning }
                            if (running.isNotEmpty()) {
                                Text(
                                    stringResource(
                                        Res.string.tasks_summary,
                                        running.size.toString(),
                                        formatBytes(running.sumOf { it.bytesPerSecond }),
                                    ),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        if (tasks.any { !it.isRunning }) {
                            TextButton(onClick = onClearFinished) { Text(stringResource(Res.string.tasks_clear_finished)) }
                        }
                        IconButton(onClick = onClose) {
                            Icon(painterResource(Res.drawable.ic_close), stringResource(Res.string.action_close))
                        }
                    }
                    if (tasks.isEmpty()) {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            Text(stringResource(Res.string.tasks_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    } else {
                        LazyColumn(
                            contentPadding = PaddingValues(12.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            items(tasks, key = { it.versionId }) { task ->
                                TaskCard(
                                    task,
                                    onCancel = { onCancel(task.versionId) },
                                    onDismiss = { onDismiss(task.versionId) },
                                    modifier = Modifier.animateItem(),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** One task: what it is, its stage, its progress, and how fast it goes. */
@Composable
private fun TaskCard(task: InstallSnapshot, onCancel: () -> Unit, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    val running = task.isRunning
    val failed = task.stage == InstallStage.FAILED
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = Corners.card,
        color = when {
            failed -> colors.errorContainer
            running -> colors.surfaceContainerHigh
            else -> colors.surfaceContainer
        },
        modifier = modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(start = 12.dp, end = 4.dp, top = 10.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ShapeIcon(
                    painterResource(kindIcon(task.kind)),
                    MaterialShapes.Cookie6Sided,
                    size = 36.dp,
                    rotating = running,
                    container = if (failed) colors.error else colors.secondaryContainer,
                    content = if (failed) colors.onError else colors.onSecondaryContainer,
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(taskTitle(task), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        task.error ?: stageLabel(task.stage),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (failed) colors.onErrorContainer else colors.onSurfaceVariant,
                        maxLines = if (failed) 4 else 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (running) {
                    IconButton(onClick = onCancel) {
                        Icon(painterResource(Res.drawable.ic_close), stringResource(Res.string.action_cancel_install))
                    }
                } else {
                    TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_dismiss)) }
                }
            }
            if (running) {
                Column(Modifier.padding(end = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (task.indeterminate) {
                        LinearWavyProgressIndicator(Modifier.fillMaxWidth())
                    } else {
                        val progress by animateFloatAsState(task.fraction.toFloat(), ProgressIndicatorDefaults.ProgressAnimationSpec)
                        LinearWavyProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
                    }
                    task.detail?.let { line ->
                        Text(
                            line,
                            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (task.filesTotal > 0u) TaskStats(task)
                }
            }
        }
    }
}

/** Files, bytes, speed and time left, as short facts on one line. */
@Composable
private fun TaskStats(task: InstallSnapshot) {
    val facts = buildList {
        add(stringResource(Res.string.task_files, task.filesDone.toString(), task.filesTotal.toString()))
        val already = task.filesSkipped + task.filesReused
        if (already > 0u) add(stringResource(Res.string.task_reused, already.toString()))
        if (task.bytesTotal > 0u) add(stringResource(Res.string.task_bytes, formatBytes(task.bytesDone), formatBytes(task.bytesTotal)))
        if (task.bytesPerSecond > 0.0) {
            add(stringResource(Res.string.task_speed, formatBytes(task.bytesPerSecond)))
            val left = (task.bytesTotal.toDouble() - task.bytesDone.toDouble()) / task.bytesPerSecond
            if (left in 1.0..86_400.0) add(stringResource(Res.string.task_eta, formatDuration(left.toLong())))
        }
    }
    Text(
        facts.joinToString(" · "),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 2,
    )
}

/** Whether a task cannot tell how far it is (a build, or before the file list is known). */
private val InstallSnapshot.indeterminate: Boolean
    get() = stage == InstallStage.BUILDING || (stage == InstallStage.PREPARING && filesTotal == 0uL)

/** The share of the running tasks' work that is done, by bytes when they are known. */
private fun overallFraction(running: List<InstallSnapshot>): Float {
    val total = running.sumOf { it.bytesTotal.toDouble() }
    return if (total > 0) (running.sumOf { it.bytesDone.toDouble() } / total).toFloat()
    else running.map { it.fraction }.average().toFloat()
}

private fun formatDuration(seconds: Long): String =
    if (seconds < 60) "${seconds}s" else "%d:%02d".format(seconds / 60, seconds % 60)

private fun kindIcon(kind: TaskKind): DrawableResource = when (kind) {
    TaskKind.GAME -> Res.drawable.ic_deployed_code
    TaskKind.MODPACK -> Res.drawable.ic_inventory_2
    TaskKind.JAVA -> Res.drawable.ic_coffee
    TaskKind.BUILD -> Res.drawable.ic_terminal
}

@Composable
private fun taskTitle(task: InstallSnapshot): String {
    val failed = task.stage == InstallStage.FAILED
    val done = task.stage == InstallStage.DONE
    return stringResource(
        when {
            task.stage == InstallStage.CANCELLED -> Res.string.install_cancelled
            task.kind == TaskKind.BUILD -> if (failed) Res.string.task_build_failed else if (done) Res.string.task_build_done else Res.string.task_build_running
            task.kind == TaskKind.JAVA -> if (failed) Res.string.task_java_failed else if (done) Res.string.task_java_done else Res.string.task_java_running
            failed -> Res.string.install_failed
            done -> Res.string.install_done
            else -> Res.string.install_running
        },
        task.versionId,
    )
}

@Composable
private fun stageLabel(stage: InstallStage): String = when (stage) {
    InstallStage.PREPARING -> stringResource(Res.string.install_stage_preparing)
    InstallStage.DOWNLOADING -> stringResource(Res.string.install_stage_downloading)
    InstallStage.EXTRACTING -> stringResource(Res.string.install_stage_extracting)
    InstallStage.LOADER -> stringResource(Res.string.install_stage_loader)
    InstallStage.BUILDING -> stringResource(Res.string.install_stage_building)
    else -> ""
}
