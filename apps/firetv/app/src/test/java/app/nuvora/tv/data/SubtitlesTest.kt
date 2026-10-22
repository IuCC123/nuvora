package app.nuvora.tv.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SubtitlesTest {
    private fun track(language: String?, title: String? = null, forced: Boolean = false, default: Boolean = false, sdh: Boolean = false, styled: Boolean = false) =
        SubtitleOption(languageCode(language), title, forced, default, sdh, styled)

    private val anime = listOf(
        track("eng", "Signs & Songs", forced = true),
        track("eng", "Full", default = true),
        track("spa", styled = true),
    )

    @Test fun `auto shows the viewer's language over foreign audio`() {
        assertEquals(1, chooseSubtitle(anime, "jpn", "auto", "en", null))
    }

    @Test fun `auto over the viewer's own language shows only forced lines`() {
        assertEquals(0, chooseSubtitle(anime, "eng", "auto", "eng", null))
    }

    @Test fun `auto falls back to the file's default when nothing is in the viewer's language`() {
        val tracks = listOf(track("spa"), track("fre", default = true))
        assertEquals(1, chooseSubtitle(tracks, "jpn", "auto", "eng", null))
    }

    @Test fun `off and a remembered off show nothing`() {
        assertNull(chooseSubtitle(anime, "jpn", "off", "eng", null))
        assertNull(chooseSubtitle(anime, "jpn", "always", "eng", RememberedSubtitle(off = true)))
    }

    @Test fun `a remembered choice carries to a file that orders tracks differently`() {
        val remembered = anime[0].remembered()
        val next = listOf(track("spa", styled = true), track("eng", "Full"), track("eng", "Signs & Songs", forced = true))
        assertEquals(2, chooseSubtitle(next, "jpn", "off", "eng", remembered))
    }

    @Test fun `labels skip a title that only repeats the language or a tag`() {
        assertEquals("English · Signs & Songs · Forced", anime[0].label)
        assertEquals("Spanish · Styled", track("spa", "Spanish", styled = true).label)
        assertEquals("Unknown language", track(null).label)
    }

    @Test fun `language codes in any form become the server's three letters`() {
        assertEquals("ger", languageCode("de"))
        assertEquals("ger", languageCode("deu"))
        assertEquals("ger", languageCode("de-AT"))
        assertEquals("chi", languageCode("ZHO"))
        assertNull(languageCode("und"))
        assertNull(languageCode(""))
    }
}
