package app.nuvora.tv.ui.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.PosterCard
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.NvChip
import app.nuvora.tv.ui.common.PosterCardView
import app.nuvora.tv.ui.common.PosterWidth
import app.nuvora.tv.ui.common.key
import app.nuvora.tv.ui.common.onFocusGained
import kotlinx.coroutines.android.awaitFrame
import app.nuvora.tv.ui.home.Section
import app.nuvora.tv.ui.home.Shell
import app.nuvora.tv.ui.open
import app.nuvora.tv.ui.theme.Nv

private enum class Sort(val label: String, val sort: String, val order: String?) {
    Title("A–Z", "title", null),
    Added("Recently added", "added", "desc"),
    Year("Newest", "year", "desc"),
}

/** Every movie or show of one library, as a grid that loads as you scroll. */
@Composable
fun LibraryScreen(nav: NavHostController, id: Long, name: String) {
    val api = LocalApi.current
    var sort by rememberSaveable(id) { mutableStateOf(Sort.Title) }
    var unwatched by rememberSaveable(id) { mutableStateOf(false) }
    val cards = remember(id, sort, unwatched) { mutableStateListOf<PosterCard>() }
    var next by remember(id, sort, unwatched) { mutableStateOf<String?>(null) }
    var total by remember(id, sort, unwatched) { mutableStateOf<Int?>(null) }
    var loading by remember(id, sort, unwatched) { mutableStateOf(false) }
    var error by remember(id, sort, unwatched) { mutableStateOf<String?>(null) }
    val grid = rememberLazyGridState()
    // The card focus was on, so coming back from a title puts it there again.
    var lastKey by rememberSaveable(id) { mutableStateOf<String?>(null) }
    val requester = remember { FocusRequester() }

    suspend fun more() {
        if (loading || (total != null && next == null)) return
        loading = true
        try {
            val page = api.libraryItems(id, sort.sort, sort.order, if (unwatched) "unwatched" else "all", next)
            cards.addAll(page.items)
            next = page.next
            total = page.total
        } catch (e: ApiException) {
            error = e.message
        }
        loading = false
    }

    LaunchedEffect(id, sort, unwatched) { more() }
    LaunchedEffect(cards.isNotEmpty()) {
        if (cards.isEmpty()) return@LaunchedEffect
        awaitFrame()
        runCatching { requester.requestFocus() }
    }
    val nearEnd by remember { derivedStateOf { (grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) > cards.size - 14 } }
    LaunchedEffect(nearEnd, cards.size) { if (nearEnd) more() }

    Shell(nav, Section.Library(id)) {
        Column(Modifier.fillMaxSize().padding(top = 36.dp)) {
            Row(Modifier.padding(horizontal = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(name, style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary)
                total?.let {
                    Spacer(Modifier.width(14.dp))
                    Text("$it", style = MaterialTheme.typography.titleMedium, color = Nv.textMuted)
                }
                Spacer(Modifier.weight(1f))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Sort.entries.forEach { option ->
                        NvChip(option.label, sort == option) { sort = option }
                    }
                    Spacer(Modifier.width(12.dp))
                    NvChip("Unwatched", unwatched) { unwatched = !unwatched }
                }
            }
            when {
                error != null && cards.isEmpty() -> ErrorState("Can't load $name", error.orEmpty())
                cards.isEmpty() && total == null -> Loading()
                cards.isEmpty() -> ErrorState("Nothing here", if (unwatched) "You've watched everything in $name." else "$name is empty.")
                else -> LazyVerticalGrid(
                    columns = GridCells.Adaptive(PosterWidth),
                    state = grid,
                    contentPadding = PaddingValues(start = 48.dp, end = 48.dp, top = 24.dp, bottom = 48.dp),
                    horizontalArrangement = Arrangement.spacedBy(22.dp),
                    verticalArrangement = Arrangement.spacedBy(26.dp),
                    modifier = Modifier.focusRestorer(),
                ) {
                    items(cards, key = { it.key }) { card ->
                        val target = card.key == (lastKey ?: cards.first().key)
                        PosterCardView(
                            card,
                            onClick = { nav.open(card.kind, card.id) },
                            modifier = Modifier.onFocusGained { lastKey = card.key }.then(if (target) Modifier.focusRequester(requester) else Modifier),
                        )
                    }
                }
            }
        }
    }
}
