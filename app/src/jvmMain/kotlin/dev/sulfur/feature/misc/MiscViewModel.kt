package dev.sulfur.feature.misc

import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.isJava
import dev.sulfur.ffi.InstallStage
import dev.sulfur.ffi.JavaRuntime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

class MiscViewModel(
    private val repository: SulfurRepository,
    private val scope: CoroutineScope,
) {
    private val _java = MutableStateFlow<List<JavaRuntime>?>(null)
    /** Detected Java runtimes; `null` until the first scan finishes. */
    val java: StateFlow<List<JavaRuntime>?> = _java.asStateFlow()

    private val _scanning = MutableStateFlow(false)
    val scanning: StateFlow<Boolean> = _scanning.asStateFlow()

    private val _launcherLog = MutableStateFlow<List<String>>(emptyList())
    val launcherLog: StateFlow<List<String>> = _launcherLog.asStateFlow()

    init {
        scanJava()
        // Pick up runtimes Sulfur just downloaded.
        scope.launch {
            repository.installs
                .map { list -> list.count { it.isJava && it.stage == InstallStage.DONE } }
                .distinctUntilChanged()
                .drop(1)
                .collect { scanJava() }
        }
    }

    fun scanJava() {
        if (_scanning.value) return
        _scanning.value = true
        scope.launch {
            _java.value = repository.detectJava()
            _scanning.value = false
        }
    }

    fun loadLauncherLog() {
        scope.launch {
            _launcherLog.value = withContext(Dispatchers.IO) {
                runCatching { File(repository.paths.logsDir, "sulfur.log").readLines().takeLast(200) }
                    .getOrDefault(emptyList())
            }
        }
    }
}
