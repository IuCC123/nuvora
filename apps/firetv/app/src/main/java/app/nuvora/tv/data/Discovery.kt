package app.nuvora.tv.data

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import java.net.Inet4Address
import java.net.InetAddress
import kotlin.coroutines.resume

/** A Nuvora server announcing itself on the local network (the server's `discovery.rs`). */
data class FoundServer(val id: String, val name: String, val url: String)

private const val SERVICE_TYPE = "_nuvora._tcp"
private const val LOOK_FOR_MOVED_MS = 10_000L
private const val RESOLVE_MS = 5_000L

/**
 * One resolved announcement as a server to offer, or null when it can't be
 * used: no id, or no IPv4 address to reach it at.
 */
fun foundServer(attributes: Map<String, ByteArray?>, host: InetAddress?, port: Int): FoundServer? {
    val text = { key: String -> attributes[key]?.toString(Charsets.UTF_8)?.trim()?.takeIf { it.isNotEmpty() } }
    val id = text("id") ?: return null
    if (host !is Inet4Address || port !in 1..65535) return null
    return FoundServer(id, text("name") ?: "Nuvora", "http://${host.hostAddress}:$port")
}

/** What discovery reports, handled in order so the list has one writer. */
private sealed interface Seen {
    data class Found(val info: NsdServiceInfo) : Seen
    data class Lost(val serviceName: String) : Seen
}

/**
 * The Nuvora servers on the network, updated as they come and go, for as
 * long as the flow is collected. Announcements are resolved one at a time:
 * older Android versions refuse a second resolve while one runs.
 */
fun discoverServers(context: Context): Flow<List<FoundServer>> = callbackFlow {
    val nsd = context.getSystemService(NsdManager::class.java) ?: run {
        close()
        return@callbackFlow
    }
    val seen = Channel<Seen>(Channel.UNLIMITED)
    val listener = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) {
            seen.trySend(Seen.Found(info))
        }

        override fun onServiceLost(info: NsdServiceInfo) {
            seen.trySend(Seen.Lost(info.serviceName))
        }

        override fun onDiscoveryStarted(serviceType: String) {}
        override fun onDiscoveryStopped(serviceType: String) {}
        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
            close()
        }
        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
    }

    launch {
        val servers = linkedMapOf<String, FoundServer>()
        for (event in seen) {
            val changed = when (event) {
                is Seen.Found -> {
                    val server = resolve(nsd, event.info)
                    if (server != null) servers[event.info.serviceName] = server
                    server != null
                }
                is Seen.Lost -> servers.remove(event.serviceName) != null
            }
            if (changed) trySend(servers.values.toList())
        }
    }
    trySend(emptyList())
    nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
    awaitClose {
        runCatching { nsd.stopServiceDiscovery(listener) }
        seen.close()
    }
}

@Suppress("DEPRECATION") // resolveService is the one way that works on every Fire OS version.
private suspend fun resolve(nsd: NsdManager, info: NsdServiceInfo): FoundServer? = withTimeoutOrNull(RESOLVE_MS) {
    suspendCancellableCoroutine { done ->
        nsd.resolveService(info, object : NsdManager.ResolveListener {
            override fun onServiceResolved(resolved: NsdServiceInfo) {
                done.resume(foundServer(resolved.attributes, resolved.host, resolved.port))
            }

            override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
                done.resume(null)
            }
        })
    }
}

/**
 * Keeps the signed-in server reachable. A server saved before servers had ids
 * learns its id. When the saved address no longer answers as this server,
 * looks for the same server on the network and moves to its new address.
 */
suspend fun SessionStore.checkServer(context: Context) {
    val server = session.value?.server ?: return
    val answer = runCatching { guest(server.url).authState() }.getOrNull()
    if (answer != null && (server.id == null || answer.serverId == server.id)) {
        if (server.id == null && answer.serverId.isNotEmpty()) rememberServerId(answer.serverId)
        return
    }
    val id = server.id ?: return
    val found = withTimeoutOrNull(LOOK_FOR_MOVED_MS) {
        discoverServers(context).mapNotNull { list -> list.firstOrNull { it.id == id } }.first()
    } ?: return
    val confirmed = runCatching { guest(found.url).authState() }.getOrNull()
    if (confirmed?.serverId == id) moveServer(found.url)
}
