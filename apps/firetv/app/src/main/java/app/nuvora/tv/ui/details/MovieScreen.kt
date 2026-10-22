package app.nuvora.tv.ui.details

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
    // The version being resumed, else the first this TV can play.
    val preferred = movie.versions.firstOrNull { it.fileId == movie.watch.fileId && it.playback.playable }
        ?: movie.versions.firstOrNull { it.playback.playable }
        ?: movie.versions.firstOrNull()
    var chosenId by rememberSaveable(movie.id) { mutableLongStateOf(preferred?.fileId ?: 0L) }
    val version = movie.versions.firstOrNull { it.fileId == chosenId } ?: preferred
    var picking by remember { mutableStateOf(false) }
    val resumeAt = movie.watch.positionMs?.takeIf { version != null && movie.watch.fileId == version.fileId && version.playback.playable }
    val focus = rememberInitialFocus(movie.id)
    val meta = listOfNotNull(
        movie.year?.toString(),
        runtimeText(movie.runtimeMs),
        movie.details.rating?.let { "★ %.1f".format(it) },
        version?.summary?.takeIf { it.isNotBlank() },
    )
    Box {
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
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    if (version == null || !version.playback.playable) {
                        NotPlayable(version?.playback?.reasons.orEmpty().ifEmpty { listOfNotNull(version?.playback?.reason ?: "Its files are missing from the server.") })
                    } else if (resumeAt != null) {
                        NvButton("Resume from ${clockText(resumeAt)}", { nav.navigate(PlayerRoute(movie.id, version.fileId, resumeAt)) }, Modifier.initialFocus(focus), icon = NvIcons.Play, primary = true)
                        NvButton("From the start", { nav.navigate(PlayerRoute(movie.id, version.fileId, 0)) }, icon = NvIcons.Restart)
                    } else {
                        NvButton("Play", { nav.navigate(PlayerRoute(movie.id, version.fileId, 0)) }, Modifier.initialFocus(focus), icon = NvIcons.Play, primary = true)
                    }
                    NvButton(
                        if (movie.watch.watched) "Mark unwatched" else "Mark watched",
                        { scope.launch { runCatching { api.setWatched(movie.id, !movie.watch.watched) }; reload() } },
                        icon = NvIcons.Check,
                    )
                    if (movie.versions.size > 1) {
                        NvButton("${movie.versions.size} versions", { picking = true }, icon = NvIcons.Layers)
                    }
                }
            },
            below = {
                Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
                    version?.let { VersionFacts(it) }
                    CastRow(movie.details.cast)
                }
            },
        )
        if (picking) {
            VersionPicker(movie.versions, version?.fileId ?: 0L, onPick = { chosenId = it.fileId; picking = false }, onClose = { picking = false })
        }
    }
}
