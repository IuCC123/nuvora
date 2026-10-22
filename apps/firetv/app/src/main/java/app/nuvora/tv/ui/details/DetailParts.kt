package app.nuvora.tv.ui.details

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
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import androidx.tv.material3.Card
import androidx.tv.material3.CardDefaults
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.CastMember
import app.nuvora.tv.data.Image
import app.nuvora.tv.ui.common.ArtworkImage
import app.nuvora.tv.ui.common.TitleCard
import app.nuvora.tv.ui.theme.Nv

/** `5430000` -> `"1h 30m"`. */
fun runtimeText(ms: Long?): String? {
    if (ms == null || ms <= 0) return null
    val minutes = (ms / 60000).toInt()
    return if (minutes >= 60) "${minutes / 60}h ${minutes % 60}m" else "${minutes}m"
}

/** `754000` -> `"12:34"`, `3723000` -> `"1:02:03"`. */
fun clockText(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    val h = total / 3600
    val m = (total % 3600) / 60
    val s = total % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}

/**
 * A title's page: the backdrop across the screen, darkened where the text
 * sits; the poster and the details in front; [below] (seasons, episodes,
 * cast) underneath.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun DetailLayout(
    backdrop: Image?,
    poster: Image?,
    title: String,
    year: Int?,
    meta: List<String>,
    tagline: String?,
    genres: List<String>,
    overview: String?,
    actions: @Composable () -> Unit,
    below: @Composable () -> Unit,
) {
    Box(Modifier.fillMaxSize()) {
        if (backdrop != null) {
            ArtworkImage(backdrop, Modifier.fillMaxWidth().aspectRatio(16f / 9f).align(Alignment.TopEnd), size = "large")
        }
        Box(Modifier.fillMaxSize().background(Brush.horizontalGradient(0f to Nv.canvas, 0.45f to Nv.canvas.copy(alpha = 0.92f), 1f to Color.Transparent)))
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.35f to Color.Transparent, 0.8f to Nv.canvas)))
        // TV lists keep the focused item at a fixed point down the screen; a
        // detail page instead scrolls only as far as the focused row needs,
        // so the title stays up while Play is focused.
        CompositionLocalProvider(LocalBringIntoViewSpec provides MinimalScroll) {
        androidx.compose.foundation.lazy.LazyColumn(
            contentPadding = PaddingValues(top = 56.dp, bottom = 48.dp),
            verticalArrangement = Arrangement.spacedBy(32.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                Row(Modifier.padding(horizontal = 56.dp)) {
                    Box(Modifier.width(220.dp).aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp))) {
                        if (poster != null) ArtworkImage(poster, Modifier.fillMaxSize(), size = "large") else TitleCard(title, year)
                    }
                    Spacer(Modifier.width(40.dp))
                    Column(Modifier.width(720.dp)) {
                        Text(title, style = MaterialTheme.typography.displaySmall, color = Nv.textPrimary, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Spacer(Modifier.height(10.dp))
                        if (meta.isNotEmpty()) {
                            Text(meta.joinToString("  ·  "), style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary)
                        }
                        if (genres.isNotEmpty()) {
                            Spacer(Modifier.height(6.dp))
                            Text(genres.joinToString(", "), style = MaterialTheme.typography.bodyMedium, color = Nv.textMuted)
                        }
                        if (!tagline.isNullOrBlank()) {
                            Spacer(Modifier.height(14.dp))
                            Text(tagline, style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
                        }
                        if (!overview.isNullOrBlank()) {
                            Spacer(Modifier.height(12.dp))
                            Text(overview, style = MaterialTheme.typography.bodyLarge, color = Nv.textSecondary, maxLines = 5, overflow = TextOverflow.Ellipsis)
                        }
                        Spacer(Modifier.height(24.dp))
                        actions()
                    }
                }
            }
            item { below() }
        }
        }
    }
}

/** Scrolls just enough to show what gained focus, from whichever edge it is past. */
@OptIn(ExperimentalFoundationApi::class)
private val MinimalScroll = object : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float {
        val end = offset + size
        return when {
            offset >= 0 && end <= containerSize -> 0f
            offset < 0 || size > containerSize -> offset
            else -> end - containerSize
        }
    }
}

/** The cast, as round photos with who they play. */
@Composable
fun CastRow(cast: List<CastMember>) {
    if (cast.isEmpty()) return
    Column {
        Text("Cast", style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary, modifier = Modifier.padding(start = 56.dp, bottom = 14.dp))
        LazyRow(
            contentPadding = PaddingValues(horizontal = 56.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(22.dp),
            modifier = Modifier.focusRestorer(),
        ) {
            items(cast.distinctBy { it.name + it.character }.take(30)) { member ->
                Column(Modifier.width(120.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Card(
                        onClick = {},
                        shape = CardDefaults.shape(CircleShape),
                        scale = CardDefaults.scale(focusedScale = 1.06f),
                        border = CardDefaults.border(focusedBorder = Border(androidx.compose.foundation.BorderStroke(3.dp, Nv.accent), shape = CircleShape)),
                        modifier = Modifier.size(104.dp),
                    ) {
                        val face = member.image ?: member.characterImage
                        if (face != null) {
                            ArtworkImage(face, Modifier.fillMaxSize())
                        } else {
                            Box(Modifier.fillMaxSize().background(Nv.surfaceRaised), contentAlignment = Alignment.Center) {
                                Text(member.name.split(" ").mapNotNull { it.firstOrNull()?.uppercase() }.take(2).joinToString(""), color = Nv.textSecondary)
                            }
                        }
                    }
                    Spacer(Modifier.height(8.dp))
                    Text(member.name, style = MaterialTheme.typography.labelLarge, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    member.character?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = Nv.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

/** Why nothing plays here, when the server says so. */
@Composable
fun NotPlayable(reasons: List<String>) {
    Column(Modifier.clip(RoundedCornerShape(10.dp)).background(Nv.surface).padding(16.dp)) {
        Text("This can't play on this TV", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
        reasons.forEach { Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.textSecondary) }
    }
}
