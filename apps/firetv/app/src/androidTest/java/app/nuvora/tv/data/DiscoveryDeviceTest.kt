package app.nuvora.tv.data

import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Discovery against the device's own mDNS: a server is announced on this
 * device and must turn up in [discoverServers].
 *
 * Instrumentation arguments: `serverId` and `port` announce a real server's
 * identity (default: a made-up one on 8640); `holdMs` keeps the announcement
 * up afterwards, so the app can be watched following a server that moved.
 */
@RunWith(AndroidJUnit4::class)
class DiscoveryDeviceTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val args = InstrumentationRegistry.getArguments()

    @Test
    fun a_server_announced_on_the_network_is_found() = runBlocking {
        val id = args.getString("serverId") ?: "0123456789abcdef0123456789abcdef"
        val port = args.getString("port")?.toInt() ?: 8640
        val nsd = context.getSystemService(NsdManager::class.java)
        val registered = CompletableDeferred<Unit>()
        val registration = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                registered.complete(Unit)
            }
            override fun onRegistrationFailed(info: NsdServiceInfo, errorCode: Int) {
                registered.completeExceptionally(IllegalStateException("registration failed: $errorCode"))
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {}
            override fun onUnregistrationFailed(info: NsdServiceInfo, errorCode: Int) {}
        }
        val info = NsdServiceInfo().apply {
            serviceName = "nuvora-${id.take(12)}"
            serviceType = "_nuvora._tcp"
            setPort(port)
            setAttribute("id", id)
            setAttribute("name", "Living room")
        }
        nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, registration)
        try {
            withTimeout(10_000) { registered.await() }
            val found = withTimeout(20_000) { discoverServers(context).first { list -> list.any { it.id == id } } }
            val server = found.single { it.id == id }
            assertEquals("Living room", server.name)
            assertTrue(server.url, server.url.matches(Regex("""http://\d+\.\d+\.\d+\.\d+:$port""")))
            delay(args.getString("holdMs")?.toLong() ?: 0)
        } finally {
            nsd.unregisterService(registration)
        }
    }
}
