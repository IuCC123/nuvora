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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
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
import androidx.tv.material3.FilterChip
import androidx.tv.material3.FilterChipDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.Episode
import app.nuvora.tv.data.ShowDetail
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.common.ArtworkImage
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.ProgressBar
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.theme.Nv

@Composable
fun ShowScreen(nav: NavHostController, id: Long) {
    val (item) = rememberLoad(id) { item(id) }
    when (val state = item) {
        Load.Loading -> Loading()
        is Load.Failed -> ErrorState("Can't open this show", state.error.message ?: "", "Back") { nav.popBackStack() }
        is Load.Ready -> (state.value as? ShowDetail)?.let { Show(nav, it) }
            ?: ErrorState("This isn't a show", "It may have been changed on the server.", "Back") { nav.popBackStack() }
    }
}

@Composable
private fun Show(nav: NavHostController, show: ShowDetail) {
    val next = show.next
    var seasonId by rememberSaveable(show.id) {
        mutableLongStateOf(next?.seasonId ?: show.seasons.firstOrNull()?.id ?: 0L)
    }
    val focus = rememberInitialFocus(show.id)
    val meta = listOfNotNull(
        show.yearRange,
        "${show.seasons.count { it.index > 0 }} season${if (show.seasons.count { it.index > 0 } == 1) "" else "s"}",
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
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                if (next != null) {
                    val resume = next.positionMs
                    NvButton(
                        if (resume != null) "Resume ${next.label} from ${clockText(resume)}" else "Play ${next.label}",
                        { nav.navigate(PlayerRoute(next.id)) },
                        Modifier.initialFocus(focus),
                        icon = NvIcons.Play,
                        primary = true,
                    )
                } else {
                    Text("You've watched every episode.", style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary)
                }
            }
        },
        below = {
            Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
                if (show.seasons.size > 1) {
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = 56.dp),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        modifier = Modifier.focusRestorer(),
                    ) {
                        items(show.seasons, key = { it.id }) { season ->
                            FilterChip(
                                selected = season.id == seasonId,
                                onClick = { seasonId = season.id },
                                colors = FilterChipDefaults.colors(
                                    containerColor = Nv.surface,
                                    contentColor = Nv.textSecondary,
                                    selectedContainerColor = Nv.surfaceRaised,
                                    selectedContentColor = Nv.accent,
                                    focusedContainerColor = Nv.textPrimary,
                                    focusedContentColor = Nv.canvas,
                                    focusedSelectedContainerColor = Nv.accent,
                                    focusedSelectedContentColor = Nv.accentText,
                                ),
                            ) { Text(season.title, style = MaterialTheme.typography.labelLarge) }
                        }
                    }
                }
                if (seasonId != 0L) Episodes(nav, seasonId)
                CastRow(show.details.cast)
            }
        },
    )
}

@Composable
private fun Episodes(nav: NavHostController, seasonId: Long) {
    val (episodes) = rememberLoad(seasonId) { episodes(seasonId) }
    val list = (episodes as? Load.Ready<List<Episode>>)?.value ?: return
    LazyRow(
        contentPadding = PaddingValues(horizontal = 56.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(22.dp),
        modifier = Modifier.focusRestorer(),
    ) {
        items(list, key = { it.id }) { episode ->
            EpisodeCard(episode) { nav.navigate(PlayerRoute(episode.id)) }
        }
    }
}

private val thumbShape = RoundedCornerShape(8.dp)

@Composable
private fun EpisodeCard(episode: Episode, onClick: () -> Unit) {
    Column(Modifier.width(300.dp)) {
        Card(
            onClick = onClick,
            shape = CardDefaults.shape(thumbShape),
            scale = CardDefaults.scale(focusedScale = 1.06f),
            border = CardDefaults.border(focusedBorder = Border(BorderStroke(3.dp, Nv.accent), shape = thumbShape)),
            colors = CardDefaults.colors(containerColor = Nv.surface),
            modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f),
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
