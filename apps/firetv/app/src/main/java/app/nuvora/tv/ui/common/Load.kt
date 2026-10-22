package app.nuvora.tv.ui.common

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.lifecycle.compose.LifecycleResumeEffect
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.NuvoraApi
import kotlinx.coroutines.android.awaitFrame

/** A screen's data: loading, there, or failed. */
sealed interface Load<out T> {
    data object Loading : Load<Nothing>
    data class Ready<T>(val value: T) : Load<T>
    data class Failed(val error: ApiException) : Load<Nothing>
}

/** Called when the server says the session has ended. */
val LocalSessionEnded = staticCompositionLocalOf<() -> Unit> { {} }

/**
 * Loads [block] when [keys] change, and again quietly each time the screen
 * comes back into view (after the player, progress has moved): the old data
 * stays up while the new loads.
 */
@Composable
fun <T> rememberLoad(vararg keys: Any?, block: suspend NuvoraApi.() -> T): Pair<Load<T>, () -> Unit> {
    val api = LocalApi.current
    val sessionEnded = LocalSessionEnded.current
    var state by remember(*keys) { mutableStateOf<Load<T>>(Load.Loading) }
    var generation by remember(*keys) { mutableIntStateOf(0) }
    var resumes by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        // The first resume is the screen opening, already loading.
        if (resumes++ > 0) generation++
        onPauseOrDispose {}
    }
    LaunchedEffect(api, generation, *keys) {
        state = try {
            Load.Ready(api.block())
        } catch (e: ApiException) {
            if (e.unauthorized) sessionEnded()
            if (state is Load.Ready) state else Load.Failed(e)
        }
    }
    return state to { generation++ }
}

/** A requester that takes focus once the screen is laid out. */
@Composable
fun rememberInitialFocus(key: Any? = Unit): FocusRequester {
    val requester = remember { FocusRequester() }
    LaunchedEffect(key) {
        awaitFrame()
        runCatching { requester.requestFocus() }
    }
    return requester
}

fun Modifier.initialFocus(requester: FocusRequester): Modifier = focusRequester(requester)

fun Modifier.onFocusGained(block: () -> Unit): Modifier = onFocusChanged { if (it.isFocused) block() }

/**
 * Scrolls just enough to show what gained focus, from whichever edge it is
 * past, plus a little of what comes next so the page reads as going on.
 */
@OptIn(ExperimentalFoundationApi::class)
val MinimalScroll = object : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float {
        val end = offset + size
        val margin = containerSize * 0.08f
        return when {
            offset >= 0 && end <= containerSize -> 0f
            offset < 0 || size > containerSize -> offset - margin
            else -> end - containerSize + margin
        }
    }
}
