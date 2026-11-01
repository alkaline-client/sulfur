package dev.sulfur.feature.mods

import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.UiMessage
import dev.sulfur.ffi.InstalledMod
import dev.sulfur.ffi.ModHit
import dev.sulfur.ffi.ModQuery
import dev.sulfur.ffi.ModRef
import dev.sulfur.ffi.ModSort
import dev.sulfur.ffi.ModSource
import dev.sulfur.ffi.ProjectKind
import dev.sulfur.ffi.mergeModHits
import dev.sulfur.resources.Res
import dev.sulfur.resources.msg_list_installed
import dev.sulfur.resources.msg_mod_installed
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Where to search: one site, or both with the same project merged into one result. */
enum class SourceChoice(val sources: List<ModSource>) {
    Both(listOf(ModSource.MODRINTH, ModSource.CURSE_FORGE)),
    Modrinth(listOf(ModSource.MODRINTH)),
    CurseForge(listOf(ModSource.CURSE_FORGE)),
}

data class ModsUiState(
    val source: SourceChoice = SourceChoice.Both,
    val kind: ProjectKind = ProjectKind.MOD,
    val query: String = "",
    val sort: ModSort = ModSort.RELEVANCE,
    /** Installed version the search is for; its folder is shown alongside. */
    val versionId: String? = null,
    /** Worlds of the version, for data packs. */
    val worlds: List<String> = emptyList(),
    val world: String? = null,
    val hits: List<ModHit> = emptyList(),
    val total: ULong = 0uL,
    /** Results requested from each source so far (the next page's offset). */
    val fetched: UInt = 0u,
    val loading: Boolean = false,
    val error: String? = null,
    /** Keys ([key]) of projects being installed. */
    val installing: Set<String> = emptySet(),
    val installedMods: List<InstalledMod> = emptyList(),
    /** Projects picked to download together (Ctrl-click), in the order picked. */
    val downloadList: List<ListedProject> = emptyList(),
) {
    val canLoadMore: Boolean get() = !loading && fetched.toULong() < total

    /** Picking projects: a click adds or removes instead of installing. */
    val picking: Boolean get() = downloadList.isNotEmpty()

    fun isListed(hit: ModHit): Boolean = downloadList.any { it.hit.key == hit.key && it.kind == kind }
}

/** A project in the download list, with the kind it was found as (mods, shaders…). */
data class ListedProject(val hit: ModHit, val kind: ProjectKind)

/** Kinds a download list can hold; data packs go into one world at a time and are left out. */
val ProjectKind.listable: Boolean get() = this != ProjectKind.DATA_PACK

/** Stable identity of a search result. */
val ModHit.key: String get() = "$source-$id"

/** The project on every source it was found on, this hit's own first. */
val ModHit.refs: List<ModRef> get() = listOf(ModRef(source, id, pageUrl)) + also

@OptIn(FlowPreview::class)
class ModsViewModel(
    private val repository: SulfurRepository,
    private val scope: CoroutineScope,
) {
    private val _ui = MutableStateFlow(ModsUiState())
    val ui: StateFlow<ModsUiState> = _ui.asStateFlow()
    private var searchJob: Job? = null

    init {
        // Follow the version picked on the Launch page until the user picks one here.
        scope.launch {
            repository.installed.collect { all ->
                val list = all.filterNot { it.alkaline }
                val current = _ui.value.versionId
                if (current == null || list.none { it.id == current }) {
                    val preferred = repository.settings.value.selectedVersion
                    val pick = list.firstOrNull { it.id == preferred && it.loader != null }
                        ?: list.firstOrNull { it.loader != null }
                        ?: list.firstOrNull()
                    if (pick?.id != current) selectVersion(pick?.id)
                }
            }
        }
        // Popular projects right away, then search as the user types.
        search()
        scope.launch {
            _ui.map { it.query }.distinctUntilChanged().drop(1).debounce(400).collect { search() }
        }
    }

    fun setQuery(query: String) = _ui.update { it.copy(query = query) }

    fun setSource(source: SourceChoice) {
        _ui.update { it.copy(source = source) }
        search()
    }

    fun setKind(kind: ProjectKind) {
        _ui.update { it.copy(kind = kind, installedMods = emptyList()) }
        refreshInstalledMods()
        search()
    }

    fun setSort(sort: ModSort) {
        _ui.update { it.copy(sort = sort) }
        search()
    }

    fun selectVersion(versionId: String?) {
        _ui.update { it.copy(versionId = versionId, worlds = emptyList(), world = null) }
        refreshWorlds()
        refreshInstalledMods()
        search()
    }

    fun selectWorld(world: String) {
        _ui.update { it.copy(world = world) }
        refreshInstalledMods()
    }

    fun search(append: Boolean = false) {
        val state = _ui.value
        searchJob?.cancel()
        _ui.update { it.copy(loading = true, error = null, hits = if (append) it.hits else emptyList()) }
        searchJob = scope.launch {
            try {
                val offset = if (append) state.fetched else 0u
                val page = repository.searchMods(
                    ModQuery(
                        sources = state.source.sources,
                        kind = state.kind,
                        text = state.query,
                        versionId = state.versionId,
                        sort = state.sort,
                        offset = offset,
                    )
                )
                _ui.update {
                    it.copy(
                        loading = false,
                        // Later pages may hold a project already shown from the other source.
                        hits = if (append) mergeModHits(it.hits, page.hits) else page.hits,
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

    fun install(hit: ModHit) {
        val state = _ui.value
        val versionId = state.versionId ?: return
        _ui.update { it.copy(installing = it.installing + hit.key) }
        scope.launch {
            runCatching { repository.installMod(hit.refs, state.kind, versionId, state.world) }
                .onSuccess { repository.notify(UiMessage(Res.string.msg_mod_installed, hit.title)) }
                .onFailure(repository::notifyError)
            _ui.update { it.copy(installing = it.installing - hit.key) }
            refreshInstalledMods()
        }
    }

    /** Adds [hit] to the download list, or takes it out if it is there. */
    fun toggleListed(hit: ModHit) {
        _ui.update { state ->
            if (!state.kind.listable) return@update state
            val entry = ListedProject(hit, state.kind)
            val list = if (state.isListed(hit)) state.downloadList.filterNot { it.hit.key == hit.key && it.kind == state.kind }
            else state.downloadList + entry
            state.copy(downloadList = list)
        }
    }

    fun removeListed(entry: ListedProject) = _ui.update { it.copy(downloadList = it.downloadList - entry) }

    fun clearList() = _ui.update { it.copy(downloadList = emptyList()) }

    /**
     * Installs everything in the download list into the selected version, a
     * few at a time. Projects that fail stay in the list so they can be retried.
     */
    fun installList() {
        val state = _ui.value
        val versionId = state.versionId ?: return
        val entries = state.downloadList.filter { it.hit.key !in state.installing }
        if (entries.isEmpty()) return
        _ui.update { it.copy(installing = it.installing + entries.map { e -> e.hit.key }) }
        scope.launch {
            val gate = Semaphore(LIST_PARALLEL)
            val results = entries.map { entry ->
                async {
                    gate.withPermit {
                        val result = runCatching { repository.installMod(entry.hit.refs, entry.kind, versionId, null) }
                        result.onFailure(repository::notifyError)
                        _ui.update { it.copy(installing = it.installing - entry.hit.key) }
                        entry to result.isSuccess
                    }
                }
            }.awaitAll()
            val done = results.filter { it.second }.map { it.first }
            _ui.update { it.copy(downloadList = it.downloadList - done.toSet()) }
            if (done.isNotEmpty()) {
                repository.notify(UiMessage(Res.string.msg_list_installed, done.size.toString(), versionId))
            }
            refreshInstalledMods()
        }
    }

    fun setEnabled(mod: InstalledMod, enabled: Boolean) =
        modify { state, id -> repository.setModEnabled(id, state.kind, state.world, mod.fileName, enabled) }

    fun remove(mod: InstalledMod) = modify { state, id -> repository.removeMod(id, state.kind, state.world, mod.fileName) }

    private fun modify(action: suspend (ModsUiState, String) -> Unit) {
        val state = _ui.value
        val versionId = state.versionId ?: return
        scope.launch {
            runCatching { action(state, versionId) }.onFailure(repository::notifyError)
            refreshInstalledMods()
        }
    }

    fun refreshInstalledMods() {
        val state = _ui.value
        val versionId = state.versionId
        scope.launch {
            val mods = when {
                versionId == null -> emptyList()
                state.kind == ProjectKind.DATA_PACK && state.world == null -> emptyList()
                else -> repository.installedMods(versionId, state.kind, state.world)
            }
            _ui.update {
                if (it.versionId == versionId && it.kind == state.kind && it.world == state.world) it.copy(installedMods = mods) else it
            }
        }
    }

    private fun refreshWorlds() {
        val versionId = _ui.value.versionId ?: return
        scope.launch {
            val worlds = repository.worlds(versionId)
            _ui.update {
                if (it.versionId != versionId) it
                else it.copy(worlds = worlds, world = it.world?.takeIf { w -> w in worlds } ?: worlds.firstOrNull())
            }
            refreshInstalledMods()
        }
    }

    private companion object {
        /** Results per source and page; matches the core. */
        const val PAGE = 20u

        /** Projects of a download list installed at the same time. */
        const val LIST_PARALLEL = 3
    }
}
