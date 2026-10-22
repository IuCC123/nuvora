package app.nuvora.tv.ui.search

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.PosterCard
import app.nuvora.tv.data.SearchResponse
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.PosterCardView
import app.nuvora.tv.ui.common.PosterWidth
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.home.Section
import app.nuvora.tv.ui.home.Shell
import app.nuvora.tv.ui.open
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.delay

/** Type with the remote (or the phone keyboard); results follow as you type. */
@Composable
fun SearchScreen(nav: NavHostController) {
    val api = LocalApi.current
    var query by rememberSaveable { mutableStateOf("") }
    var results by remember { mutableStateOf<SearchResponse?>(null) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    val focus = rememberInitialFocus()

    LaunchedEffect(query) {
        if (query.isBlank()) {
            results = null
            return@LaunchedEffect
        }
        delay(300)
        try {
            results = api.search(query.trim())
            error = null
        } catch (e: ApiException) {
            error = e.message
        }
    }

    Shell(nav, Section.Search) {
        Column(Modifier.fillMaxSize().padding(top = 36.dp)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                singleLine = true,
                placeholder = { androidx.compose.material3.Text("Movies, shows, episodes, people", color = Nv.textMuted) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search, autoCorrectEnabled = false),
                shape = RoundedCornerShape(50),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedTextColor = Nv.textPrimary,
                    unfocusedTextColor = Nv.textPrimary,
                    focusedBorderColor = Nv.accent,
                    unfocusedBorderColor = Nv.hairline,
                    cursorColor = Nv.accent,
                    focusedContainerColor = Nv.surface,
                    unfocusedContainerColor = Nv.surface,
                ),
                modifier = Modifier.padding(horizontal = 48.dp).fillMaxWidth(0.6f).initialFocus(focus),
            )
            val found = results
            when {
                error != null -> Message(error.orEmpty())
                query.isBlank() -> Message("Find anything by its title, an episode by its name, or an actor or voice actor.")
                found == null -> {}
                found.titles.isEmpty() && found.episodes.isEmpty() -> Message("Nothing matches “${query.trim()}”.")
                else -> Results(nav, found)
            }
        }
    }
}

@Composable
private fun Results(nav: NavHostController, found: SearchResponse) {
    val episodeCards = found.episodes.map {
        PosterCard(id = it.id, kind = "episode", title = it.showTitle, poster = it.poster, subtitle = "S${it.season} · E${it.episode} · ${it.title}")
    }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(PosterWidth),
        contentPadding = PaddingValues(start = 48.dp, end = 48.dp, top = 28.dp, bottom = 48.dp),
        horizontalArrangement = Arrangement.spacedBy(22.dp),
        verticalArrangement = Arrangement.spacedBy(26.dp),
        modifier = Modifier.focusRestorer(),
    ) {
        if (found.titles.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { Heading("Movies & shows") }
            items(found.titles, key = { "t${it.kind}${it.id}" }) { card ->
                PosterCardView(card, onClick = { nav.open(card.kind, card.id) })
            }
        }
        if (episodeCards.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { Heading("Episodes") }
            items(episodeCards, key = { "e${it.id}" }) { card ->
                PosterCardView(card, onClick = { nav.navigate(PlayerRoute(card.id)) })
            }
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary)
}

@Composable
private fun Message(text: String) {
    Text(text, style = MaterialTheme.typography.bodyLarge, color = Nv.textSecondary, modifier = Modifier.padding(start = 52.dp, top = 28.dp))
}
