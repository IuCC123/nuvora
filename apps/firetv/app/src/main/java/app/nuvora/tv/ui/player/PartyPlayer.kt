package app.nuvora.tv.ui.player

import android.os.SystemClock
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import app.nuvora.tv.data.PartyCorrection
import app.nuvora.tv.data.PartyLink
import app.nuvora.tv.data.PartyMember
import app.nuvora.tv.data.PartyPlayback
import app.nuvora.tv.data.partyCorrection
import app.nuvora.tv.data.positionAt
import app.nuvora.tv.ui.details.clockText
import kotlinx.coroutines.delay
import kotlin.math.abs
import kotlin.math.ceil

/** How often a playing video is checked against the party's clock. */
private const val CHECK_MS = 500L
private const val NOTE_MS = 3_000L

/** The player's side of Watch Together. */
@Stable
class PartyPlayer(private val link: PartyLink) {
    /** In a party, and it is playing this file: the party's clock drives the video. */
    var active by mutableStateOf(false)
        internal set
    /** Everyone else in the party. */
    var others by mutableStateOf(emptyList<PartyMember>())
        internal set
    /** Whole seconds until everyone starts together, while counting down. */
    var countdown by mutableStateOf<Int?>(null)
        internal set
    /** What someone else just did: `"Bo paused"`. */
    var note by mutableStateOf<String?>(null)
        internal set

    fun play(positionMs: Long) = link.sendPlayback("play", positionMs)
    fun pause(positionMs: Long) = link.sendPlayback("pause", positionMs)
    fun seek(positionMs: Long) = link.sendPlayback("seek", positionMs)
}

/**
 * Keeps [player] with the party. Opening the player while in a party starts
 * this file for everyone; after that every play, pause and seek goes through
 * the party and comes back for everyone (this player included) at the same
 * moment. Small drift is closed by playing a little faster or slower, large
 * drift by jumping.
 */
@Composable
fun rememberPartyPlayer(link: PartyLink, player: ExoPlayer, me: Long?, itemId: Long, fileId: Long, startMs: Long, converted: Boolean): PartyPlayer {
    val state = remember(link) { PartyPlayer(link) }
    val snapshot by link.state.collectAsState()
    val party = snapshot.party
    val playback = party?.playback
    // Without the live connection the player is on its own: it can't hear the party, nor tell it anything.
    val active = snapshot.connected && playback != null && playback.itemId == itemId && playback.fileId == fileId
    val inParty = snapshot.connected && party != null
    state.active = active
    state.others = party?.members?.filter { it.id != me }.orEmpty()
    val latest by rememberUpdatedState(if (active) playback else null)

    // The app knows what's open, so a start for this file doesn't open it again.
    DisposableEffect(link, itemId, fileId) {
        val open = itemId to fileId
        link.playing.value = open
        onDispose { if (link.playing.value == open) link.playing.value = null }
    }

    // Opened while in a party on something else: this is what the party watches now.
    // Decided once, on opening: when the party later moves on, this player is the one leaving.
    val starts = remember(link) { inParty && !active }
    LaunchedEffect(starts) { if (starts) link.sendStart(itemId, fileId, startMs) }

    // A jump while playing lands this far ahead, the time the player took to load after the last one.
    val sync = remember(player) { Sync(if (converted) 1_500L else 300L) }

    fun apply() {
        val p = latest ?: return
        if (player.playbackState == Player.STATE_IDLE) return
        val now = link.serverNow()
        val duration = player.duration.takeIf { it > 0 }
        val target = p.positionAt(now).let { if (duration != null) minOf(it, duration) else it }
        val current = player.currentPosition
        if (p.playing && now < p.atMs) {
            // Counting down to a shared start: hold at the start position.
            player.playWhenReady = false
            if (abs(current - target) > 300) player.seekTo(target)
            // A new start counts down on screen; a resume or jump just holds for its short lead.
            state.countdown = if (p.action == "start") ceil((p.atMs - now) / 1000.0).toInt() else null
            return
        }
        state.countdown = null
        if (!p.playing) {
            player.setPlaybackSpeed(1f)
            player.playWhenReady = false
            if (abs(current - target) > 500) player.seekTo(target)
            return
        }
        when (player.playbackState) {
            // Still loading after a jump: let it catch up before judging the drift.
            Player.STATE_BUFFERING -> {
                player.playWhenReady = true
                return
            }
            Player.STATE_ENDED -> return
        }
        when (val correction = partyCorrection(current, target)) {
            is PartyCorrection.Seek -> {
                player.setPlaybackSpeed(1f)
                player.seekTo(correction.toMs + sync.lead)
                sync.seekedAt = SystemClock.elapsedRealtime()
            }
            is PartyCorrection.Nudge -> player.setPlaybackSpeed(correction.rate)
            PartyCorrection.InSync -> player.setPlaybackSpeed(1f)
        }
        player.playWhenReady = true
    }

    DisposableEffect(player) {
        val listener = object : Player.Listener {
            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState != Player.STATE_READY) return
                sync.seekedAt?.let { sync.lead = (SystemClock.elapsedRealtime() - it).coerceIn(0, 4_000) }
                sync.seekedAt = null
                apply()
            }
        }
        player.addListener(listener)
        onDispose { player.removeListener(listener) }
    }

    // Every change as it arrives, then on a steady beat.
    LaunchedEffect(active, playback?.seq) {
        if (!active) {
            // Out of the party: back to normal speed.
            player.setPlaybackSpeed(1f)
            state.countdown = null
            return@LaunchedEffect
        }
        while (true) {
            apply()
            delay(CHECK_MS)
        }
    }

    // Say what someone else did.
    LaunchedEffect(playback?.seq) {
        val p = latest ?: return@LaunchedEffect
        val who = p.by?.takeIf { it.id != me }?.displayName ?: return@LaunchedEffect
        state.note = describe(who, p)
        delay(NOTE_MS)
        state.note = null
    }
    return state
}

private class Sync(var lead: Long) {
    var seekedAt: Long? = null
}

private fun describe(who: String, p: PartyPlayback) = when (p.action) {
    "pause" -> "$who paused"
    "play" -> "$who pressed play"
    "seek" -> "$who jumped to ${clockText(p.positionMs)}"
    else -> "$who started this"
}
