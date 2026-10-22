package app.nuvora.tv.ui.player

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.nuvora.tv.data.SubtitleStyle
import app.nuvora.tv.data.languageName
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.OptionChips
import app.nuvora.tv.ui.theme.Nv

/** A panel down the right edge, over a dimmed picture. */
@Composable
private fun SidePanel(title: String, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.5f))) {
        Column(Modifier.align(Alignment.CenterEnd).fillMaxHeight().width(460.dp).background(Nv.surface).padding(28.dp)) {
            Text(title, style = MaterialTheme.typography.headlineSmall, color = Nv.textPrimary)
            Spacer(Modifier.height(18.dp))
            content()
        }
    }
}

/** Off and every subtitle track, then how they look. A pick applies at once and is remembered for the show or movie. */
@Composable
fun SubtitleMenu(
    tracks: List<TextTrack>,
    selected: TextTrack?,
    style: SubtitleStyle,
    onPick: (TextTrack?) -> Unit,
    onStyle: (SubtitleStyle) -> Unit,
) {
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    SidePanel("Subtitles") {
        LazyColumn(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            item {
                TrackRow("Off", selected == null, if (selected == null) Modifier.focusRequester(first) else Modifier) { onPick(null) }
            }
            items(tracks) { track ->
                val on = selected == track
                TrackRow(track.option.label, on, if (on) Modifier.focusRequester(first) else Modifier) { onPick(track) }
            }
            if (tracks.isEmpty()) {
                item { Text("This video has no subtitles.", style = MaterialTheme.typography.bodyMedium, color = Nv.textMuted, modifier = Modifier.padding(16.dp)) }
            }
            item {
                Column(Modifier.padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text("Size", style = MaterialTheme.typography.labelLarge, color = Nv.textSecondary)
                    OptionChips(SubtitleStyle.SIZES, style.size) { onStyle(style.copy(size = it)) }
                    Spacer(Modifier.height(4.dp))
                    Text("Background", style = MaterialTheme.typography.labelLarge, color = Nv.textSecondary)
                    OptionChips(SubtitleStyle.BACKGROUNDS, style.background) { onStyle(style.copy(background = it)) }
                }
            }
        }
    }
}

/** The audio tracks ExoPlayer found. */
@OptIn(UnstableApi::class)
@Composable
fun AudioMenu(tracks: Tracks, player: ExoPlayer, onDone: () -> Unit) {
    val groups = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    SidePanel("Audio") {
        LazyColumn(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val all = groups.flatMap { group -> (0 until group.length).map { group to it } }
            val focused = all.indexOfFirst { (g, i) -> g.isTrackSelected(i) }.coerceAtLeast(0)
            items(all.size) { n ->
                val (group, index) = all[n]
                TrackRow(audioLabel(group.getTrackFormat(index)), group.isTrackSelected(index), if (n == focused) Modifier.focusRequester(first) else Modifier) {
                    player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                        .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index))
                        .build()
                    onDone()
                }
            }
        }
    }
}

@Composable
private fun TrackRow(label: String, selected: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = Color.Transparent,
            contentColor = if (selected) Nv.accent else Nv.textPrimary,
            focusedContainerColor = Nv.textPrimary,
            focusedContentColor = Nv.canvas,
        ),
        modifier = modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.width(28.dp)) { if (selected) Icon(NvIcons.Check, null, modifier = Modifier.size(18.dp)) }
            Text(label, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** `"Japanese · Stereo"`, `"English · Commentary · 5.1"`. */
@OptIn(UnstableApi::class)
private fun audioLabel(format: Format): String {
    val language = languageName(format.language?.takeIf { it != C.LANGUAGE_UNDETERMINED })
    val extras = buildList {
        format.label?.takeIf { it.isNotBlank() && !it.equals(language, ignoreCase = true) }?.let(::add)
        when (format.channelCount) {
            1 -> add("Mono")
            2 -> add("Stereo")
            6 -> add("5.1")
            8 -> add("7.1")
        }
    }
    return (listOf(language) + extras).joinToString(" · ")
}
