package app.nuvora.tv.ui.settings

import androidx.activity.compose.BackHandler
import androidx.annotation.OptIn
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.runtime.CompositionLocalProvider
import app.nuvora.tv.ui.common.MinimalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.SubtitleView
import androidx.navigation.NavHostController
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.nuvora.tv.BuildConfig
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.LANGUAGES
import app.nuvora.tv.data.ScanStatus
import app.nuvora.tv.data.ScanSummary
import app.nuvora.tv.data.Session
import app.nuvora.tv.data.SubtitleStyle
import app.nuvora.tv.data.languageName
import app.nuvora.tv.data.probeDeviceProfile
import app.nuvora.tv.store
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.OptionChips
import app.nuvora.tv.ui.player.applyStyle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.Locale
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.home.Section
import app.nuvora.tv.ui.home.Shell
import app.nuvora.tv.ui.theme.Nv

private val MODES = listOf("auto" to "Auto", "always" to "Always", "off" to "Off")
private val MODE_HINTS = mapOf(
    "auto" to "On when the audio isn't in your language, or when the file marks a track to show.",
    "always" to "On whenever there's a track in your language, or one the file marks to show.",
    "off" to "Off until you turn them on in the player.",
)

@kotlin.OptIn(ExperimentalFoundationApi::class)
@Composable
fun SettingsScreen(nav: NavHostController, session: Session?, onSignOut: () -> Unit, onChangeServer: () -> Unit) {
    val context = LocalContext.current
    val store = context.store
    val api = LocalApi.current
    val scope = rememberCoroutineScope()
    val profile = remember { probeDeviceProfile(context) }
    val focus = rememberInitialFocus()
    val user = session?.user
    var style by remember { mutableStateOf(store.subtitleStyle) }
    var picking by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<String?>(null) }

    fun saveSubtitles(mode: String, language: String?) {
        scope.launch {
            problem = null
            try {
                store.updateUser(api.setSubtitlePrefs(mode, language))
            } catch (e: ApiException) {
                problem = e.message
            }
        }
    }

    Shell(nav, Section.Settings) {
        Box(Modifier.fillMaxSize()) {
            // Scroll only as far as the focused panel needs, so the page opens at its top.
            CompositionLocalProvider(LocalBringIntoViewSpec provides MinimalScroll) {
            LazyColumn(
                contentPadding = PaddingValues(horizontal = 56.dp, vertical = 44.dp),
                verticalArrangement = Arrangement.spacedBy(22.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                item { Text("Settings", style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary) }
                item {
                    Panel {
                        Label("SIGNED IN")
                        Text(user?.displayName.orEmpty(), style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary)
                        Text(
                            "${session?.server?.name.orEmpty()} · ${session?.server?.url.orEmpty().removePrefix("http://").removePrefix("https://")}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Nv.textSecondary,
                        )
                        Spacer(Modifier.height(18.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                            NvButton("Sign out", onSignOut, Modifier.initialFocus(focus), icon = NvIcons.SignOut)
                            NvButton("Use another server", onChangeServer, icon = NvIcons.Server)
                        }
                    }
                }
                if (user != null) {
                    item {
                        val mode = user.subtitleMode
                        Panel {
                            Label("SUBTITLES")
                            Text("When to show them", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
                            Spacer(Modifier.height(8.dp))
                            OptionChips(MODES, mode) { saveSubtitles(it, user.subtitleLanguage) }
                            Spacer(Modifier.height(6.dp))
                            Text(MODE_HINTS[mode].orEmpty(), style = MaterialTheme.typography.bodyMedium, color = Nv.textSecondary)
                            Spacer(Modifier.height(18.dp))
                            Text("Language", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
                            Spacer(Modifier.height(8.dp))
                            NvButton(
                                user.subtitleLanguage?.let(::languageName) ?: "This TV's language (${Locale.getDefault().displayLanguage})",
                                { picking = true },
                                icon = NvIcons.Subtitles,
                            )
                            problem?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.danger) }
                            Text(
                                "Saved to your account, so the web app and other TVs use it too. The player remembers what you pick for each show or movie.",
                                style = MaterialTheme.typography.bodySmall,
                                color = Nv.textMuted,
                                modifier = Modifier.padding(top = 10.dp),
                            )
                        }
                    }
                }
                item {
                    Panel {
                        Label("HOW SUBTITLES LOOK ON THIS TV")
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                Text("Size", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
                                OptionChips(SubtitleStyle.SIZES, style.size) { style = style.copy(size = it); store.subtitleStyle = style }
                                Spacer(Modifier.height(8.dp))
                                Text("Background", style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary)
                                OptionChips(SubtitleStyle.BACKGROUNDS, style.background) { style = style.copy(background = it); store.subtitleStyle = style }
                            }
                            Spacer(Modifier.width(24.dp))
                            SubtitlePreview(style, Modifier.width(360.dp).aspectRatio(16f / 9f))
                        }
                    }
                }
                if (user?.role == "admin") item { ScanPanel() }
                item {
                    Panel {
                        Label("THIS TV PLAYS")
                        Text(
                            profile.video.joinToString("   ") { v ->
                                val name = when (v.codec) { "h264" -> "H.264"; "hevc" -> "HEVC"; "av1" -> "AV1"; "vp9" -> "VP9"; else -> v.codec }
                                val res = if (v.maxHeight >= 2160) "4K" else "${v.maxHeight}p"
                                val hdr = if (v.hdr.isNotEmpty()) " " + v.hdr.joinToString("/") { it.uppercase().replace("_", " ") } else ""
                                "$name $res${if (v.maxBitDepth > 8) " 10-bit" else ""}$hdr"
                            },
                            style = MaterialTheme.typography.bodyLarge,
                            color = Nv.textPrimary,
                        )
                        Text(
                            "Audio: " + profile.audio.joinToString(", ") { it.codec.uppercase().replace("EAC3", "E-AC-3").replace("AC3", "AC-3") },
                            style = MaterialTheme.typography.bodyMedium,
                            color = Nv.textSecondary,
                        )
                        Text(
                            "Anything else plays too: the server converts just the part this TV can't decode.",
                            style = MaterialTheme.typography.bodySmall,
                            color = Nv.textMuted,
                        )
                    }
                }
                item { Text("Nuvora for TV ${BuildConfig.VERSION_NAME}", style = MaterialTheme.typography.bodySmall, color = Nv.textMuted) }
            }
            }
            if (picking && user != null) {
                LanguagePicker(
                    current = user.subtitleLanguage,
                    onPick = { code ->
                        picking = false
                        saveSubtitles(user.subtitleMode, code)
                    },
                    onClose = { picking = false },
                )
            }
        }
    }
}

/** A frame of video with a line of subtitles, drawn by the player's own subtitle view. */
@OptIn(UnstableApi::class)
@Composable
private fun SubtitlePreview(style: SubtitleStyle, modifier: Modifier) {
    Box(modifier.clip(RoundedCornerShape(10.dp)).background(Brush.linearGradient(listOf(Color(0xFF3B4A5C), Color(0xFFB98A4E), Color(0xFFE9D7B5))))) {
        AndroidView(
            factory = { ctx ->
                SubtitleView(ctx).apply {
                    setCues(listOf(Cue.Builder().setText("Where are we going?\nSomewhere bright.").build()))
                }
            },
            update = { it.applyStyle(style) },
            modifier = Modifier.fillMaxSize(),
        )
    }
}

/** Every language, the TV's own first; picking one saves it. */
@Composable
private fun LanguagePicker(current: String?, onPick: (String?) -> Unit, onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val options = remember { listOf<Pair<String?, String>>(null to "This TV's language (${Locale.getDefault().displayLanguage})") + LANGUAGES.sortedBy { it.name }.map { it.code to it.name } }
    val at = options.indexOfFirst { it.first == current }.coerceAtLeast(0)
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.5f))) {
        Column(Modifier.align(Alignment.CenterEnd).fillMaxHeight().width(460.dp).background(Nv.surface).padding(28.dp)) {
            Text("Subtitle language", style = MaterialTheme.typography.headlineSmall, color = Nv.textPrimary)
            Spacer(Modifier.height(18.dp))
            LazyColumn(state = rememberLazyListState(initialFirstVisibleItemIndex = (at - 3).coerceAtLeast(0)), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(options.size) { n ->
                    val (code, name) = options[n]
                    val selected = code == current
                    Surface(
                        onClick = { onPick(code) },
                        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
                        colors = ClickableSurfaceDefaults.colors(
                            containerColor = Color.Transparent,
                            contentColor = if (selected) Nv.accent else Nv.textPrimary,
                            focusedContainerColor = Nv.textPrimary,
                            focusedContentColor = Nv.canvas,
                        ),
                        modifier = (if (n == at) Modifier.focusRequester(first) else Modifier).fillMaxWidth(),
                    ) {
                        Row(Modifier.padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.width(28.dp)) { if (selected) Icon(NvIcons.Check, null, modifier = Modifier.size(18.dp)) }
                            Text(name, style = MaterialTheme.typography.titleSmall)
                        }
                    }
                }
            }
        }
    }
}

/** For admins: scan every library now, and how the last scan went. */
@Composable
private fun ScanPanel() {
    val api = LocalApi.current
    val scope = rememberCoroutineScope()
    var status by remember { mutableStateOf<ScanStatus?>(null) }
    var problem by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        while (true) {
            status = runCatching { api.scanStatus() }.getOrNull() ?: status
            delay(if (status?.running == true) 1_000 else 5_000)
        }
    }
    Panel {
        Label("LIBRARIES")
        val s = status
        Text(
            when {
                s == null -> "…"
                s.running -> when (s.phase) {
                    "discovering" -> "Scanning: looking for files…"
                    "reading" -> "Scanning: reading ${s.done} of ${s.total} files"
                    "artwork" -> "Scanning: fetching artwork…"
                    else -> "Scanning: tidying up…"
                }
                s.last != null -> lastScan(s.last)
                else -> "Not scanned yet."
            },
            style = MaterialTheme.typography.titleMedium,
            color = Nv.textPrimary,
        )
        Text("New files show up after a scan. Folders and libraries are set up in a browser, in Settings › Libraries.", style = MaterialTheme.typography.bodyMedium, color = Nv.textSecondary)
        Spacer(Modifier.height(14.dp))
        NvButton(
            if (s?.running == true) "Scanning…" else "Scan libraries now",
            {
                scope.launch {
                    problem = null
                    try {
                        status = api.startScan()
                    } catch (e: ApiException) {
                        problem = e.message
                    }
                }
            },
            icon = NvIcons.Restart,
            enabled = s?.running != true,
        )
        problem?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.danger) }
    }
}

private fun lastScan(last: ScanSummary): String {
    val ago = (System.currentTimeMillis() - last.finishedAt) / 60_000
    val `when` = when {
        ago < 1 -> "just now"
        ago < 60 -> "$ago min ago"
        ago < 60 * 24 -> "${ago / 60} h ago"
        else -> "${ago / 1440} days ago"
    }
    val changes = listOfNotNull(
        last.added.takeIf { it > 0 }?.let { "$it added" },
        last.changed.takeIf { it > 0 }?.let { "$it changed" },
        last.removed.takeIf { it > 0 }?.let { "$it removed" },
        last.failed.takeIf { it > 0 }?.let { "$it couldn't be read" },
    )
    return "Last scan $`when`: " + (changes.joinToString(", ").ifEmpty { "nothing new" }) + "."
}

@Composable
private fun Panel(content: @Composable () -> Unit) {
    Column(Modifier.width(860.dp).clip(RoundedCornerShape(14.dp)).background(Nv.surface).padding(24.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        content()
    }
}

@Composable
private fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.labelSmall, color = Nv.textMuted)
    Spacer(Modifier.height(4.dp))
}
