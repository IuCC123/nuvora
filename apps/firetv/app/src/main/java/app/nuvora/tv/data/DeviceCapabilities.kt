package app.nuvora.tv.data

import android.content.Context
import android.media.AudioFormat
import android.media.MediaCodecInfo.CodecProfileLevel
import android.media.MediaCodecList
import android.os.Build
import android.view.Display
import android.view.WindowManager
import androidx.media3.common.AudioAttributes
import androidx.media3.exoplayer.audio.AudioCapabilities

/**
 * What this TV decodes, measured with the platform's codec list, display
 * and audio output, in the server's canonical names. The server turns it into
 * "plays directly" or "convert this part".
 */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
fun probeDeviceProfile(context: Context): DeviceProfile {
    val decoders = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.filter { !it.isEncoder }

    fun decoder(mime: String) = decoders.filter { info -> info.supportedTypes.any { it.equals(mime, ignoreCase = true) } }

    val hdr = displayHdr(context)
    val video = buildList {
        for ((codec, mime) in listOf("h264" to "video/avc", "hevc" to "video/hevc", "av1" to "video/av01", "vp9" to "video/x-vnd.on2.vp9")) {
            val found = decoder(mime)
            if (found.isEmpty()) continue
            var width = 1920
            var height = 1080
            var tenBit = false
            for (info in found) {
                val caps = info.getCapabilitiesForType(mime)
                caps.videoCapabilities?.let {
                    width = maxOf(width, it.supportedWidths.upper)
                    height = maxOf(height, it.supportedHeights.upper)
                }
                tenBit = tenBit || caps.profileLevels.any { pl ->
                    when (codec) {
                        "hevc" -> pl.profile == CodecProfileLevel.HEVCProfileMain10 || pl.profile == CodecProfileLevel.HEVCProfileMain10HDR10
                        "vp9" -> pl.profile == CodecProfileLevel.VP9Profile2 || pl.profile == CodecProfileLevel.VP9Profile2HDR
                        "av1" -> pl.profile == CodecProfileLevel.AV1ProfileMain10 || pl.profile == CodecProfileLevel.AV1ProfileMain10HDR10
                        else -> false
                    }
                }
            }
            val dolbyVision = codec == "hevc" && decoder("video/dolby-vision").isNotEmpty() && "dolby_vision" in hdr
            val formats = if (tenBit) hdr.filter { it != "dolby_vision" || dolbyVision } else emptyList()
            add(VideoSupport(codec, width.coerceAtMost(7680), height.coerceAtMost(4320), if (tenBit) 10 else 8, formats))
        }
    }

    // A one-off read at startup; the receiver Media3 now suggests reports changes over time.
    @Suppress("DEPRECATION")
    val passthrough = AudioCapabilities.getCapabilities(context, AudioAttributes.DEFAULT, null)
    val audio = buildList {
        // ExoPlayer decodes these itself or with the platform on every TV.
        add(AudioSupport("aac", 8))
        add(AudioSupport("mp3", 2))
        add(AudioSupport("opus", 8))
        add(AudioSupport("vorbis", 8))
        if (Build.VERSION.SDK_INT >= 27 || decoder("audio/flac").isNotEmpty()) add(AudioSupport("flac", 8))
        add(AudioSupport("pcm", 8))
        // Dolby and DTS: decoded on the device, or sent to the TV or receiver as they are.
        fun has(mime: String, encoding: Int) = decoder(mime).isNotEmpty() || passthrough.supportsEncoding(encoding)
        if (has("audio/ac3", AudioFormat.ENCODING_AC3)) add(AudioSupport("ac3", 6))
        if (has("audio/eac3", AudioFormat.ENCODING_E_AC3)) add(AudioSupport("eac3", 8))
        if (has("audio/vnd.dts", AudioFormat.ENCODING_DTS)) add(AudioSupport("dts", 8))
        if (Build.VERSION.SDK_INT >= 28 && passthrough.supportsEncoding(AudioFormat.ENCODING_DOLBY_TRUEHD)) add(AudioSupport("truehd", 8))
    }

    return DeviceProfile(
        // ExoPlayer reads all of these itself.
        containers = listOf("mp4", "mov", "mkv", "webm", "ts", "avi"),
        video = video,
        audio = audio,
    )
}

/** HDR formats the connected screen shows, in the server's names. */
@Suppress("DEPRECATION")
private fun displayHdr(context: Context): List<String> {
    val display: Display = (context.getSystemService(Context.WINDOW_SERVICE) as WindowManager).defaultDisplay
    val types = display.hdrCapabilities?.supportedHdrTypes ?: return emptyList()
    return buildList {
        if (Display.HdrCapabilities.HDR_TYPE_HDR10 in types) add("hdr10")
        if (Display.HdrCapabilities.HDR_TYPE_HLG in types) add("hlg")
        if (Display.HdrCapabilities.HDR_TYPE_DOLBY_VISION in types) add("dolby_vision")
    }
}
