package app.nuvora.tv.ui.details

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.tv.material3.Border
import androidx.tv.material3.Card
import androidx.tv.material3.CardDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.Episode
import app.nuvora.tv.data.EpisodeDetail
import app.nuvora.tv.data.ShowDetail
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.common.ArtworkImage
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.NvChip
import app.nuvora.tv.ui.common.onFocusGained
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.ProgressBar
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.theme.Nv

@Composable
fun ShowScreen(nav: NavHostController, id: Long) {
    val (item, reload) = rememberLoad(id) { item(id) }
    when (val state = item) {
        Load.Loading -> Loading()
        is Load.Failed -> ErrorState("Can't open this show", state.error.message ?: "", "Back") { nav.popBackStack() }
        is Load.Ready -> (state.value as? ShowDetail)?.let { Show(nav, it, reload) }
            ?: ErrorState("This isn't a show", "It may have been changed on the server.", "Back") { nav.popBackStack() }
    }
}

@Composable
private fun Show(nav: NavHostController, show: ShowDetail, reload: () -> Unit) {
    val api = LocalApi.current
    val scope = rememberCoroutineScope()
    val next = show.next
    var seasonId by rememberSaveable(show.id) {
        mutableLongStateOf(next?.seasonId ?: show.seasons.firstOrNull()?.id ?: 0L)
    }
    // Bumped after marking something watched, so the episodes load again.
    var changes by remember { mutableIntStateOf(0) }
    val focus = rememberInitialFocus(show.id)
    val allWatched = show.episodeCount > 0 && show.watchedCount >= show.episodeCount
    fun mark(id: Long, watched: Boolean) {
        scope.launch {
            runCatching { api.setWatched(id, watched) }
            changes++
            reload()
        }
    }
    val seasons = show.seasons.count { it.index > 0 }
    val meta = listOfNotNull(
        show.yearRange,
        "$seasons season${if (seasons == 1) "" else "s"}",
        "${show.watchedCount} of ${show.episodeCount} watched",
        show.details.rating?.let { "★ %.1f".format(it) },
    )
    DetailLayout(
        backdrop = show.backdrop,
        poster = show.poster,
        title = show.title,
        year = null,
        meta = meta,
        tagline = show.details.tagline,
        genres = show.details.genres,
        overview = show.overview,
        actions = {
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
                if (next != null) {
                    val resume = next.positionMs
                    NvButton(
                        if (resume != null) "Resume ${next.label} from ${clockText(resume)}" else "Play ${next.label}",
                        { nav.navigate(PlayerRoute(next.id)) },
                        Modifier.initialFocus(focus),
                        icon = NvIcons.Play,
                        primary = true,
                    )
                }
                NvButton(
                    if (allWatched) "Mark unwatched" else "Mark watched",
                    { mark(show.id, !allWatched) },
                    if (next == null) Modifier.initialFocus(focus) else Modifier,
                    icon = NvIcons.Check,
                )
                if (next == null) {
                    Text("You've watched every episode.", style = MaterialTheme.typography.titleSmall, color = Nv.textSecondary)
                }
            }
        },
        below = {
            Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
                val season = show.seasons.firstOrNull { it.id == seasonId }
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 56.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier.focusRestorer(),
                ) {
                    if (show.seasons.size > 1) {
                        items(show.seasons, key = { it.id }) { s ->
                            val done = s.episodeCount > 0 && s.watchedCount >= s.episodeCount
                            NvChip(s.title + if (done) "  ✓" else "", s.id == seasonId) { seasonId = s.id }
                        }
                    }
                    if (season != null) {
                        item(key = "mark") {
                            val done = season.episodeCount > 0 && season.watchedCount >= season.episodeCount
                            Row {
                                if (show.seasons.size > 1) Spacer(Modifier.width(14.dp))
                                NvChip(if (done) "Mark ${season.title} unwatched" else "Mark ${season.title} watched", false) { mark(season.id, !done) }
                            }
                        }
                    }
                }
                if (seasonId != 0L) Episodes(nav, seasonId, changes) { episode -> mark(episode.id, !episode.watch.watched) }
                CastRow(show.details.cast)
            }
        },
    )
}

@Composable
private fun Episodes(nav: NavHostController, seasonId: Long, changes: Int, onToggleWatched: (Episode) -> Unit) {
    val (episodes, reload) = rememberLoad(seasonId) { episodes(seasonId) }
    // Reloaded in place, so the row (and focus in it) stays while the new marks load.
    LaunchedEffect(changes) { if (changes > 0) reload() }
    val list = (episodes as? Load.Ready<List<Episode>>)?.value ?: return
    var focused by remember(seasonId) { mutableStateOf<Episode?>(null) }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        // Above the row, so the page shows it whenever an episode has focus.
        EpisodeInfo(list.firstOrNull { it.id == focused?.id })
        LazyRow(
            contentPadding = PaddingValues(horizontal = 56.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(22.dp),
            modifier = Modifier.focusRestorer(),
        ) {
            items(list, key = { it.id }) { episode ->
                EpisodeCard(
                    episode,
                    onClick = { nav.navigate(PlayerRoute(episode.id)) },
                    onLongClick = { onToggleWatched(episode) },
                    modifier = Modifier.onFocusGained { focused = episode },
                )
            }
        }
    }
}

/** The focused episode: its overview, and how to mark it. Fixed height, so the page doesn't jump. */
@Composable
private fun EpisodeInfo(episode: Episode?) {
    val api = LocalApi.current
    var overview by remember(episode?.id) { mutableStateOf<String?>(null) }
    LaunchedEffect(episode?.id) {
        val id = episode?.id ?: return@LaunchedEffect
        delay(350)
        overview = runCatching { (api.item(id) as? EpisodeDetail)?.overview }.getOrNull()
    }
    Column(Modifier.padding(horizontal = 56.dp).height(112.dp).width(900.dp)) {
        if (episode == null) {
            Text("Episodes", style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary)
            return@Column
        }
        Text("${episode.label} · ${episode.title}", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(4.dp))
        Text(
            overview ?: "",
            style = MaterialTheme.typography.bodyMedium,
            color = Nv.textSecondary,
            maxLines = 3,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.weight(1f))
        Text(
            if (episode.watch.watched) "Watched · hold OK to mark unwatched" else "Hold OK to mark watched",
            style = MaterialTheme.typography.labelMedium,
            color = Nv.textMuted,
        )
    }
}

private val thumbShape = RoundedCornerShape(8.dp)

@Composable
private fun EpisodeCard(episode: Episode, onClick: () -> Unit, onLongClick: () -> Unit, modifier: Modifier = Modifier) {
    Column(Modifier.width(300.dp)) {
        Card(
            onClick = onClick,
            onLongClick = onLongClick,
            shape = CardDefaults.shape(thumbShape),
            scale = CardDefaults.scale(focusedScale = 1.06f),
            border = CardDefaults.border(focusedBorder = Border(BorderStroke(3.dp, Nv.accent), shape = thumbShape)),
            colors = CardDefaults.colors(containerColor = Nv.surface),
            modifier = modifier.fillMaxWidth().aspectRatio(16f / 9f),
        ) {
            Box(Modifier.fillMaxSize()) {
                ArtworkImage(episode.thumb, Modifier.fillMaxSize())
                if (episode.watch.watched && episode.watch.positionMs == null) {
                    Box(
                        Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).clip(CircleShape).background(Nv.accent),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(NvIcons.Check, null, tint = Nv.accentText, modifier = Modifier.size(15.dp))
                    }
                }
                val position = episode.watch.positionMs
                val runtime = episode.runtimeMs
                if (position != null && runtime != null && runtime > 0) {
                    ProgressBar(position.toFloat() / runtime, Modifier.align(Alignment.BottomCenter))
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Text(episode.label, style = MaterialTheme.typography.labelMedium, color = Nv.textMuted)
        Text(episode.title, style = MaterialTheme.typography.titleSmall, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        runtimeText(episode.runtimeMs)?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Nv.textMuted) }
    }
}
