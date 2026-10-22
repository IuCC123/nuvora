package app.nuvora.tv.ui.common

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ButtonDefaults
import androidx.tv.material3.Card
import androidx.tv.material3.CardDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.Image as Artwork
import app.nuvora.tv.data.NuvoraApi
import app.nuvora.tv.data.PosterCard
import app.nuvora.tv.ui.theme.Nv
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.request.crossfade

/** The signed-in server's API, for every screen under the root. */
val LocalApi = staticCompositionLocalOf<NuvoraApi> { error("no API: not signed in") }

/** An artwork: its ThumbHash blur at once, the image fading in over it. */
@Composable
fun ArtworkImage(
    image: Artwork?,
    modifier: Modifier = Modifier,
    size: String = "small",
    contentScale: ContentScale = ContentScale.Crop,
) {
    val api = LocalApi.current
    Box(modifier.background(Nv.surface)) {
        if (image == null) return@Box
        val placeholder = remember(image.id) { ThumbHash.bitmap(image.id, image.placeholder)?.asImageBitmap() }
        if (placeholder != null) {
            Image(placeholder, null, Modifier.fillMaxSize(), contentScale = contentScale)
        }
        val context = LocalContext.current
        val request = remember(image.id, size) {
            ImageRequest.Builder(context).data(api.imageUrl(image, size)).crossfade(220).build()
        }
        AsyncImage(request, contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = contentScale)
    }
}

private val posterShape = RoundedCornerShape(6.dp)

/**
 * A poster in a row or grid: scales up with an amber ring when focused, with
 * progress, watched and new-episode marks, and its title underneath.
 */
@Composable
fun PosterCardView(card: PosterCard, onClick: () -> Unit, modifier: Modifier = Modifier, width: Dp = PosterWidth) {
    Column(modifier.width(width)) {
        Card(
            onClick = onClick,
            shape = CardDefaults.shape(posterShape),
            scale = CardDefaults.scale(focusedScale = 1.08f),
            border = CardDefaults.border(focusedBorder = Border(BorderStroke(3.dp, Nv.accent), shape = posterShape)),
            colors = CardDefaults.colors(containerColor = Nv.surface),
            modifier = Modifier.fillMaxWidth().aspectRatio(2f / 3f),
        ) {
            Box(Modifier.fillMaxSize()) {
                if (card.poster != null) {
                    ArtworkImage(card.poster, Modifier.fillMaxSize())
                } else {
                    TitleCard(card.title, card.year)
                }
                card.unwatchedCount?.takeIf { it > 0 && !card.watched }?.let { count ->
                    Badge(count.toString(), Modifier.align(Alignment.TopEnd).padding(6.dp))
                }
                if (card.watched && card.progress == null) {
                    Box(
                        Modifier.align(Alignment.TopEnd).padding(6.dp).size(22.dp).clip(CircleShape).background(Nv.accent),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(NvIcons.Check, null, tint = Nv.accentText, modifier = Modifier.size(14.dp))
                    }
                }
                card.progress?.let { ProgressBar(it, Modifier.align(Alignment.BottomCenter)) }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(card.title, style = MaterialTheme.typography.titleSmall, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        val second = card.subtitle ?: card.year?.toString()
        if (second != null) {
            Text(second, style = MaterialTheme.typography.bodySmall, color = Nv.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** A poster for something without artwork: its title, lettered. */
@Composable
fun TitleCard(title: String, year: Int?) {
    Box(
        Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(Color(0xFF2A2D33), Color(0xFF17191C)))).padding(12.dp),
        contentAlignment = Alignment.BottomStart,
    ) {
        Column {
            Text(title.uppercase(), style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary, maxLines = 4, fontWeight = FontWeight.Bold)
            if (year != null) Text(year.toString(), style = MaterialTheme.typography.labelSmall, color = Nv.textSecondary)
        }
    }
}

@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(4.dp).background(Color.Black.copy(alpha = 0.6f))) {
        Box(Modifier.fillMaxWidth(fraction.coerceIn(0.02f, 1f)).height(4.dp).background(Nv.accent))
    }
}

@Composable
fun Badge(text: String, modifier: Modifier = Modifier) {
    Box(modifier.clip(RoundedCornerShape(50)).background(Color.Black.copy(alpha = 0.72f)).padding(horizontal = 8.dp, vertical = 2.dp)) {
        Text(text, style = MaterialTheme.typography.labelMedium, color = Nv.textPrimary)
    }
}

/** The width of a poster in rows and grids: about eight across a TV. */
val PosterWidth = 116.dp

/** A stable key for a card, for lists and for putting focus back on it. */
val PosterCard.key get() = "$kind:$id"

/**
 * A titled row of posters. The card keyed [focusKey] (the first when null)
 * carries [focusRequester], so a screen can put focus on it.
 */
@Composable
fun PosterRow(
    title: String,
    cards: List<PosterCard>,
    onOpen: (PosterCard) -> Unit,
    modifier: Modifier = Modifier,
    onFocus: (PosterCard) -> Unit = {},
    focusRequester: FocusRequester? = null,
    focusKey: String? = null,
) {
    Column(modifier) {
        Text(title, style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary, modifier = Modifier.padding(start = 48.dp, bottom = 8.dp))
        LazyRow(
            contentPadding = PaddingValues(horizontal = 48.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(18.dp),
            modifier = Modifier.focusRestorer(),
        ) {
            itemsIndexed(cards, key = { _, card -> card.key }) { index, card ->
                val target = focusRequester != null && (card.key == focusKey || (focusKey == null && index == 0))
                PosterCardView(
                    card,
                    onClick = { onOpen(card) },
                    width = PosterWidth,
                    modifier = Modifier.onFocusGained { onFocus(card) }.then(if (target) Modifier.focusRequester(focusRequester) else Modifier),
                )
            }
        }
    }
}

/** A pill button; `primary` is the amber one. */
@Composable
fun NvButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, icon: ImageVector? = null, primary: Boolean = false, enabled: Boolean = true) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier,
        shape = ButtonDefaults.shape(RoundedCornerShape(50)),
        colors = if (primary) {
            ButtonDefaults.colors(
                containerColor = Nv.accent,
                contentColor = Nv.accentText,
                focusedContainerColor = Color(0xFFF5B52A),
                focusedContentColor = Nv.accentText,
            )
        } else {
            ButtonDefaults.colors(
                containerColor = Nv.surfaceRaised,
                contentColor = Nv.textPrimary,
                focusedContainerColor = Nv.textPrimary,
                focusedContentColor = Nv.canvas,
            )
        },
        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            if (icon != null) Icon(icon, null, modifier = Modifier.size(20.dp))
            Text(text, style = MaterialTheme.typography.labelLarge)
        }
    }
}

@Composable
fun Loading(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text("Loading…", style = MaterialTheme.typography.bodyLarge, color = Nv.textMuted)
    }
}

/** What went wrong, and the one thing that helps. */
@Composable
fun ErrorState(title: String, detail: String, action: String? = null, onAction: (() -> Unit)? = null) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.width(560.dp)) {
            Text(title, style = MaterialTheme.typography.headlineSmall, color = Nv.textPrimary, textAlign = TextAlign.Center)
            Spacer(Modifier.height(10.dp))
            Text(detail, style = MaterialTheme.typography.bodyLarge, color = Nv.textSecondary, textAlign = TextAlign.Center)
            if (action != null && onAction != null) {
                Spacer(Modifier.height(24.dp))
                val focus = rememberInitialFocus()
                NvButton(action, onAction, primary = true, modifier = Modifier.initialFocus(focus))
            }
        }
    }
}
