package dev.sulfur.feature.launch

import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.UiMessage
import dev.sulfur.ffi.CoreException
import dev.sulfur.ffi.GameSession
import dev.sulfur.resources.Res
import dev.sulfur.resources.msg_install_first
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

sealed interface GameState {
    data object Idle : GameState
    data class Launching(val versionId: String) : GameState
    data class Running(val versionId: String, val pid: UInt?) : GameState
    data class Exited(val versionId: String, val code: Int?) : GameState
    data class Failed(val message: String) : GameState
}

class LaunchViewModel(
    private val repository: SulfurRepository,
    private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow<GameState>(GameState.Idle)
    val state: StateFlow<GameState> = _state.asStateFlow()

    private val _logs = MutableStateFlow<List<String>>(emptyList())
    /** The last lines of game output. */
    val logs: StateFlow<List<String>> = _logs.asStateFlow()

    private var session: GameSession? = null

    /** The remembered selection if it is still installed, else the first installed version. */
    val selected: StateFlow<String?> = combine(repository.settings, repository.installed) { settings, installed ->
        settings.selectedVersion?.takeIf { id -> installed.any { it.id == id } } ?: installed.firstOrNull()?.id
    }.stateIn(scope, SharingStarted.Eagerly, null)

    fun select(versionId: String) {
        repository.updateSettings { it.copy(selectedVersion = versionId) }
    }

    fun play() {
        val id = selected.value ?: run {
            repository.notify(UiMessage(Res.string.msg_install_first))
            return
        }
        if (_state.value is GameState.Launching || _state.value is GameState.Running) return
        _state.value = GameState.Launching(id)
        _logs.value = emptyList()
        scope.launch {
            val started = try {
                repository.launch(id)
            } catch (e: CoreException) {
                _state.value = GameState.Failed(e.message ?: e.toString())
                return@launch
            }
            session?.close()
            session = started
            _state.value = GameState.Running(id, started.pid())
            follow(started, id)
        }
    }

    fun stop() {
        session?.kill()
    }

    fun dismiss() {
        if (_state.value is GameState.Exited || _state.value is GameState.Failed) _state.value = GameState.Idle
    }

    private suspend fun follow(game: GameSession, id: String) {
        var next = 0uL
        while (true) {
            val running = game.isRunning()
            val chunk = game.logsSince(next)
            next = chunk.next
            if (chunk.lines.isNotEmpty()) _logs.update { (it + chunk.lines).takeLast(MAX_LOG_LINES) }
            if (!running) break
            delay(250)
        }
        _state.value = GameState.Exited(id, game.exitCode())
    }

    private companion object {
        const val MAX_LOG_LINES = 400
    }
}
