package app.nuvora.tv.ui.details

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import app.nuvora.tv.data.MovieDetail
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.common.rememberLoad
import kotlinx.coroutines.launch

@Composable
fun MovieScreen(nav: NavHostController, id: Long) {
    val (item, reload) = rememberLoad(id) { item(id) }
    when (val state = item) {
        Load.Loading -> Loading()
        is Load.Failed -> ErrorState("Can't open this movie", state.error.message ?: "", "Back") { nav.popBackStack() }
        is Load.Ready -> (state.value as? MovieDetail)?.let { Movie(nav, it, reload) }
            ?: ErrorState("This isn't a movie", "It may have been changed on the server.", "Back") { nav.popBackStack() }
    }
}

@Composable
private fun Movie(nav: NavHostController, movie: MovieDetail, reload: () -> Unit) {
    val api = LocalApi.current
    val scope = rememberCoroutineScope()
    val version = movie.versions.firstOrNull { it.playback.playable } ?: movie.versions.firstOrNull()
    val resumeAt = movie.watch.positionMs?.takeIf { movie.watch.fileId != null && movie.versions.any { v -> v.fileId == movie.watch.fileId && v.playback.playable } }
    val focus = rememberInitialFocus(movie.id)
    val meta = listOfNotNull(
        movie.year?.toString(),
        runtimeText(movie.runtimeMs),
        movie.details.rating?.let { "★ %.1f".format(it) },
        version?.summary?.takeIf { it.isNotBlank() },
    )
    DetailLayout(
        backdrop = movie.backdrop,
        poster = movie.poster,
        title = movie.title,
        year = movie.year,
        meta = meta,
        tagline = movie.details.tagline,
        genres = movie.details.genres,
        overview = movie.overview,
        actions = {
            if (version == null || !version.playback.playable) {
                NotPlayable(version?.playback?.reasons.orEmpty().ifEmpty { listOfNotNull(version?.playback?.reason ?: "Its files are missing from the server.") })
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    if (resumeAt != null) {
                        NvButton("Resume from ${clockText(resumeAt)}", { nav.navigate(PlayerRoute(movie.id, movie.watch.fileId ?: 0, resumeAt)) }, Modifier.initialFocus(focus), icon = NvIcons.Play, primary = true)
                        NvButton("From the start", { nav.navigate(PlayerRoute(movie.id, version.fileId, 0)) }, icon = NvIcons.Restart)
                    } else {
                        NvButton("Play", { nav.navigate(PlayerRoute(movie.id, version.fileId, 0)) }, Modifier.initialFocus(focus), icon = NvIcons.Play, primary = true)
                    }
                    NvButton(
                        if (movie.watch.watched) "Mark unwatched" else "Mark watched",
                        { scope.launch { runCatching { api.setWatched(movie.id, !movie.watch.watched) }; reload() } },
                        icon = NvIcons.Check,
                    )
                }
            }
        },
        below = { CastRow(movie.details.cast) },
    )
}
