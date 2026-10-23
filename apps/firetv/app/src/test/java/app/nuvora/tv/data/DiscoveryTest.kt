package app.nuvora.tv.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.net.InetAddress

class DiscoveryTest {
    private fun txt(vararg pairs: Pair<String, String>) = pairs.associate { (k, v) -> k to v.toByteArray() }

    private val lan = InetAddress.getByName("192.168.1.20")

    @Test fun `an announcement becomes a server to connect to`() {
        assertEquals(
            FoundServer("58555d33a1010df93667f2cdd7561528", "Living room", "http://192.168.1.20:8640"),
            foundServer(txt("id" to "58555d33a1010df93667f2cdd7561528", "name" to "Living room", "version" to "0.3.0"), lan, 8640),
        )
    }

    @Test fun `a server without a name is called Nuvora`() {
        assertEquals("Nuvora", foundServer(txt("id" to "abc"), lan, 8640)?.name)
        assertEquals("Nuvora", foundServer(mapOf("id" to "abc".toByteArray(), "name" to null), lan, 8640)?.name)
    }

    @Test fun `no id, no IPv4 address or no port means it can't be used`() {
        assertNull(foundServer(txt("name" to "Living room"), lan, 8640))
        assertNull(foundServer(txt("id" to " "), lan, 8640))
        assertNull(foundServer(txt("id" to "abc"), InetAddress.getByName("fe80::1"), 8640))
        assertNull(foundServer(txt("id" to "abc"), null, 8640))
        assertNull(foundServer(txt("id" to "abc"), lan, 0))
    }
}
