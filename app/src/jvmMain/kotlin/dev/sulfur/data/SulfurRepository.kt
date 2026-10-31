package dev.sulfur.data

import dev.sulfur.ffi.AccountInfo
import dev.sulfur.ffi.AccountKind
import dev.sulfur.ffi.AccountPolicy
import dev.sulfur.ffi.ArchivedVersion
import dev.sulfur.ffi.Banner
import dev.sulfur.ffi.CoreException
import dev.sulfur.ffi.CorePaths
import dev.sulfur.ffi.DesktopWallpaper
import dev.sulfur.ffi.ForeignConfig
import dev.sulfur.ffi.GameSession
import dev.sulfur.ffi.InstallRequest
import dev.sulfur.ffi.InstallSnapshot
import dev.sulfur.ffi.InstallStage
import dev.sulfur.ffi.InstallTask
import dev.sulfur.ffi.InstalledMod
import dev.sulfur.ffi.InstalledVersion
import dev.sulfur.ffi.Isolation
import dev.sulfur.ffi.JavaRuntime
import dev.sulfur.ffi.LoaderKind
import dev.sulfur.ffi.LoaderVersion
import dev.sulfur.ffi.ModPage
import dev.sulfur.ffi.ModQuery
import dev.sulfur.ffi.ModRef
import dev.sulfur.ffi.ModSort
import dev.sulfur.ffi.ModpackInfo
import dev.sulfur.ffi.ModpackVersion
import dev.sulfur.ffi.NewsEntry
import dev.sulfur.ffi.ProjectKind
import dev.sulfur.ffi.Settings
import dev.sulfur.ffi.SignInTask
import dev.sulfur.ffi.SulfurCore
import dev.sulfur.ffi.VersionList
import dev.sulfur.ffi.VersionSettings
import dev.sulfur.resources.Res
import dev.sulfur.resources.msg_error
import dev.sulfur.resources.msg_install_failed
import dev.sulfur.resources.msg_install_ready
import dev.sulfur.resources.msg_java_ready
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jetbrains.compose.resources.StringResource

/**
 * A user-facing message, resolved to text in the UI so it follows the app's language.
 * Core errors arrive as untranslated text and are passed as arguments.
 */
class UiMessage(val text: StringResource, vararg val args: Any)

/**
 * The single gateway from the UI to the Rust core.
 *
 * Exposes core state as flows, runs blocking FFI calls off the UI thread and
 * polls running installs so screens can animate progress without callbacks.
 */
class SulfurRepository(
    private val core: SulfurCore,
    private val scope: CoroutineScope,
) {
    val coreVersion: String = core.version()
    val paths: CorePaths = core.paths()

    private val _accountPolicy = MutableStateFlow(core.accountPolicy())
    /** Whether offline accounts may be used here: in mainland China, or next to a Microsoft account that owns the game. */
    val accountPolicy: StateFlow<AccountPolicy> = _accountPolicy.asStateFlow()

    private val _settings = MutableStateFlow(core.settings())
    val settings: StateFlow<Settings> = _settings.asStateFlow()

    private val _accounts = MutableStateFlow(core.accounts())
    /** Accounts that can be played with: Microsoft accounts, then offline ones where allowed. */
    val accounts: StateFlow<List<AccountInfo>> = _accounts.asStateFlow()

    /**
     * The account Play uses, by the core's rule: the selected account, else
     * the first offline account, else the first Microsoft account.
     */
    val currentAccount: StateFlow<AccountInfo?> = combine(_settings, _accounts) { s, accounts ->
        accounts.firstOrNull { s.selectedAccount.isNotEmpty() && it.id == s.selectedAccount }
            ?: accounts.firstOrNull { it.kind == AccountKind.OFFLINE }
            ?: accounts.firstOrNull()
    }.stateIn(scope, SharingStarted.Eagerly, null)

    private val _installed = MutableStateFlow<List<InstalledVersion>>(emptyList())
    val installed: StateFlow<List<InstalledVersion>> = _installed.asStateFlow()

    private val _installs = MutableStateFlow<List<InstallSnapshot>>(emptyList())
    /** Installs started this session, newest first, refreshed ~10 times per second. */
    val installs: StateFlow<List<InstallSnapshot>> = _installs.asStateFlow()

    private val _messages = MutableSharedFlow<UiMessage>(extraBufferCapacity = 16)
    /** One-off user-facing messages (shown as snackbars). */
    val messages: SharedFlow<UiMessage> = _messages

    private val tasks = LinkedHashMap<String, InstallTask>()

    init {
        refreshInstalled()
        scope.launch { persistSettings() }
        scope.launch { pollInstalls() }
    }

    fun notify(message: UiMessage) {
        _messages.tryEmit(message)
    }

    /** Reports a failure from the core; its message is shown as is. */
    fun notifyError(error: Throwable) {
        notify(UiMessage(Res.string.msg_error, error.message ?: error.toString()))
    }

    fun refreshInstalled() {
        scope.launch {
            _installed.value = withContext(Dispatchers.IO) { core.installedVersions() }
        }
    }

    /** Updates settings immediately in the UI; they are saved shortly after. */
    fun updateSettings(transform: (Settings) -> Settings) {
        _settings.update(transform)
    }

    fun setVersionIsolation(versionId: String, isolation: Isolation?) {
        scope.launch {
            runCatching { withContext(Dispatchers.IO) { core.setVersionIsolation(versionId, isolation) } }
                .onFailure(::notifyError)
            refreshInstalled()
        }
    }

    /** One version's overrides of the game and Java settings. */
    fun versionSettings(versionId: String): VersionSettings = core.versionSettings(versionId)

    fun setVersionSettings(versionId: String, overrides: VersionSettings) {
        scope.launch {
            runCatching { withContext(Dispatchers.IO) { core.setVersionSettings(versionId, overrides) } }
                .onFailure(::notifyError)
        }
    }

    /** The desktop's wallpaper (a picture the UI can decode, or a color), or null if unknown. */
    suspend fun desktopWallpaper(): DesktopWallpaper? = withContext(Dispatchers.IO) { core.desktopWallpaper() }

    suspend fun launcherNews(refresh: Boolean): List<NewsEntry> = core.launcherNews(refresh)

    /** A Markdown document from a file or an https address, for the Launch page. */
    suspend fun readDocument(location: String): String = core.readDocument(location)

    suspend fun versionList(refresh: Boolean): VersionList = core.versionList(refresh)

    suspend fun banners(refresh: Boolean): List<Banner> = core.banners(refresh)

    /** Local path of a downloaded image (cached on disk by the core). */
    suspend fun cachedImage(url: String): String = core.cachedImage(url)

    suspend fun loaderVersions(kind: LoaderKind, gameVersion: String): List<LoaderVersion> =
        core.loaderVersions(kind, gameVersion)

    fun install(versionId: String) = install(InstallRequest(versionId, null, null, null, false, null))

    fun install(request: InstallRequest) = start { core.installVersion(request) }

    /** Starts a core install and follows its progress; refusals are shown as messages. */
    private fun start(begin: () -> InstallTask) {
        try {
            track(begin())
        } catch (e: CoreException) {
            notifyError(e)
        }
    }

    private fun track(task: InstallTask) {
        val id = task.versionId()
        synchronized(tasks) {
            tasks.remove(id)
            tasks[id] = task
        }
    }

    /** Client jars preserved by Omniarchive. */
    suspend fun archivedVersions(refresh: Boolean): List<ArchivedVersion> = core.archivedVersions(refresh)

    fun installArchived(id: String) = start { core.installArchived(id) }

    suspend fun searchModpacks(text: String, sort: ModSort, offset: UInt): ModPage = core.searchModpacks(text, sort, offset)

    suspend fun modpackVersions(projectId: String): List<ModpackVersion> = core.modpackVersions(projectId)

    /** What a `.mrpack` file holds, read without installing it. */
    suspend fun readModpackFile(path: String): ModpackInfo = withContext(Dispatchers.IO) { core.readModpackFile(path) }

    /** Installs a modpack as a new version named [name]; progress shows with the installs. */
    fun installModpack(version: ModpackVersion, name: String) = start { core.installModpack(version, name) }

    fun installModpackFile(path: String, name: String) = start { core.installModpackFile(path, name) }

    /** Other launchers' settings in [folders] and in their user-wide locations. */
    suspend fun findLauncherConfigs(folders: List<String>): List<ForeignConfig> =
        withContext(Dispatchers.IO) { core.findLauncherConfigs(folders) }

    /** Copies another launcher's settings into Sulfur's. */
    suspend fun importLauncherConfig(config: ForeignConfig, useGameDir: Boolean) {
        val imported = withContext(Dispatchers.IO) { core.importLauncherConfig(config, useGameDir) }
        _settings.value = imported
        refreshInstalled()
    }

    fun cancelInstall(versionId: String) {
        synchronized(tasks) { tasks[versionId] }?.cancel()
    }

    fun dismissInstall(versionId: String) {
        synchronized(tasks) { tasks.remove(versionId) }?.close()
    }

    suspend fun launch(versionId: String): GameSession {
        // The account and its name may have changed within the save delay.
        withContext(Dispatchers.IO) { core.updateSettings(_settings.value) }
        return core.launch(versionId)
    }

    /** Plays with [account] from now on. */
    fun selectAccount(account: AccountInfo) = updateSettings { it.copy(selectedAccount = account.id) }

    /** Adds an offline account and plays with it; fails with a message for the user. */
    suspend fun addOfflineAccount(name: String): Result<AccountInfo> = runCatching {
        withContext(Dispatchers.IO) { core.addOfflineAccount(name) }
    }.onSuccess {
        accountsChanged()
        selectAccount(it)
    }

    suspend fun renameOfflineAccount(account: AccountInfo, name: String): Result<AccountInfo> = runCatching {
        withContext(Dispatchers.IO) { core.renameOfflineAccount(account.id, name) }
    }.onSuccess {
        val wasCurrent = currentAccount.value?.id == account.id
        accountsChanged()
        if (wasCurrent) selectAccount(it)
    }

    /** Starts a Microsoft sign-in; poll the task, then call [accountsChanged]. */
    fun signInMicrosoft(): SignInTask = core.signInMicrosoft()

    fun removeAccount(account: AccountInfo) {
        scope.launch {
            runCatching { withContext(Dispatchers.IO) { core.removeAccount(account.id) } }.onFailure(::notifyError)
            accountsChanged()
        }
    }

    /** Reloads accounts and the offline rule after a sign-in or removal. */
    fun accountsChanged() {
        _accounts.value = core.accounts()
        _accountPolicy.value = core.accountPolicy()
    }

    suspend fun detectJava(): List<JavaRuntime> = core.detectJava()

    /** Java major versions Sulfur can download. */
    val downloadableJava: List<UInt> = core.downloadableJava()

    /** Downloads a Java runtime; progress shows with the installs. */
    fun installJava(major: UInt) = track(core.installJava(major))

    suspend fun searchMods(query: ModQuery): ModPage = core.searchMods(query)

    /** Installs a project (and a mod's dependencies); returns the new file names. */
    suspend fun installMod(project: List<ModRef>, kind: ProjectKind, versionId: String, world: String?): List<String> =
        core.installMod(project, kind, versionId, world)

    suspend fun installedMods(versionId: String, kind: ProjectKind, world: String?): List<InstalledMod> =
        withContext(Dispatchers.IO) { core.installedMods(versionId, kind, world) }

    suspend fun worlds(versionId: String): List<String> = withContext(Dispatchers.IO) { core.worlds(versionId) }

    suspend fun setModEnabled(versionId: String, kind: ProjectKind, world: String?, fileName: String, enabled: Boolean) =
        withContext(Dispatchers.IO) { core.setModEnabled(versionId, kind, world, fileName, enabled) }

    suspend fun removeMod(versionId: String, kind: ProjectKind, world: String?, fileName: String) =
        withContext(Dispatchers.IO) { core.removeMod(versionId, kind, world, fileName) }

    @OptIn(FlowPreview::class)
    private suspend fun persistSettings() {
        var saved = _settings.value
        settings.drop(1).debounce(400).collect { value ->
            // Reading every installed version is only needed when where they live changed.
            val moved = value.gameDir != saved.gameDir || value.isolation != saved.isolation
            saved = value
            runCatching { withContext(Dispatchers.IO) { core.updateSettings(value) } }
                .onSuccess { if (moved) refreshInstalled() }
                .onFailure(::notifyError)
        }
    }

    private suspend fun pollInstalls() {
        val finished = HashSet<String>()
        while (scope.isActive) {
            val snapshots = synchronized(tasks) { tasks.values.toList() }.map { it.snapshot() }.reversed()
            _installs.value = snapshots
            for (s in snapshots) {
                val done = s.stage == InstallStage.DONE || s.stage == InstallStage.FAILED || s.stage == InstallStage.CANCELLED
                if (done && finished.add(s.versionId)) {
                    when (s.stage) {
                        InstallStage.DONE -> notify(
                            UiMessage(if (s.isJava) Res.string.msg_java_ready else Res.string.msg_install_ready, s.versionId)
                        )
                        InstallStage.FAILED -> notify(UiMessage(Res.string.msg_install_failed, s.versionId, s.error.orEmpty()))
                        else -> {}
                    }
                    refreshInstalled()
                } else if (!done) {
                    finished.remove(s.versionId)
                }
            }
            // Finished installs stay listed until dismissed; they need no fast polling.
            delay(if (snapshots.any { it.isRunning }) 100 else 500)
        }
    }
}

/** Whether this is a Java download rather than a game install. */
val InstallSnapshot.isJava: Boolean get() = versionId.startsWith("Java ")

/** Whether an install is still working. */
val InstallSnapshot.isRunning: Boolean
    get() = stage == InstallStage.PREPARING || stage == InstallStage.DOWNLOADING ||
        stage == InstallStage.EXTRACTING || stage == InstallStage.LOADER
