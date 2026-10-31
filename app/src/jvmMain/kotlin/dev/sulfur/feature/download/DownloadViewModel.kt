package dev.sulfur.feature.download

import dev.sulfur.data.SulfurRepository
import dev.sulfur.ffi.ArchivedVersion
import dev.sulfur.ffi.Banner
import dev.sulfur.ffi.CoreException
import dev.sulfur.ffi.InstallRequest
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.LoaderVersion
import dev.sulfur.ffi.RemoteVersion
import dev.sulfur.ffi.VersionGroup
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** The parts of the Download page. */
enum class DownloadSection { Game, Mods, Modpacks, Archive }

/** Omniarchive's jars, loaded when the Archive section is first shown. */
data class ArchiveState(
    val loading: Boolean = false,
    val error: String? = null,
    val versions: List<ArchivedVersion> = emptyList(),
    /** Archive folder shown; null for all. */
    val category: String? = null,
    val query: String = "",
) {
    val shown: List<ArchivedVersion>
        get() = versions.filter {
            (category == null || it.category == category) && it.fileName.contains(query.trim(), ignoreCase = true)
        }
}

data class DownloadUiState(
    val section: DownloadSection = DownloadSection.Game,
    val loading: Boolean = false,
    val error: String? = null,
    val fromCache: Boolean = false,
    val versions: List<RemoteVersion> = emptyList(),
    /** Major updates, newest first. */
    val groups: List<VersionGroup> = emptyList(),
    /** Artwork by group key; groups without art are filled with color. */
    val banners: Map<String, Banner> = emptyMap(),
    val query: String = "",
    /** Key of the group shown in the floating card. */
    val openGroup: String? = null,
    /** Version whose install options dialog is open. */
    val installTarget: String? = null,
    val archive: ArchiveState = ArchiveState(),
)

/**
 * Versions matching every word of [query], by id or by [labels] (the names of
 * a version's update and channel, in any language). Exact ids come first, then
 * ids starting with the query, then the rest in list order (newest first).
 */
internal fun searchVersions(query: String, versions: List<RemoteVersion>, labels: (RemoteVersion) -> String): List<RemoteVersion> {
    val q = query.trim().lowercase()
    if (q.isEmpty()) return emptyList()
    val words = q.split(Regex("\\s+"))
    return versions
        .filter { v -> "${v.id} ${labels(v)}".lowercase().let { text -> words.all { it in text } } }
        .sortedBy { v ->
            val id = v.id.lowercase()
            when {
                id == q -> 0
                id.startsWith(q) -> 1
                id.contains(q) -> 2
                else -> 3
            }
        }
}

class DownloadViewModel(
    private val repository: SulfurRepository,
    private val scope: CoroutineScope,
) {
    private val _ui = MutableStateFlow(DownloadUiState())
    val ui: StateFlow<DownloadUiState> = _ui.asStateFlow()

    init {
        refresh(force = false)
    }

    fun refresh(force: Boolean = true) {
        if (_ui.value.loading) return
        _ui.update { it.copy(loading = true, error = null) }
        scope.launch {
            try {
                val list = repository.versionList(force)
                _ui.update {
                    it.copy(
                        loading = false,
                        fromCache = list.fromCache,
                        versions = list.versions,
                        groups = list.groups,
                    )
                }
            } catch (e: CoreException) {
                _ui.update { it.copy(loading = false, error = e.message) }
                return@launch
            }
            // Artwork is decoration: failures leave the color fill.
            runCatching { repository.banners(force) }
                .onSuccess { banners -> _ui.update { it.copy(banners = banners.associateBy { b -> b.key }) } }
        }
    }

    fun setQuery(query: String) = _ui.update { it.copy(query = query) }

    fun showSection(section: DownloadSection) {
        _ui.update { it.copy(section = section, openGroup = null) }
        if (section == DownloadSection.Archive && _ui.value.archive.versions.isEmpty()) loadArchive(refresh = false)
    }

    fun loadArchive(refresh: Boolean = true) {
        if (_ui.value.archive.loading) return
        _ui.update { it.copy(archive = it.archive.copy(loading = true, error = null)) }
        scope.launch {
            try {
                val versions = repository.archivedVersions(refresh)
                _ui.update { it.copy(archive = it.archive.copy(loading = false, versions = versions)) }
            } catch (e: CoreException) {
                _ui.update { it.copy(archive = it.archive.copy(loading = false, error = e.message)) }
            }
        }
    }

    fun showArchiveCategory(category: String?) = _ui.update { it.copy(archive = it.archive.copy(category = category)) }

    fun setArchiveQuery(query: String) = _ui.update { it.copy(archive = it.archive.copy(query = query)) }

    fun installArchived(id: String) = repository.installArchived(id)

    fun openGroup(key: String?) = _ui.update { it.copy(openGroup = key) }

    fun install(versionId: String) = repository.install(versionId)

    fun showInstallOptions(versionId: String?) = _ui.update { it.copy(installTarget = versionId) }

    fun install(request: InstallRequest) {
        _ui.update { it.copy(installTarget = null) }
        repository.install(request)
    }

    suspend fun loaderVersions(kind: LoaderKind, gameVersion: String): List<LoaderVersion> =
        repository.loaderVersions(kind, gameVersion)

    fun cancel(versionId: String) = repository.cancelInstall(versionId)

    fun dismiss(versionId: String) = repository.dismissInstall(versionId)
}
