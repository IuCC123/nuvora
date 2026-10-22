package app.nuvora.tv.data

import org.junit.Assert.assertEquals
import org.junit.Test

class PartyClockTest {
    private fun playback(playing: Boolean, positionMs: Long, atMs: Long) =
        PartyPlayback(1, 2, "movie", "Film", null, playing, positionMs, atMs, seq = 1, by = null, action = "play")

    @Test fun `a playing party moves on in real time from its moment`() {
        assertEquals(65_000, playback(true, 60_000, 1_000).positionAt(6_000))
    }

    @Test fun `before a shared start it holds at the start position`() {
        assertEquals(60_000, playback(true, 60_000, 10_000).positionAt(7_500))
    }

    @Test fun `a paused party stays put`() {
        assertEquals(60_000, playback(false, 60_000, 1_000).positionAt(99_000))
    }

    @Test fun `small drift is nudged, large drift jumps`() {
        assertEquals(PartyCorrection.InSync, partyCorrection(10_100, 10_000))
        assertEquals(PartyCorrection.Nudge(0.95f), partyCorrection(11_000, 10_000))
        assertEquals(PartyCorrection.Nudge(1.05f), partyCorrection(9_000, 10_000))
        assertEquals(PartyCorrection.Seek(10_000), partyCorrection(13_000, 10_000))
    }
}
