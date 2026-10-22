package app.nuvora.tv.ui.home

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.HomeResponse
import app.nuvora.tv.data.PosterCard
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.PosterRow
import app.nuvora.tv.ui.common.ThumbHash
import app.nuvora.tv.ui.common.key
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.LibraryRoute
import app.nuvora.tv.ui.open
import app.nuvora.tv.ui.theme.Nv

/** Continue Watching, Recently Added and each library, as rows of posters. */
@Composable
fun HomeScreen(nav: NavHostController) {
    val (home, reload) = rememberLoad { home() }
    Shell(nav, Section.Home) {
        when (val state = home) {
            Load.Loading -> Loading()
            is Load.Failed -> ErrorState("Can't load Home", state.error.message ?: "", "Try again", reload)
            is Load.Ready -> HomeRows(nav, state.value)
        }
    }
}

@Composable
private fun HomeRows(nav: NavHostController, home: HomeResponse) {
    val rows = home.rows.filter { it.items.isNotEmpty() }
    if (rows.isEmpty()) {
        ErrorState("Nothing here yet", "Add a library on the server (Settings › Libraries in a browser), and it shows up here after a scan.")
        return
    }
    // Where focus was, as "row|card", so coming back from a title puts it there again.
    var last by rememberSaveable { mutableStateOf<String?>(null) }
    val (lastRow, lastCard) = last?.split("|", limit = 2).let { it?.getOrNull(0) to it?.getOrNull(1) }
    var focused by remember {
        mutableStateOf(rows.firstOrNull { rowKey(it) == lastRow }?.items?.firstOrNull { it.key == lastCard } ?: rows.first().items.first())
    }
    val requester = rememberInitialFocus()
    val focusRow = rows.firstOrNull { rowKey(it) == lastRow } ?: rows.first()
    Box(Modifier.fillMaxSize()) {
        FocusedBackdrop(focused)
        Column(Modifier.fillMaxSize()) {
            FocusedInfo(focused, Modifier.padding(start = 48.dp, top = 36.dp, end = 48.dp).height(118.dp))
            LazyColumn(
                contentPadding = PaddingValues(bottom = 48.dp),
                modifier = Modifier.weight(1f).focusRestorer(),
            ) {
                items(rows, key = ::rowKey) { row ->
                    PosterRow(
                        title = row.title,
                        cards = row.items,
                        onOpen = { card -> nav.open(card.kind, card.id) },
                        onFocus = {
                            focused = it
                            last = "${rowKey(row)}|${it.key}"
                        },
                        focusRequester = if (row === focusRow) requester else null,
                        focusKey = if (row === focusRow) lastCard else null,
                        onSeeAll = row.libraryId?.let { id -> { nav.navigate(LibraryRoute(id, row.title)) } },
                        total = row.total,
                        modifier = Modifier.padding(bottom = 18.dp),
                    )
                }
            }
        }
    }
}

private fun rowKey(row: app.nuvora.tv.data.HomeRow) = "${row.kind}:${row.libraryId}:${row.title}"

/** The focused title's name and second line, large, above the rows. */
@Composable
private fun FocusedInfo(card: PosterCard?, modifier: Modifier) {
    Column(modifier) {
        Text(
            card?.title.orEmpty(),
            style = MaterialTheme.typography.displaySmall,
            color = Nv.textPrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(6.dp))
        val second = listOfNotNull(card?.subtitle, card?.year?.toString()).firstOrNull()
        Text(second.orEmpty(), style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary)
    }
}

/**
 * A soft wash of the focused poster's colours behind the top of the screen:
 * its ThumbHash, stretched, is already a perfect blur on every Android version.
 */
@Composable
private fun FocusedBackdrop(card: PosterCard?) {
    Crossfade(card?.poster, animationSpec = tween(500), label = "backdrop") { poster ->
        Box(Modifier.fillMaxSize()) {
            val bitmap = poster?.let { remember(it.id) { ThumbHash.bitmap(it.id, it.placeholder)?.asImageBitmap() } }
            if (bitmap != null) {
                Image(
                    bitmap,
                    null,
                    contentScale = ContentScale.Crop,
                    alpha = 0.55f,
                    modifier = Modifier.align(Alignment.TopEnd).fillMaxWidth(0.7f).fillMaxHeight(0.75f),
                )
            }
            Box(Modifier.fillMaxSize().background(Brush.horizontalGradient(0.25f to Nv.canvas, 0.75f to Color.Transparent)))
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.2f to Color.Transparent, 0.62f to Nv.canvas)))
        }
    }
}
