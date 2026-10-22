package app.nuvora.tv.ui.player

import android.content.Context
import android.graphics.Color
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView
import app.nuvora.tv.data.NuvoraApi
import app.nuvora.tv.data.SubtitleOption
import app.nuvora.tv.data.SubtitleStyle
import app.nuvora.tv.data.Version
import app.nuvora.tv.data.languageCode
import kotlin.random.Random

const val SKIP_MS = 10_000L

/**
 * An ExoPlayer for [version], at [startMs]. Converted streams get a session
 * id (one ffmpeg run per player) and start where playback starts; subtitles
 * the stream doesn't carry are added from the server. Subtitles start off:
 * the player picks a track once it sees them all.
 */
@OptIn(UnstableApi::class)
fun buildPlayer(context: Context, api: NuvoraApi, version: Version, startMs: Long, autoplay: Boolean): ExoPlayer {
    val headers = buildMap { api.token?.let { put("Authorization", "Bearer $it") } }
    val dataSource = OkHttpDataSource.Factory(api.http).setDefaultRequestProperties(headers)
    val renderers = DefaultRenderersFactory(context).setEnableDecoderFallback(true)
    val player = ExoPlayer.Builder(context, renderers)
        .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
        .setSeekBackIncrementMs(SKIP_MS)
        .setSeekForwardIncrementMs(SKIP_MS)
        .build()

    val playback = version.playback
    val hls = playback.kind == "hls"
    val uri = api.resolve(playback.url!!).let { base ->
        if (!hls) base else "$base?session=${sessionId()}&start=${startMs.coerceAtLeast(0) / 1000}"
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
            .setRoleFlags(if (track.isHearingImpaired) C.ROLE_FLAG_DESCRIBES_MUSIC_AND_SOUND else 0)
            .setId("nuvora-${track.index}")
            .build()
    }
    val item = MediaItem.Builder()
        .setUri(uri)
        .setMimeType(if (hls) MimeTypes.APPLICATION_M3U8 else null)
        .setSubtitleConfigurations(sideLoaded)
        .build()

    player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
        .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
        .build()
    player.setMediaItem(item, startMs.coerceAtLeast(0))
    player.prepare()
    player.playWhenReady = autoplay
    return player
}

/** A subtitle track ExoPlayer found, with what the chooser and the menu need to know. */
data class TextTrack(val group: Tracks.Group, val index: Int, val option: SubtitleOption)

@OptIn(UnstableApi::class)
fun textTracks(tracks: Tracks): List<TextTrack> =
    tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT }.flatMap { group ->
        (0 until group.length).filter { group.isTrackSupported(it) }.map { i -> TextTrack(group, i, option(group.getTrackFormat(i))) }
    }

@OptIn(UnstableApi::class)
private fun option(format: Format): SubtitleOption {
    val label = format.label?.trim()?.takeIf { it.isNotEmpty() }
    return SubtitleOption(
        language = languageCode(format.language),
        title = label,
        forced = format.selectionFlags and C.SELECTION_FLAG_FORCED != 0,
        isDefault = format.selectionFlags and C.SELECTION_FLAG_DEFAULT != 0,
        hearingImpaired = format.roleFlags and C.ROLE_FLAG_DESCRIBES_MUSIC_AND_SOUND != 0 || label?.contains("SDH", ignoreCase = true) == true,
        styled = format.sampleMimeType == MimeTypes.TEXT_SSA || format.codecs == MimeTypes.TEXT_SSA,
    )
}

/** Shows [track], or no subtitles for null. */
fun ExoPlayer.showSubtitle(track: TextTrack?) {
    trackSelectionParameters = trackSelectionParameters.buildUpon().apply {
        if (track == null) {
            setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
        } else {
            setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
            setOverrideForType(TrackSelectionOverride(track.group.mediaTrackGroup, track.index))
        }
    }.build()
}

/** The look chosen in the menu, over styled (ASS) subtitles' own colours and positions. */
@OptIn(UnstableApi::class)
fun SubtitleView.applyStyle(style: SubtitleStyle) {
    setApplyEmbeddedStyles(true)
    setApplyEmbeddedFontSizes(false)
    setStyle(
        CaptionStyleCompat(
            Color.WHITE,
            if (style.background == "box") 0xCC000000.toInt() else Color.TRANSPARENT,
            Color.TRANSPARENT,
            when (style.background) {
                "shadow" -> CaptionStyleCompat.EDGE_TYPE_OUTLINE
                else -> CaptionStyleCompat.EDGE_TYPE_NONE
            },
            Color.BLACK,
            null,
        ),
    )
    setFractionalTextSize(
        when (style.size) {
            "small" -> 0.045f
            "large" -> 0.07f
            else -> 0.056f
        },
    )
    setBottomPaddingFraction(0.08f)
}

private fun sessionId(): String = (1..32).map { "0123456789abcdef"[Random.nextInt(16)] }.joinToString("")
