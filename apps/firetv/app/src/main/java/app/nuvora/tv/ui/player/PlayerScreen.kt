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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.CaptionStyleCompat
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
import app.nuvora.tv.data.NuvoraApi
import app.nuvora.tv.data.ProgressRequest
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
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.Locale
import kotlin.random.Random

/** What the player plays: one movie or episode, one version of it. */
private data class Playable(
    val itemId: Long,
    val title: String,
    val subtitle: String?,
    val version: Version,
    val watch: WatchState,
    val next: EpisodeRef?,
)

@Composable
fun PlayerScreen(nav: NavHostController, itemId: Long, fileId: Long, startMs: Long, user: User?) {
    val (item) = rememberLoad(itemId) { item(itemId) }
    when (val state = item) {
        Load.Loading -> Loading()
        is Load.Failed -> ErrorState("Can't play this", state.error.message ?: "", "Back") { nav.popBackStack() }
        is Load.Ready -> {
            val playable = when (val detail = state.value) {
                is MovieDetail -> playable(detail.id, detail.title, null, detail.versions, detail.watch, null, fileId)
                is EpisodeDetail -> playable(detail.id, detail.showTitle, "${detail.label} · ${detail.title}", detail.versions, detail.watch, detail.next, fileId)
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

private fun playable(id: Long, title: String, subtitle: String?, versions: List<Version>, watch: WatchState, next: EpisodeRef?, fileId: Long): Playable? {
    val version = versions.firstOrNull { it.fileId == fileId && it.playback.playable }
        ?: versions.firstOrNull { it.fileId == watch.fileId && it.playback.playable }
        ?: versions.firstOrNull { it.playback.playable }
        ?: return null
    return Playable(id, title, subtitle, version, watch, next)
}

private const val SKIP_MS = 10_000L
private const val HIDE_AFTER_MS = 4_000L
private const val REPORT_EVERY_MS = 10_000L

private enum class Menu { None, Subtitles, Audio }

@SuppressLint("UnsafeOptInUsageError")
@OptIn(UnstableApi::class)
@Composable
private fun Player(nav: NavHostController, playable: Playable, startMs: Long, user: User?) {
    val api = LocalApi.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val player = remember(playable.itemId, playable.version.fileId) { buildPlayer(context, api, playable, startMs, user) }

    var playing by remember { mutableStateOf(true) }
    var position by remember { mutableLongStateOf(startMs) }
    var duration by remember { mutableLongStateOf(playable.version.media?.durationMs ?: 0L) }
    var buffering by remember { mutableStateOf(true) }
    var ended by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var controls by remember { mutableStateOf(true) }
    var touched by remember { mutableIntStateOf(0) }
    var menu by remember { mutableStateOf(Menu.None) }
    var tracks by remember { mutableStateOf(Tracks.EMPTY) }
    val rootFocus = remember { FocusRequester() }
    val playFocus = remember { FocusRequester() }

    fun show() {
        controls = true
        touched++
    }

    fun report() {
        val dur = player.duration.takeIf { it > 0 } ?: duration
        if (dur <= 0) return
        val body = ProgressRequest(playable.version.fileId, player.currentPosition.coerceIn(0, dur), dur)
        scope.launch { runCatching { api.reportProgress(playable.itemId, body) } }
    }

    fun playNext(next: EpisodeRef) {
        report()
        nav.navigate(PlayerRoute(next.id, 0, 0)) {
            popUpTo<PlayerRoute> { inclusive = true }
        }
    }

    DisposableEffect(player) {
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

    fun seekBy(delta: Long) {
        val target = (player.currentPosition + delta).coerceIn(0, (player.duration.takeIf { it > 0 } ?: duration).coerceAtLeast(0))
        player.seekTo(target)
        position = target
        show()
    }

    fun toggle() {
        if (player.isPlaying) player.pause() else {
            if (player.playbackState == Player.STATE_ENDED) player.seekTo(0)
            player.play()
        }
        show()
    }

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
                        menu != Menu.None -> { menu = Menu.None; true }
                        controls && playing -> { controls = false; true }
                        else -> false
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, AndroidKeyEvent.KEYCODE_SPACE -> { toggle(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY -> { player.play(); show(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_PAUSE -> { player.pause(); show(); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> { seekBy(SKIP_MS * 3); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_REWIND -> { seekBy(-SKIP_MS * 3); true }
                    AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> if (!controls) { toggle(); true } else { touched++; false }
                    AndroidKeyEvent.KEYCODE_DPAD_LEFT -> if (!controls) { seekBy(-SKIP_MS); true } else { touched++; false }
                    AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (!controls) { seekBy(SKIP_MS); true } else { touched++; false }
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
                    subtitleView?.apply {
                        setApplyEmbeddedStyles(true)
                        setStyle(
                            CaptionStyleCompat(
                                android.graphics.Color.WHITE,
                                android.graphics.Color.TRANSPARENT,
                                android.graphics.Color.TRANSPARENT,
                                CaptionStyleCompat.EDGE_TYPE_OUTLINE,
                                android.graphics.Color.BLACK,
                                null,
                            ),
                        )
                        setFractionalTextSize(0.058f)
                        setBottomPaddingFraction(0.08f)
                    }
                    this.player = player
                }
            },
            modifier = Modifier.fillMaxSize(),
        )

        if (buffering && error == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Loading…", style = MaterialTheme.typography.titleLarge, color = Nv.textSecondary)
            }
        }

        AnimatedVisibility(controls && error == null, enter = fadeIn(), exit = fadeOut()) {
            Controls(
                playable = playable,
                playing = playing,
                position = position,
                duration = duration,
                playFocus = playFocus,
                hasSubtitles = tracks.groups.any { it.type == C.TRACK_TYPE_TEXT },
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

        if (menu != Menu.None) {
            TrackMenu(
                type = if (menu == Menu.Subtitles) C.TRACK_TYPE_TEXT else C.TRACK_TYPE_AUDIO,
                tracks = tracks,
                player = player,
                onDone = { menu = Menu.None; show() },
            )
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

@Composable
private fun Controls(
    playable: Playable,
    playing: Boolean,
    position: Long,
    duration: Long,
    playFocus: FocusRequester,
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
        Column(Modifier.padding(start = 56.dp, top = 40.dp)) {
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

/** Subtitles or audio: the tracks ExoPlayer found, plus Off for subtitles. */
@OptIn(UnstableApi::class)
@Composable
private fun TrackMenu(type: Int, tracks: Tracks, player: ExoPlayer, onDone: () -> Unit) {
    val groups = tracks.groups.filter { it.type == type }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.5f))) {
        Column(
            Modifier.align(Alignment.CenterEnd).fillMaxHeight().width(420.dp).background(Nv.surface).padding(28.dp),
        ) {
            Text(if (type == C.TRACK_TYPE_TEXT) "Subtitles" else "Audio", style = MaterialTheme.typography.headlineSmall, color = Nv.textPrimary)
            Spacer(Modifier.height(18.dp))
            LazyColumn(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (type == C.TRACK_TYPE_TEXT) {
                    item {
                        val off = groups.none { it.isSelected }
                        TrackRow("Off", off, Modifier.focusRequester(first)) {
                            player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                                .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
                                .build()
                            onDone()
                        }
                    }
                }
                items(groups.flatMapIndexed { _, group -> (0 until group.length).map { group to it } }) { (group, index) ->
                    val format = group.getTrackFormat(index)
                    val selected = group.isTrackSelected(index)
                    val isFirst = type == C.TRACK_TYPE_AUDIO && group == groups.firstOrNull() && index == 0
                    TrackRow(trackLabel(format), selected, if (isFirst) Modifier.focusRequester(first) else Modifier) {
                        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                            .setTrackTypeDisabled(type, false)
                            .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index))
                            .build()
                        onDone()
                    }
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

/** `"Spanish · Signs"`, `"English · SDH"`, `"Japanese · 5.1"`. */
@OptIn(UnstableApi::class)
private fun trackLabel(format: androidx.media3.common.Format): String {
    val language = format.language?.takeIf { it != C.LANGUAGE_UNDETERMINED }?.let { Locale.forLanguageTag(it).getDisplayLanguage(Locale.getDefault()) }
        ?.replaceFirstChar { it.titlecase() }
    val extras = buildList {
        format.label?.takeIf { it.isNotBlank() && it != language }?.let(::add)
        if (format.selectionFlags and C.SELECTION_FLAG_FORCED != 0) add("Forced")
        when (format.channelCount) {
            1 -> add("Mono")
            2 -> add("Stereo")
            6 -> add("5.1")
            8 -> add("7.1")
        }
    }
    return (listOfNotNull(language ?: "Unknown language") + extras).joinToString(" · ")
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

// ------------------------------------------------------------------- setup

@OptIn(UnstableApi::class)
private fun buildPlayer(context: android.content.Context, api: NuvoraApi, playable: Playable, startMs: Long, user: User?): ExoPlayer {
    val headers = buildMap { api.token?.let { put("Authorization", "Bearer $it") } }
    val dataSource = OkHttpDataSource.Factory(api.http).setDefaultRequestProperties(headers)
    val renderers = DefaultRenderersFactory(context).setEnableDecoderFallback(true)
    val player = ExoPlayer.Builder(context, renderers)
        .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
        .setSeekBackIncrementMs(SKIP_MS)
        .setSeekForwardIncrementMs(SKIP_MS)
        .build()

    val version = playable.version
    val playback = version.playback
    val hls = playback.kind == "hls"
    val uri = api.resolve(playback.url!!).let { base ->
        if (!hls) base else "$base?session=${sessionId()}&start=${startMs / 1000}"
    }
    // Tracks inside the file reach ExoPlayer with the file when it plays directly;
    // a converted stream carries none, and files next to the video never do.
    val sideLoaded = version.media?.subtitles.orEmpty().filter { it.renderable && (hls || it.external) }.map { track ->
        val ass = track.format == "ass"
        MediaItem.SubtitleConfiguration.Builder(android.net.Uri.parse(api.subtitleUrl(version.fileId, track.index, if (ass) "ass" else "vtt")))
            .setMimeType(if (ass) MimeTypes.TEXT_SSA else MimeTypes.TEXT_VTT)
            .setLanguage(track.language)
            .setLabel(track.title)
            .setSelectionFlags((if (track.isDefault) C.SELECTION_FLAG_DEFAULT else 0) or (if (track.isForced) C.SELECTION_FLAG_FORCED else 0))
            .setId("nuvora-${track.index}")
            .build()
    }
    val item = MediaItem.Builder()
        .setUri(uri)
        .setMimeType(if (hls) MimeTypes.APPLICATION_M3U8 else null)
        .setSubtitleConfigurations(sideLoaded)
        .build()

    player.trackSelectionParameters = subtitleChoice(player.trackSelectionParameters.buildUpon(), user, version).build()
    player.setMediaItem(item, startMs.coerceAtLeast(0))
    player.prepare()
    player.playWhenReady = true
    return player
}

/**
 * The account's subtitle setting: `always` in its language, `off`, or
 * `auto`: subtitles in its language only when the audio is in another.
 */
@OptIn(UnstableApi::class)
private fun subtitleChoice(
    params: androidx.media3.common.TrackSelectionParameters.Builder,
    user: User?,
    version: Version,
): androidx.media3.common.TrackSelectionParameters.Builder {
    val language = user?.subtitleLanguage ?: Locale.getDefault().isO3Language
    val audio = version.media?.audio?.let { list -> list.firstOrNull { it.isDefault } ?: list.firstOrNull() }?.language
    val on = when (user?.subtitleMode) {
        "always" -> true
        "off" -> false
        else -> audio != null && !sameLanguage(audio, language)
    }
    return if (on) {
        params.setPreferredTextLanguage(language).setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
    } else {
        // No language preference, and default-flagged tracks ignored: only
        // forced subtitles (signs, lines in another language) still show.
        params.setPreferredTextLanguage(null)
            .setSelectUndeterminedTextLanguage(false)
            .setIgnoredTextSelectionFlags(C.SELECTION_FLAG_DEFAULT)
    }
}

private fun sameLanguage(a: String, b: String): Boolean {
    fun norm(code: String) = runCatching { Locale.forLanguageTag(code).isO3Language }.getOrDefault(code).ifEmpty { code }
    return norm(a.lowercase()) == norm(b.lowercase())
}

private fun sessionId(): String = (1..32).map { "0123456789abcdef"[Random.nextInt(16)] }.joinToString("")
