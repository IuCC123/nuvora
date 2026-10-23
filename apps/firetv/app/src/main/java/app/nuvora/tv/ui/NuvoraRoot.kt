package app.nuvora.tv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import android.view.inputmethod.InputMethodManager
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.lifecycle.compose.LifecycleStartEffect
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import app.nuvora.tv.data.LiveEvent
import app.nuvora.tv.data.SavedServer
import app.nuvora.tv.data.SessionStore
import app.nuvora.tv.data.checkServer
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
import app.nuvora.tv.ui.together.InviteDialog
import app.nuvora.tv.ui.together.TogetherScreen
import app.nuvora.tv.ui.together.followParty
import kotlinx.serialization.Serializable

@Serializable object ConnectRoute
@Serializable data class SignInRoute(val url: String, val name: String, val id: String? = null)
@Serializable object HomeRoute
@Serializable data class LibraryRoute(val id: Long, val name: String)
@Serializable data class MovieRoute(val id: Long)
@Serializable data class ShowRoute(val id: Long)
@Serializable object SearchRoute
@Serializable object SettingsRoute
@Serializable object TogetherRoute
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
    // Per session, not per user record: saving a setting doesn't make a new API (nor reconnect).
    val api = remember(session?.server?.url, session?.token) { store.api() }

    // Each time the app comes to the front: is the server still where we left it?
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle, session?.server?.url) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) { store.checkServer(context) }
    }

    // Watch Together's live connection, while the app is in front and signed in.
    LifecycleStartEffect(api) {
        api?.let(store.party::start)
        onStopOrDispose { store.party.stop() }
    }
    // Someone in the party started something: everyone goes to it.
    LaunchedEffect(store.party) {
        store.party.events.collect { event ->
            if (event !is LiveEvent.Party || !event.started) return@collect
            val playback = event.party.playback ?: return@collect
            if (store.party.playing.value != playback.itemId to playback.fileId) nav.followParty(store.party, event.party)
        }
    }
    val live by store.party.state.collectAsState()
    var answered by rememberSaveable { mutableStateOf(emptyList<String>()) }
    val newInvite = live.invites.lastOrNull { it.key !in answered }

    LaunchedEffect(session == null) {
        val onAuthScreen = nav.currentDestination?.let { it.hasRoute(ConnectRoute::class) || it.hasRoute(SignInRoute::class) } ?: true
        if (session == null && !onAuthScreen) {
            val last = store.servers().firstOrNull()
            nav.navigate(if (last != null) SignInRoute(last.url, last.name, last.id) else ConnectRoute) {
                popUpTo(0) { inclusive = true }
            }
        }
    }

    // Leaving a screen with text fields, end the keyboard's session with the
    // app: Gboard for TV otherwise keeps it and swallows the remote's arrows.
    val view = LocalView.current
    val entry by nav.currentBackStackEntryAsState()
    val typing = entry?.destination?.let { it.hasRoute(ConnectRoute::class) || it.hasRoute(SignInRoute::class) || it.hasRoute(SearchRoute::class) } ?: true
    LaunchedEffect(entry, typing) {
        if (typing) return@LaunchedEffect
        val imm = view.context.getSystemService(InputMethodManager::class.java) ?: return@LaunchedEffect
        imm.hideSoftInputFromWindow(view.windowToken, 0)
        imm.restartInput(view)
    }

    Box(Modifier.fillMaxSize().background(Nv.canvas)) {
        CompositionLocalProvider(LocalSessionEnded provides { store.signOut() }) {
            NavHost(nav, startDestination = if (session == null) ConnectRoute else HomeRoute) {
                composable<ConnectRoute> {
                    ConnectScreen(store, onFound = { server -> nav.navigate(SignInRoute(server.url, server.name, server.id)) })
                }
                composable<SignInRoute> { entry ->
                    val route = entry.toRoute<SignInRoute>()
                    SignInScreen(
                        store = store,
                        server = SavedServer(route.url, route.name, route.id),
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
                composable<TogetherRoute> { Signed(api) { TogetherScreen(nav, session?.user?.id) } }
                composable<PlayerRoute> { entry ->
                    val route = entry.toRoute<PlayerRoute>()
                    Signed(api) { PlayerScreen(nav, route.itemId, route.fileId, route.startMs, session?.user) }
                }
            }
            if (newInvite != null && api != null) {
                Signed(api) { InviteDialog(nav, newInvite) { answered = answered + newInvite.key } }
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
