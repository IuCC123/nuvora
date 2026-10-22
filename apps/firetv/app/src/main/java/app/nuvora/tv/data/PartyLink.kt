package app.nuvora.tv.data

import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import kotlin.math.abs
import kotlin.math.max

// Watch Together's shared clock, as in packages/core/src/party.ts: the server
// describes playback as "at server time at_ms the position is position_ms,
// playing or paused", and every player works out where it should be.

/** Where the party is, in ms, at server time [serverNowMs]. Before `at_ms` (a countdown) it is `position_ms`. */
fun PartyPlayback.positionAt(serverNowMs: Long): Long =
    positionMs + if (playing) max(0, serverNowMs - atMs) else 0

/** How a player should follow the party. */
sealed interface PartyCorrection {
    data object InSync : PartyCorrection
    /** A little off: play slightly faster or slower until caught up. */
    data class Nudge(val rate: Float) : PartyCorrection
    /** Too far off to nudge: jump. */
    data class Seek(val toMs: Long) : PartyCorrection
}

/** Under this many ms apart counts as together. */
private const val IN_SYNC_MS = 250
/** Past this many ms apart, jump instead of nudging. */
private const val SEEK_PAST_MS = 2_000

fun partyCorrection(currentMs: Long, targetMs: Long): PartyCorrection {
    val ahead = currentMs - targetMs
    return when {
        abs(ahead) <= IN_SYNC_MS -> PartyCorrection.InSync
        abs(ahead) > SEEK_PAST_MS -> PartyCorrection.Seek(targetMs)
        else -> PartyCorrection.Nudge(if (ahead > 0) 0.95f else 1.05f)
    }
}

/**
 * The server's clock, estimated from ping round trips: the offset from the
 * fastest round trip seen. Local times are [SystemClock.elapsedRealtime], so
 * a change to the TV's wall clock doesn't move it.
 */
class ServerClock {
    private var best = Long.MAX_VALUE
    @Volatile private var offset = 0L

    fun sample(sent: Long, serverTime: Long, received: Long) {
        val rtt = received - sent
        if (rtt < 0 || rtt > best) return
        best = rtt
        offset = serverTime - (sent + rtt / 2)
    }

    /** A server time with no round trip to go by (the first `hello`). */
    fun seed(serverTime: Long, received: Long) {
        if (best == Long.MAX_VALUE) offset = serverTime - received
    }

    fun now(): Long = SystemClock.elapsedRealtime() + offset
}

/** What the live connection knows. */
data class PartySnapshot(
    /** The live connection is up. */
    val connected: Boolean = false,
    val party: PartyView? = null,
    /** Invitations to me, oldest first. */
    val invites: List<PartyInvite> = emptyList(),
)

/** A frame from `/api/live`, after the snapshot has taken it in. */
sealed interface LiveEvent {
    /** [started]: someone just started something for everyone (a new `start`, not one already seen). */
    data class Party(val party: PartyView, val started: Boolean) : LiveEvent
    data class Left(val partyId: String) : LiveEvent
    data class Invites(val invites: List<PartyInvite>) : LiveEvent
}

/** Reconnect delays, in ms; the last repeats. */
private val BACKOFF = listOf(1_000L, 2_000L, 5_000L, 10_000L)
private const val PING_EVERY_MS = 20_000L

/**
 * The live connection to `/api/live` while the app is open and signed in:
 * Watch Together's party and invitations, the commands that go back, and
 * the server's clock. Reconnects on its own.
 */
class PartyLink {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _state = MutableStateFlow(PartySnapshot())
    val state: StateFlow<PartySnapshot> = _state
    private val _events = MutableSharedFlow<LiveEvent>(extraBufferCapacity = 32)
    val events: SharedFlow<LiveEvent> = _events

    /** The movie or episode file the player has open, as (item, file), so a start for it doesn't reopen it. */
    val playing = MutableStateFlow<Pair<Long, Long>?>(null)

    private var api: NuvoraApi? = null
    private var socket: WebSocket? = null
    private var attempts = 0
    private var reconnect: Job? = null
    private var pinger: Job? = null
    private var clock = ServerClock()

    /** Connects with [api]'s session; a different session replaces the old connection. */
    fun start(api: NuvoraApi) {
        if (this.api === api) return
        stop()
        this.api = api
        clock = ServerClock()
        open()
    }

    fun stop() {
        api = null
        reconnect?.cancel()
        pinger?.cancel()
        socket?.close(1000, null)
        socket = null
        _state.value = PartySnapshot()
    }

    fun send(command: JsonObject) {
        socket?.send(command.toString())
    }

    fun sendPlayback(type: String, positionMs: Long) =
        send(JsonObject(mapOf("type" to JsonPrimitive(type), "position_ms" to JsonPrimitive(positionMs.coerceAtLeast(0)))))

    fun sendStart(itemId: Long, fileId: Long, positionMs: Long) = send(
        JsonObject(
            mapOf(
                "type" to JsonPrimitive("start"),
                "item_id" to JsonPrimitive(itemId),
                "file_id" to JsonPrimitive(fileId),
                "position_ms" to JsonPrimitive(positionMs.coerceAtLeast(0)),
            ),
        ),
    )

    /** The server's clock, in unix ms. */
    fun serverNow(): Long = clock.now()

    /** An answer from the REST calls (join, invite), without waiting for its live echo. */
    fun setParty(party: PartyView?) = _state.update { it.copy(party = party) }

    fun setInvites(invites: List<PartyInvite>) = _state.update { it.copy(invites = invites) }

    private fun open() {
        val api = api ?: return
        val request = api.liveRequest()
        socket = api.http.newWebSocket(request, Listener(api))
    }

    private inner class Listener(private val owner: NuvoraApi) : WebSocketListener() {
        private fun mine(webSocket: WebSocket) = socket === webSocket && api === owner

        override fun onOpen(webSocket: WebSocket, response: Response) {
            scope.launch {
                if (!mine(webSocket)) return@launch
                attempts = 0
                pinger?.cancel()
                pinger = scope.launch {
                    while (isActive) {
                        ping()
                        delay(PING_EVERY_MS)
                    }
                }
            }
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            val received = SystemClock.elapsedRealtime()
            scope.launch { if (mine(webSocket)) handle(text, received) }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = lost(webSocket)

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = lost(webSocket)

        private fun lost(webSocket: WebSocket) {
            scope.launch {
                if (!mine(webSocket)) return@launch
                socket = null
                pinger?.cancel()
                _state.update { it.copy(connected = false) }
                val delayMs = BACKOFF[minOf(attempts, BACKOFF.size - 1)]
                attempts++
                reconnect = scope.launch {
                    delay(delayMs)
                    if (api === owner) open()
                }
            }
        }
    }

    private fun ping() {
        send(JsonObject(mapOf("type" to JsonPrimitive("ping"), "client_time_ms" to JsonPrimitive(SystemClock.elapsedRealtime()))))
    }

    private fun handle(text: String, received: Long) {
        val frame = runCatching { json.parseToJsonElement(text).jsonObject }.getOrNull() ?: return
        val event: LiveEvent? = runCatching {
            when (frame["type"]?.jsonPrimitive?.content) {
                "hello" -> {
                    clock.seed(frame["server_time_ms"]!!.jsonPrimitive.long, received)
                    val status = json.decodeFromJsonElement(PartyStatus.serializer(), frame["status"]!!)
                    _state.value = PartySnapshot(connected = true, party = status.party, invites = status.invites)
                    // Where things stand, not a change: nothing to follow.
                    null
                }
                "pong" -> {
                    val sent = frame["client_time_ms"]!!.jsonPrimitive.content.toDouble().toLong()
                    clock.sample(sent, frame["server_time_ms"]!!.jsonPrimitive.long, received)
                    null
                }
                "party" -> {
                    val party = json.decodeFromJsonElement(PartyView.serializer(), frame["party"]!!)
                    val before = _state.value.party?.takeIf { it.id == party.id }?.playback?.seq
                    _state.update { it.copy(party = party) }
                    val playback = party.playback
                    LiveEvent.Party(party, started = playback != null && playback.action == "start" && playback.seq != before)
                }
                "party_left" -> {
                    val id = frame["party_id"]!!.jsonPrimitive.content
                    _state.update { if (it.party?.id == id) it.copy(party = null) else it }
                    LiveEvent.Left(id)
                }
                "invites" -> {
                    val invites = json.decodeFromJsonElement(
                        kotlinx.serialization.builtins.ListSerializer(PartyInvite.serializer()),
                        frame["invites"]!!,
                    )
                    _state.update { it.copy(invites = invites) }
                    LiveEvent.Invites(invites)
                }
                else -> null // A frame this build doesn't understand.
            }
        }.getOrNull()
        if (event != null) _events.tryEmit(event)
    }
}
