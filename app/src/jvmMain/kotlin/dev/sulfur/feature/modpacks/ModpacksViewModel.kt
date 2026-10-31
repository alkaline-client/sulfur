package dev.sulfur.feature.modpacks

import dev.sulfur.data.SulfurRepository
import dev.sulfur.ffi.ModHit
import dev.sulfur.ffi.ModSort
import dev.sulfur.ffi.ModpackInfo
import dev.sulfur.ffi.ModpackVersion
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** The pack whose install dialog is open. */
sealed interface PackTarget {
    /** A Modrinth project; [versions] is null while its releases load. */
    data class Remote(val hit: ModHit, val versions: List<ModpackVersion>? = null, val error: String? = null) : PackTarget

    /** A `.mrpack` file on disk. */
    data class Local(val path: String, val info: ModpackInfo) : PackTarget
}

data class ModpacksUiState(
    val query: String = "",
    val sort: ModSort = ModSort.RELEVANCE,
    val hits: List<ModHit> = emptyList(),
    val total: ULong = 0uL,
    /** Results requested so far (the next page's offset). */
    val fetched: UInt = 0u,
    val loading: Boolean = false,
    val error: String? = null,
    val target: PackTarget? = null,
) {
    val canLoadMore: Boolean get() = !loading && fetched.toULong() < total
}

/** Modrinth modpacks: search, then install one release as a version of its own. */
@OptIn(FlowPreview::class)
class ModpacksViewModel(
    private val repository: SulfurRepository,
    private val scope: CoroutineScope,
) {
    private val _ui = MutableStateFlow(ModpacksUiState())
    val ui: StateFlow<ModpacksUiState> = _ui.asStateFlow()
    private var searchJob: Job? = null
    private var started = false

    init {
        scope.launch {
            _ui.map { it.query }.distinctUntilChanged().drop(1).debounce(400).collect { search() }
        }
    }

    /** Shows popular packs the first time the section opens. */
    fun ensureLoaded() {
        if (started) return
        started = true
        search()
    }

    fun setQuery(query: String) = _ui.update { it.copy(query = query) }

    fun setSort(sort: ModSort) {
        _ui.update { it.copy(sort = sort) }
        search()
    }

    fun search(append: Boolean = false) {
        val state = _ui.value
        searchJob?.cancel()
        _ui.update { it.copy(loading = true, error = null, hits = if (append) it.hits else emptyList()) }
        searchJob = scope.launch {
            try {
                val offset = if (append) state.fetched else 0u
                val page = repository.searchModpacks(state.query, state.sort, offset)
                _ui.update {
                    it.copy(
                        loading = false,
                        hits = if (append) it.hits + page.hits.filter { h -> it.hits.none { o -> o.id == h.id } } else page.hits,
                        total = page.total,
                        fetched = offset + PAGE,
                    )
                }
            } catch (e: CancellationException) {
                // A newer search replaced this one.
                throw e
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.message ?: e.toString()) }
            }
        }
    }

    fun open(hit: ModHit) {
        _ui.update { it.copy(target = PackTarget.Remote(hit)) }
        scope.launch {
            val loaded = runCatching { repository.modpackVersions(hit.id) }
            _ui.update {
                val target = it.target as? PackTarget.Remote
                if (target?.hit?.id != hit.id) it
                else it.copy(target = target.copy(versions = loaded.getOrNull() ?: emptyList(), error = loaded.exceptionOrNull()?.message))
            }
        }
    }

    fun openFile(path: String) {
        scope.launch {
            runCatching { repository.readModpackFile(path) }
                .onSuccess { info -> _ui.update { it.copy(target = PackTarget.Local(path, info)) } }
                .onFailure(repository::notifyError)
        }
    }

    fun close() = _ui.update { it.copy(target = null) }

    fun install(version: ModpackVersion, name: String) {
        close()
        repository.installModpack(version, name)
    }

    fun installFile(path: String, name: String) {
        close()
        repository.installModpackFile(path, name)
    }

    private companion object {
        /** Results per page; matches the core. */
        const val PAGE = 20u
    }
}
