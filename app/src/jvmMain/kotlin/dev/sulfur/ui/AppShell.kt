package dev.sulfur.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationItemIconPosition
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.WideNavigationRail
import androidx.compose.material3.WideNavigationRailDefaults
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.material3.WideNavigationRailValue
import androidx.compose.material3.rememberWideNavigationRailState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import dev.sulfur.AppContainer
import dev.sulfur.feature.download.DownloadScreen
import dev.sulfur.feature.launch.LaunchScreen
import dev.sulfur.feature.misc.MiscScreen
import dev.sulfur.feature.settings.SettingsScreen
import dev.sulfur.data.LocalImages
import dev.sulfur.data.isRunning
import dev.sulfur.data.rememberBackgroundImage
import dev.sulfur.ffi.NavigationPosition
import dev.sulfur.resources.sulfur_logo
import dev.sulfur.resources.Res
import dev.sulfur.resources.ic_download
import dev.sulfur.resources.ic_download_filled
import dev.sulfur.resources.nav_collapse
import dev.sulfur.resources.nav_download
import dev.sulfur.resources.nav_expand
import dev.sulfur.resources.nav_launch
import dev.sulfur.resources.nav_misc
import dev.sulfur.resources.nav_settings
import dev.sulfur.resources.ic_menu
import dev.sulfur.resources.ic_menu_open
import dev.sulfur.resources.ic_rocket_launch
import dev.sulfur.resources.ic_rocket_launch_filled
import dev.sulfur.resources.ic_settings
import dev.sulfur.resources.ic_settings_filled
import dev.sulfur.resources.ic_widgets
import dev.sulfur.resources.ic_widgets_filled
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.getString
import org.jetbrains.compose.resources.stringResource
import org.jetbrains.compose.resources.painterResource

enum class Destination(
    val label: StringResource,
    val icon: DrawableResource,
    val selectedIcon: DrawableResource,
) {
    Launch(Res.string.nav_launch, Res.drawable.ic_rocket_launch, Res.drawable.ic_rocket_launch_filled),
    Download(Res.string.nav_download, Res.drawable.ic_download, Res.drawable.ic_download_filled),
    Settings(Res.string.nav_settings, Res.drawable.ic_settings, Res.drawable.ic_settings_filled),
    Misc(Res.string.nav_misc, Res.drawable.ic_widgets, Res.drawable.ic_widgets_filled),
}

@Composable
fun AppShell(app: AppContainer) {
    // Kept by the container so it survives the recomposition a language change causes.
    var destination by app.destination
    val snackbar = remember { SnackbarHostState() }
    val settings by app.repository.settings.collectAsState()
    val installs by app.repository.installs.collectAsState()
    val running = installs.count { it.isRunning }
    val background by rememberBackgroundImage(settings.backgroundImage, settings.backgroundBlur)

    LaunchedEffect(Unit) {
        app.repository.messages.collect { snackbar.showSnackbar(getString(it.text, *it.args)) }
    }

    CompositionLocalProvider(LocalImages provides app.images) {
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceContainer)) {
            background?.let { picture ->
                Image(
                    picture,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    alpha = settings.backgroundOpacity.coerceIn(0f, 1f),
                    modifier = Modifier.fillMaxSize(),
                )
            }
            // Over a picture the page turns slightly translucent so the picture shows at its edges.
            val pageColor = MaterialTheme.colorScheme.surface.let { if (background != null) it.copy(alpha = 0.86f) else it }
            val page: @Composable (Modifier) -> Unit = { modifier ->
                Page(app, destination, pageColor, onNavigate = { destination = it }, modifier = modifier)
            }
            when (settings.navigation) {
                NavigationPosition.TOP -> Column(Modifier.fillMaxSize()) {
                    TopBar(destination, running, onSelect = { destination = it })
                    page(Modifier.weight(1f).fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 8.dp))
                }
                NavigationPosition.SIDE -> Row(Modifier.fillMaxSize()) {
                    SideRail(destination, running, onSelect = { destination = it })
                    Column(Modifier.weight(1f).fillMaxHeight()) {
                        TitleStrip()
                        page(Modifier.weight(1f).fillMaxWidth().padding(end = 8.dp, bottom = 8.dp))
                    }
                }
            }
            SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp))
        }
    }
}

/** The Sulfur mark with its name, at the start of the navigation. */
@Composable
private fun Brand(showName: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Image(painterResource(Res.drawable.sulfur_logo), contentDescription = "Sulfur", modifier = Modifier.size(32.dp))
        if (showName) {
            Spacer(Modifier.width(10.dp))
            Text("Sulfur", style = MaterialTheme.typography.titleLargeEmphasized, color = MaterialTheme.colorScheme.onSurface)
        }
    }
}

/** Page entries in a bar above the page (the default), which is also the window's title bar. */
@Composable
private fun TopBar(destination: Destination, running: Int, onSelect: (Destination) -> Unit) {
    val chrome = LocalWindowChrome.current
    Row(
        Modifier.fillMaxWidth().height(72.dp).windowTitleBar(chrome)
            .padding(start = if (chrome?.nativeButtons == true) TrafficLightsWidth else 20.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Brand(showName = true)
        // The bar's own items, centered between the brand and the window buttons. The
        // ShortNavigationBar container is meant for the full window width.
        Row(
            Modifier.weight(1f).padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Destination.entries.forEach { item ->
                val selected = item == destination
                ShortNavigationBarItem(
                    selected = selected,
                    onClick = { onSelect(item) },
                    icon = { DestinationIcon(item, selected, running) },
                    label = { Text(stringResource(item.label)) },
                    iconPosition = NavigationItemIconPosition.Start,
                )
            }
        }
        WindowButtons()
    }
}

/** Page entries in a navigation rail on the left. */
@Composable
private fun SideRail(destination: Destination, running: Int, onSelect: (Destination) -> Unit) {
    val railState = rememberWideNavigationRailState()
    val expanded = railState.targetValue == WideNavigationRailValue.Expanded
    val scope = rememberCoroutineScope()
    WideNavigationRail(
        state = railState,
        colors = WideNavigationRailDefaults.colors(containerColor = Color.Transparent),
        header = {
            val chrome = LocalWindowChrome.current
            // On macOS the traffic lights sit above the brand.
            val top = if (chrome?.nativeButtons == true) TrafficLightsHeight else 0.dp
            Column(Modifier.windowTitleBar(chrome).padding(start = 20.dp, top = top)) {
                Box(Modifier.padding(start = 8.dp, bottom = 8.dp)) { Brand(showName = expanded) }
                IconButton(onClick = { scope.launch { railState.toggle() } }) {
                    Icon(
                        painterResource(if (expanded) Res.drawable.ic_menu_open else Res.drawable.ic_menu),
                        contentDescription = stringResource(if (expanded) Res.string.nav_collapse else Res.string.nav_expand),
                    )
                }
            }
        },
    ) {
        Destination.entries.forEach { item ->
            val selected = item == destination
            WideNavigationRailItem(
                railExpanded = expanded,
                selected = selected,
                onClick = { onSelect(item) },
                icon = { DestinationIcon(item, selected, running) },
                label = { Text(stringResource(item.label)) },
            )
        }
    }
}

@Composable
private fun DestinationIcon(item: Destination, selected: Boolean, running: Int) {
    BadgedBox(badge = {
        if (item == Destination.Download && running > 0) Badge { Text("$running") }
    }) {
        Icon(painterResource(if (selected) item.selectedIcon else item.icon), contentDescription = null)
    }
}

/** The current page on a rounded surface, with transitions between pages. */
@Composable
private fun Page(
    app: AppContainer,
    destination: Destination,
    color: Color,
    onNavigate: (Destination) -> Unit,
    modifier: Modifier,
) {
    Surface(modifier = modifier, shape = MaterialTheme.shapes.extraLarge, color = color) {
        val motion = MaterialTheme.motionScheme
        AnimatedContent(
            targetState = destination,
            transitionSpec = {
                // Pages further along the navigation enter from below, earlier ones from above.
                val direction = if (targetState.ordinal > initialState.ordinal) 1 else -1
                ContentTransform(
                    targetContentEnter = fadeIn(motion.defaultEffectsSpec()) +
                        slideInVertically(motion.defaultSpatialSpec()) { direction * it / 8 } +
                        scaleIn(motion.defaultSpatialSpec(), initialScale = 0.97f),
                    initialContentExit = fadeOut(motion.fastEffectsSpec()) +
                        slideOutVertically(motion.fastSpatialSpec()) { -direction * it / 16 },
                    sizeTransform = SizeTransform(clip = false),
                )
            },
            label = "page",
        ) { page ->
            when (page) {
                Destination.Launch -> LaunchScreen(app.launch, app.repository, onBrowseVersions = {
                    onNavigate(Destination.Download)
                })
                Destination.Download -> DownloadScreen(app.download, app.mods, app.modpacks, app.repository)
                Destination.Settings -> SettingsScreen(app.repository, app.misc)
                Destination.Misc -> MiscScreen(app.misc, app.repository)
            }
        }
    }
}
