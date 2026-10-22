package app.nuvora.tv.ui.player

import android.annotation.SuppressLint
import android.view.KeyEvent as AndroidKeyEvent
import androidx.annotation.OptIn
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
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
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.navigation.NavHostController
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Text
import app.nuvora.tv.data.EpisodeDetail
import app.nuvora.tv.data.EpisodeRef
import app.nuvora.tv.data.MovieDetail
import app.nuvora.tv.data.ProgressRequest
import app.nuvora.tv.data.RememberedSubtitle
import app.nuvora.tv.data.chooseSubtitle
import app.nuvora.tv.data.User
import app.nuvora.tv.data.Version
import app.nuvora.tv.data.WatchState
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.RESUME
import app.nuvora.tv.ui.common.ErrorState
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.Loading
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.details.clockText
import app.nuvora.tv.store
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.Locale

/** What the player plays: one movie or episode, one version of it. */
private data class Playable(
    val itemId: Long,
    val title: String,
    val subtitle: String?,
    val version: Version,
    val watch: WatchState,
    val next: EpisodeRef?,
    /** Where a subtitle choice carries over: `"show:12"` or `"movie:4"`. */
    val scope: String,
)

@Composable
fun PlayerScreen(nav: NavHostController, itemId: Long, fileId: Long, startMs: Long, user: User?) {
    val (item) = rememberLoad(itemId) { item(itemId) }
    when (val state = item) {
        Load.Loading -> Loading()
        is Load.Failed -> ErrorState("Can't play this", state.error.message ?: "", "Back") { nav.popBackStack() }
        is Load.Ready -> {
            val playable = when (val detail = state.value) {
                is MovieDetail -> playable(detail.id, detail.title, null, detail.versions, detail.watch, null, fileId, "movie:${detail.id}")
                is EpisodeDetail -> playable(detail.id, detail.showTitle, "${detail.label} · ${detail.title}", detail.versions, detail.watch, detail.next, fileId, "show:${detail.showId}")
                else -> null
            }
            if (playable == null) {
                ErrorState("Nothing to play", "This title has no file this TV can play.", "Back") { nav.popBackStack() }
            } else {
                val start = when {
                    startMs != RESUME -> startMs
                    playable.watch.fileId == playable.version.fileId -> playable.watch.positionMs ?: 0
                    else -> 0
                }
                Player(nav, playable, start, user)
            }
        }
    }
}

private fun playable(id: Long, title: String, subtitle: String?, versions: List<Version>, watch: WatchState, next: EpisodeRef?, fileId: Long, scope: String): Playable? {
    val version = versions.firstOrNull { it.fileId == fileId && it.playback.playable }
        ?: versions.firstOrNull { it.fileId == watch.fileId && it.playback.playable }
        ?: versions.firstOrNull { it.playback.playable }
        ?: return null
    return Playable(id, title, subtitle, version, watch, next, scope)
}

private const val HIDE_AFTER_MS = 5_000L
private const val REPORT_EVERY_MS = 10_000L

private enum class Menu { None, Subtitles, Audio }

@SuppressLint("UnsafeOptInUsageError")
@OptIn(UnstableApi::class)
@Composable
private fun Player(nav: NavHostController, playable: Playable, startMs: Long, user: User?) {
    val api = LocalApi.current
    val context = LocalContext.current
    val store = context.store
    val scope = rememberCoroutineScope()
    val version = playable.version
    // In a party, the party's clock starts the video, everyone at once.
    val autoplay = remember { store.party.state.value.let { !(it.connected && it.party != null) } }
    val player = remember(playable.itemId, version.fileId) { buildPlayer(context, api, version, startMs, autoplay) }
    val party = rememberPartyPlayer(store.party, player, user?.id, playable.itemId, version.fileId, startMs, version.playback.kind == "hls")

    var playing by remember { mutableStateOf(false) }
    var position by remember { mutableLongStateOf(startMs) }
    var duration by remember { mutableLongStateOf(version.media?.durationMs ?: 0L) }
    var buffering by remember { mutableStateOf(true) }
    var ended by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var controls by remember { mutableStateOf(true) }
    var touched by remember { mutableIntStateOf(0) }
    var menu by remember { mutableStateOf(Menu.None) }
    var tracks by remember { mutableStateOf(Tracks.EMPTY) }
    var style by remember { mutableStateOf(store.subtitleStyle) }
    val subtitles = remember(tracks) { textTracks(tracks) }
    val shownSubtitle = subtitles.firstOrNull { it.group.isTrackSelected(it.index) }
    val rootFocus = remember { FocusRequester() }
    val playFocus = remember { FocusRequester() }

    fun show() {
        controls = true
        touched++
    }

    fun report() {
        val dur = player.duration.takeIf { it > 0 } ?: duration
        if (dur <= 0) return
        val body = ProgressRequest(version.fileId, player.currentPosition.coerceIn(0, dur), dur)
        scope.launch { runCatching { api.reportProgress(playable.itemId, body) } }
    }

    fun playNext(next: EpisodeRef) {
        report()
        nav.navigate(PlayerRoute(next.id, 0, 0)) {
            popUpTo<PlayerRoute> { inclusive = true }
        }
    }

    /** The first time the tracks are known: the remembered choice, else the account's setting. */
    fun chooseFirstSubtitle(found: List<TextTrack>) {
        val remembered = user?.let { store.subtitleChoice(it.id, playable.scope) }
        val audio = version.media?.audio?.let { list -> list.firstOrNull { it.isDefault } ?: list.firstOrNull() }?.language
        val preferred = user?.subtitleLanguage ?: Locale.getDefault().language
        val pick = chooseSubtitle(found.map { it.option }, audio, user?.subtitleMode ?: "auto", preferred, remembered)
        player.showSubtitle(pick?.let { found[it] })
    }

    DisposableEffect(player) {
        var chosen = false
        val listener = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                playing = isPlaying
                if (!isPlaying) report()
            }

            override fun onPlaybackStateChanged(state: Int) {
                buffering = state == Player.STATE_BUFFERING
                ended = state == Player.STATE_ENDED
                if (player.duration > 0) duration = player.duration
                if (ended) {
                    report()
                    controls = playable.next == null
                }
            }

            override fun onTracksChanged(changed: Tracks) {
                tracks = changed
                val found = textTracks(changed)
                if (!chosen && found.isNotEmpty()) {
                    chosen = true
                    chooseFirstSubtitle(found)
                }
            }

            override fun onPlayerError(e: PlaybackException) {
                val status = (e.cause as? androidx.media3.datasource.HttpDataSource.InvalidResponseCodeException)?.responseCode
                error = when {
                    status == 404 -> "This file isn't on the server any more. Scan the library, then try again."
                    status == 401 -> "Your session has ended. Sign in again."
                    status != null -> "The server couldn't stream this ($status)."
                    else -> when (e.errorCode) {
                        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
                        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
                        -> "Lost the connection to the server."
                        PlaybackException.ERROR_CODE_DECODING_FAILED,
                        PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
                        PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
                        -> "This TV couldn't decode this file."
                        else -> "Playback stopped (${e.errorCodeName})."
                    }
                }
            }
        }
        player.addListener(listener)
        onDispose {
            report()
            player.removeListener(listener)
            player.release()
        }
    }

    // The clock on screen, and progress for the server every few seconds.
    LaunchedEffect(player) {
        var sinceReport = 0L
        while (true) {
            position = player.currentPosition
            if (player.duration > 0) duration = player.duration
            delay(500)
            if (player.isPlaying) {
                sinceReport += 500
                if (sinceReport >= REPORT_EVERY_MS) {
                    sinceReport = 0
                    report()
                }
            }
        }
    }

    // Controls fade while playing; they stay while paused, buffering or a menu is open.
    LaunchedEffect(touched, playing, menu) {
        if (!playing || menu != Menu.None) return@LaunchedEffect
        delay(HIDE_AFTER_MS)
        controls = false
    }
    LaunchedEffect(controls) {
        runCatching { if (controls) playFocus.requestFocus() else rootFocus.requestFocus() }
    }

    // In a party, play, pause and jumps go to the party, and come back for everyone at once.
    fun seekTo(target: Long) {
        val to = target.coerceIn(0, (player.duration.takeIf { it > 0 } ?: duration).coerceAtLeast(0))
        if (party.active) party.seek(to) else player.seekTo(to)
        position = to
        show()
    }

    fun seekBy(delta: Long) = seekTo(player.currentPosition + delta)

    fun play() {
        if (party.active) {
            party.play(if (player.playbackState == Player.STATE_ENDED) 0 else player.currentPosition)
        } else {
            if (player.playbackState == Player.STATE_ENDED) player.seekTo(0)
            player.play()
        }
        show()
    }

    fun pause() {
        if (party.active) party.pause(player.currentPosition) else player.pause()
        show()
    }

    fun toggle() = if (player.playWhenReady && player.playbackState != Player.STATE_ENDED) pause() else play()

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(rootFocus)
            .focusable()
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (event.key.nativeKeyCode) {
                    AndroidKeyEvent.KEYCODE_BACK, AndroidKeyEvent.KEYCODE_ESCAPE -> when {
                        menu != Menu.None -> { menu = Menu.None; show(); true }
                        controls && playing -> { controls = false; true }
                        else -> false
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, AndroidKeyEvent.KEYCODE_SPACE -> { toggle(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY -> { play(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_PAUSE -> { pause(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> { seekBy(SKIP_MS * 3); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_REWIND -> { seekBy(-SKIP_MS * 3); true }
                    AndroidKeyEvent.KEYCODE_CAPTIONS -> { menu = Menu.Subtitles; show(); true }
                    AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> if (!controls) { toggle(); true } else { touched++; false }
                    AndroidKeyEvent.KEYCODE_DPAD_LEFT -> if (!controls && menu == Menu.None) { seekBy(-SKIP_MS); true } else { touched++; false }
                    AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (!controls && menu == Menu.None) { seekBy(SKIP_MS); true } else { touched++; false }
                    AndroidKeyEvent.KEYCODE_DPAD_UP, AndroidKeyEvent.KEYCODE_DPAD_DOWN -> if (!controls) { show(); true } else { touched++; false }
                    else -> false
                }
            },
    ) {
        AndroidView(
            factory = { ctx ->
                PlayerView(ctx).apply {
                    useController = false
                    resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                    keepScreenOn = true
                    isFocusable = false
                    setShutterBackgroundColor(android.graphics.Color.BLACK)
                    this.player = player
                }
            },
            update = { view -> view.subtitleView?.applyStyle(style) },
            modifier = Modifier.fillMaxSize(),
        )

        if (buffering && error == null && party.countdown == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Loading…", style = MaterialTheme.typography.titleLarge, color = Nv.textSecondary)
            }
        }

        party.countdown?.let { seconds ->
            if (error == null) Countdown(seconds)
        }

        AnimatedVisibility(controls && error == null, enter = fadeIn(), exit = fadeOut()) {
            Controls(
                playable = playable,
                playing = playing,
                position = position,
                duration = duration,
                playFocus = playFocus,
                together = if (party.active) party.others.map { it.displayName } else emptyList(),
                hasSubtitles = subtitles.isNotEmpty(),
                hasAudioChoice = tracks.groups.count { it.type == C.TRACK_TYPE_AUDIO } > 1,
                onToggle = ::toggle,
                onBack = { seekBy(-SKIP_MS) },
                onForward = { seekBy(SKIP_MS) },
                onSubtitles = { menu = Menu.Subtitles },
                onAudio = { menu = Menu.Audio },
                onNext = playable.next?.let { { playNext(it) } },
                onAnyKey = { touched++ },
            )
        }

        AnimatedVisibility(party.note != null, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
            Note(party.note.orEmpty())
        }

        when (menu) {
            Menu.Subtitles -> SubtitleMenu(
                tracks = subtitles,
                selected = shownSubtitle,
                style = style,
                onPick = { track ->
                    player.showSubtitle(track)
                    user?.let { store.saveSubtitleChoice(it.id, playable.scope, track?.option?.remembered() ?: RememberedSubtitle(off = true)) }
                    menu = Menu.None
                    show()
                },
                onStyle = {
                    style = it
                    store.subtitleStyle = it
                },
            )
            Menu.Audio -> AudioMenu(tracks, player) { menu = Menu.None; show() }
            Menu.None -> {}
        }

        val next = playable.next
        if (ended && next != null && error == null) {
            UpNext(next, onPlay = { playNext(next) }, onCancel = { nav.popBackStack() })
        }

        error?.let { message ->
            Box(Modifier.fillMaxSize().background(Nv.canvas.copy(alpha = 0.92f))) {
                ErrorState("Playback stopped", message, "Back") { nav.popBackStack() }
            }
        }
    }
}

/** Before a shared start: everyone's player holds, counting down together. */
@Composable
private fun Countdown(seconds: Int) {
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.55f)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text("STARTING TOGETHER", style = MaterialTheme.typography.labelSmall, color = Nv.accent)
            Spacer(Modifier.height(8.dp))
            Text(seconds.toString(), style = MaterialTheme.typography.displaySmall.copy(fontSize = 96.sp, lineHeight = 100.sp), color = Nv.textPrimary)
        }
    }
}

/** What someone else in the party just did. */
@Composable
private fun Note(text: String) {
    Box(
        Modifier.padding(top = 36.dp).clip(RoundedCornerShape(50)).background(Nv.surface.copy(alpha = 0.92f)).padding(horizontal = 22.dp, vertical = 10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(NvIcons.Users, null, tint = Nv.accent, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(10.dp))
            Text(text, style = MaterialTheme.typography.titleSmall, color = Nv.textPrimary)
        }
    }
}

@Composable
private fun Controls(
    playable: Playable,
    playing: Boolean,
    position: Long,
    duration: Long,
    playFocus: FocusRequester,
    together: List<String>,
    hasSubtitles: Boolean,
    hasAudioChoice: Boolean,
    onToggle: () -> Unit,
    onBack: () -> Unit,
    onForward: () -> Unit,
    onSubtitles: () -> Unit,
    onAudio: () -> Unit,
    onNext: (() -> Unit)?,
    onAnyKey: () -> Unit,
) {
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxWidth().height(180.dp).background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.75f), Color.Transparent))))
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(260.dp).background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.85f)))))
        if (together.isNotEmpty()) {
            Row(
                Modifier.align(Alignment.TopEnd).padding(top = 44.dp, end = 56.dp).clip(RoundedCornerShape(50)).background(Color.White.copy(alpha = 0.12f)).padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(NvIcons.Users, null, tint = Nv.accent, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(
                    "With ${together.first()}" + if (together.size > 1) " +${together.size - 1}" else "",
                    style = MaterialTheme.typography.labelLarge,
                    color = Nv.textPrimary,
                )
            }
        }
        Column(Modifier.padding(start = 56.dp, top = 40.dp, end = 280.dp)) {
            Text(playable.title, style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            playable.subtitle?.let { Text(it, style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 56.dp, vertical = 40.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(clockText(position), style = MaterialTheme.typography.titleSmall, color = Nv.textPrimary)
                Spacer(Modifier.width(16.dp))
                Box(Modifier.weight(1f).height(6.dp).clip(RoundedCornerShape(3.dp)).background(Color.White.copy(alpha = 0.25f))) {
                    val fraction = if (duration > 0) (position.toFloat() / duration).coerceIn(0f, 1f) else 0f
                    Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(Nv.accent))
                }
                Spacer(Modifier.width(16.dp))
                Text("−" + clockText((duration - position).coerceAtLeast(0)), style = MaterialTheme.typography.titleSmall, color = Nv.textSecondary)
            }
            Spacer(Modifier.height(22.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                RoundButton(NvIcons.Rewind, "Back 10 seconds", onBack, onAnyKey)
                RoundButton(if (playing) NvIcons.Pause else NvIcons.Play, if (playing) "Pause" else "Play", onToggle, onAnyKey, Modifier.focusRequester(playFocus), big = true)
                RoundButton(NvIcons.Forward, "Forward 10 seconds", onForward, onAnyKey)
                Spacer(Modifier.weight(1f))
                if (hasSubtitles) NvButton("Subtitles", onSubtitles, icon = NvIcons.Subtitles)
                if (hasAudioChoice) NvButton("Audio", onAudio, icon = NvIcons.Audio)
                onNext?.let { NvButton("Next episode", it, icon = NvIcons.SkipNext) }
            }
        }
    }
}

@Composable
private fun RoundButton(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit, onAnyKey: () -> Unit, modifier: Modifier = Modifier, big: Boolean = false) {
    Surface(
        onClick = { onAnyKey(); onClick() },
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(50)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = Color.White.copy(alpha = 0.12f),
            contentColor = Nv.textPrimary,
            focusedContainerColor = Nv.textPrimary,
            focusedContentColor = Nv.canvas,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.1f),
        modifier = modifier.size(if (big) 72.dp else 56.dp),
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Icon(icon, contentDescription = label, modifier = Modifier.size(if (big) 32.dp else 24.dp))
        }
    }
}

/** At the end of an episode: the next one in 10 seconds, or now. */
@Composable
private fun UpNext(next: EpisodeRef, onPlay: () -> Unit, onCancel: () -> Unit) {
    var seconds by remember(next.id) { mutableIntStateOf(10) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(next.id) {
        runCatching { focus.requestFocus() }
        while (seconds > 0) {
            delay(1000)
            seconds--
        }
        onPlay()
    }
    Box(Modifier.fillMaxSize().background(Nv.canvas.copy(alpha = 0.85f)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text("UP NEXT IN $seconds", style = MaterialTheme.typography.labelSmall, color = Nv.accent)
            Spacer(Modifier.height(10.dp))
            Text(next.label, style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary)
            Text(next.title, style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary)
            Spacer(Modifier.height(26.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                NvButton("Play now", onPlay, Modifier.focusRequester(focus), icon = NvIcons.Play, primary = true)
                NvButton("Back", onCancel)
            }
        }
    }
}
