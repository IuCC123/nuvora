package app.nuvora.tv.data

import org.junit.Assert.assertEquals
import org.junit.Test

class ServerAddressTest {
    @Test fun `a bare host tries Nuvora's port first`() {
        assertEquals(
            listOf("http://192.168.1.20:8640", "https://192.168.1.20", "http://192.168.1.20"),
            serverCandidates(" 192.168.1.20 "),
        )
    }

    @Test fun `http without a port still tries Nuvora's port first`() {
        assertEquals(listOf("http://85.11.167.17:8640", "http://85.11.167.17"), serverCandidates("http://85.11.167.17"))
        assertEquals(listOf("http://nas.local:8640", "http://nas.local/web"), serverCandidates("Http://nas.local/web/"))
    }

    @Test fun `a port or https is taken as typed`() {
        assertEquals(listOf("http://nas.local:9000", "https://nas.local:9000"), serverCandidates("nas.local:9000"))
        assertEquals(listOf("https://media.example.com"), serverCandidates("https://media.example.com/"))
        assertEquals(listOf("http://box:8640/web"), serverCandidates("HTTP://Box:8640/web"))
        assertEquals(emptyList<String>(), serverCandidates("   "))
    }
}
