package app.nuvora.tv.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PreviewsTest {
    private val info = PreviewInfo(
        intervalMs = 10_000,
        width = 320,
        height = 180,
        columns = 10,
        rows = 10,
        count = 250,
        sheets = listOf("/api/previews/k/0", "/api/previews/k/1", "/api/previews/k/2"),
    )

    @Test fun `a moment finds its sheet and tile`() {
        assertEquals(PreviewTile("/api/previews/k/0", 640, 180), info.tileAt(125_000))
        assertEquals(PreviewTile("/api/previews/k/1", 0, 0), info.tileAt(1_000_000))
    }

    @Test fun `before the start is the first preview, past the end the last`() {
        assertEquals(PreviewTile("/api/previews/k/0", 0, 0), info.tileAt(-5_000))
        assertEquals(PreviewTile("/api/previews/k/2", 2880, 720), info.tileAt(99_999_000))
    }

    @Test fun `a sheet the server didn't list gives nothing`() {
        assertNull(info.copy(sheets = emptyList()).tileAt(5_000))
    }
}
