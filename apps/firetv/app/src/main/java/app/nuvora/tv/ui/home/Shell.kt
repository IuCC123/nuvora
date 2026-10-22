package app.nuvora.tv.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.runtime.remember
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigation.NavHostController
import androidx.tv.material3.DrawerValue
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.NavigationDrawer
import androidx.tv.material3.NavigationDrawerItem
import androidx.tv.material3.NavigationDrawerItemDefaults
import androidx.tv.material3.NavigationDrawerScope
import androidx.tv.material3.Text
import app.nuvora.tv.data.LibrarySummary
import app.nuvora.tv.ui.HomeRoute
import app.nuvora.tv.ui.LibraryRoute
import app.nuvora.tv.ui.SearchRoute
import app.nuvora.tv.ui.SettingsRoute
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.theme.Nv

/** Which part of the app is showing, for the drawer's highlight. */
sealed interface Section {
    data object Home : Section
    data object Search : Section
    data class Library(val id: Long) : Section
    data object Settings : Section
}

/**
 * The side navigation every main screen sits in: icons when collapsed,
 * labels when focus moves into it (Left from the content).
 */
@Composable
fun Shell(nav: NavHostController, section: Section, content: @Composable () -> Unit) {
    val (libraries) = rememberLoad { libraries() }
    val list = (libraries as? Load.Ready<List<LibrarySummary>>)?.value.orEmpty()

    fun go(to: Section) {
        if (to == section) return
        val route: Any = when (to) {
            Section.Home -> HomeRoute
            Section.Search -> SearchRoute
            Section.Settings -> SettingsRoute
            is Section.Library -> LibraryRoute(to.id, list.firstOrNull { it.id == to.id }?.name ?: "Library")
        }
        nav.navigate(route) {
            // Sections replace each other above Home, so Back from any of them returns Home.
            popUpTo(HomeRoute) { inclusive = to == Section.Home }
            launchSingleTop = true
        }
    }

    // Entering the drawer lands on the page you are on.
    val current = remember { FocusRequester() }
    NavigationDrawer(
        drawerContent = { value ->
            Column(
                Modifier.fillMaxHeight().background(Nv.canvas).padding(horizontal = 12.dp, vertical = 28.dp).focusRestorer(current),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Row(Modifier.padding(start = 12.dp, bottom = 22.dp).height(32.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(NvIcons.Play, null, tint = Nv.accent, modifier = Modifier.size(22.dp))
                    if (value == DrawerValue.Open) {
                        Spacer(Modifier.width(8.dp))
                        Text("nuvora", fontSize = 24.sp, fontWeight = FontWeight.Bold, color = Nv.textPrimary)
                    }
                }
                Item(section == Section.Search, NvIcons.Search, "Search", current) { go(Section.Search) }
                Item(section == Section.Home, NvIcons.Home, "Home", current) { go(Section.Home) }
                list.forEach { library ->
                    Item(
                        section == Section.Library(library.id),
                        if (library.kind == "shows") NvIcons.Tv else NvIcons.Film,
                        library.name,
                        current,
                    ) { go(Section.Library(library.id)) }
                }
                Spacer(Modifier.weight(1f))
                Item(section == Section.Settings, NvIcons.Settings, "Settings", current) { go(Section.Settings) }
            }
        },
    ) {
        Box(Modifier.fillMaxSize()) { content() }
    }
}

@Composable
private fun NavigationDrawerScope.Item(selected: Boolean, icon: ImageVector, label: String, current: FocusRequester, onClick: () -> Unit) {
    NavigationDrawerItem(
        selected = selected,
        onClick = onClick,
        modifier = if (selected) Modifier.focusRequester(current) else Modifier,
        leadingContent = { Icon(icon, null, modifier = Modifier.size(22.dp)) },
        colors = NavigationDrawerItemDefaults.colors(
            focusedContainerColor = Nv.textPrimary,
            focusedContentColor = Nv.canvas,
            selectedContainerColor = Nv.surfaceRaised,
            selectedContentColor = Nv.accent,
            contentColor = Nv.textSecondary,
        ),
    ) {
        Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleSmall)
    }
}
