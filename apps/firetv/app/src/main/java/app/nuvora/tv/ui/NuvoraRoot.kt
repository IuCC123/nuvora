package app.nuvora.tv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import app.nuvora.tv.data.SavedServer
import app.nuvora.tv.data.SessionStore
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.LocalSessionEnded
import app.nuvora.tv.ui.connect.ConnectScreen
import app.nuvora.tv.ui.connect.SignInScreen
import app.nuvora.tv.ui.details.MovieScreen
import app.nuvora.tv.ui.details.ShowScreen
import app.nuvora.tv.ui.home.HomeScreen
import app.nuvora.tv.ui.library.LibraryScreen
import app.nuvora.tv.ui.player.PlayerScreen
import app.nuvora.tv.ui.search.SearchScreen
import app.nuvora.tv.ui.settings.SettingsScreen
import app.nuvora.tv.ui.theme.Nv
import kotlinx.serialization.Serializable

@Serializable object ConnectRoute
@Serializable data class SignInRoute(val url: String, val name: String)
@Serializable object HomeRoute
@Serializable data class LibraryRoute(val id: Long, val name: String)
@Serializable data class MovieRoute(val id: Long)
@Serializable data class ShowRoute(val id: Long)
@Serializable object SearchRoute
@Serializable object SettingsRoute
/**
 * The player on [itemId]. [fileId] 0 picks the version (the one being
 * resumed, else the best); [startMs] -1 resumes where the user stopped.
 */
@Serializable data class PlayerRoute(val itemId: Long, val fileId: Long = 0, val startMs: Long = RESUME)

const val RESUME = -1L

/** Where a poster or result goes: a movie or show page, or (Continue Watching) straight back into an episode. */
fun NavHostController.open(kind: String, id: Long) {
    when (kind) {
        "movie" -> navigate(MovieRoute(id))
        "show" -> navigate(ShowRoute(id))
        "episode" -> navigate(PlayerRoute(id))
    }
}

/**
 * The app: Connect and Sign in until there is a session, then everything
 * else with that session's API in scope. A session the server ends (401)
 * sends the person back to Sign in for the same server.
 */
@Composable
fun NuvoraRoot(store: SessionStore) {
    val session by store.session.collectAsState()
    val nav = rememberNavController()
    val api = remember(session) { store.api() }

    LaunchedEffect(session == null) {
        val onAuthScreen = nav.currentDestination?.let { it.hasRoute(ConnectRoute::class) || it.hasRoute(SignInRoute::class) } ?: true
        if (session == null && !onAuthScreen) {
            val last = store.servers().firstOrNull()
            nav.navigate(if (last != null) SignInRoute(last.url, last.name) else ConnectRoute) {
                popUpTo(0) { inclusive = true }
            }
        }
    }

    Box(Modifier.fillMaxSize().background(Nv.canvas)) {
        CompositionLocalProvider(LocalSessionEnded provides { store.signOut() }) {
            NavHost(nav, startDestination = if (session == null) ConnectRoute else HomeRoute) {
                composable<ConnectRoute> {
                    ConnectScreen(store, onFound = { server -> nav.navigate(SignInRoute(server.url, server.name)) })
                }
                composable<SignInRoute> { entry ->
                    val route = entry.toRoute<SignInRoute>()
                    SignInScreen(
                        store = store,
                        server = SavedServer(route.url, route.name),
                        onSignedIn = { nav.navigate(HomeRoute) { popUpTo(0) { inclusive = true } } },
                        onChangeServer = { nav.navigate(ConnectRoute) { popUpTo(0) { inclusive = true } } },
                    )
                }
                composable<HomeRoute> { Signed(api) { HomeScreen(nav) } }
                composable<LibraryRoute> { entry ->
                    val route = entry.toRoute<LibraryRoute>()
                    Signed(api) { LibraryScreen(nav, route.id, route.name) }
                }
                composable<MovieRoute> { entry -> Signed(api) { MovieScreen(nav, entry.toRoute<MovieRoute>().id) } }
                composable<ShowRoute> { entry -> Signed(api) { ShowScreen(nav, entry.toRoute<ShowRoute>().id) } }
                composable<SearchRoute> { Signed(api) { SearchScreen(nav) } }
                composable<SettingsRoute> {
                    Signed(api) {
                        SettingsScreen(
                            nav,
                            session = session,
                            onSignOut = { store.signOut() },
                            onChangeServer = {
                                nav.navigate(ConnectRoute) { popUpTo(0) { inclusive = true } }
                                store.signOut()
                            },
                        )
                    }
                }
                composable<PlayerRoute> { entry ->
                    val route = entry.toRoute<PlayerRoute>()
                    Signed(api) { PlayerScreen(nav, route.itemId, route.fileId, route.startMs, session?.user) }
                }
            }
        }
    }
}

/** A screen that needs a session; renders nothing in the moment after sign-out. */
@Composable
private fun Signed(api: app.nuvora.tv.data.NuvoraApi?, content: @Composable () -> Unit) {
    if (api == null) return
    CompositionLocalProvider(LocalApi provides api, content = content)
}
