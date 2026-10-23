package app.nuvora.tv.ui.player

import android.graphics.BitmapRegionDecoder
import android.graphics.Rect
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.NuvoraApi
import app.nuvora.tv.data.PreviewInfo
import app.nuvora.tv.data.PreviewTile
import app.nuvora.tv.data.tileAt
import app.nuvora.tv.ui.details.clockText
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

/** A preview this wide on screen; previews are made 320 px wide. */
private val FRAME_WIDTH = 288.dp
private const val ASK_AGAIN_MS = 15_000L

/** How far one press moves the scrub: further the longer the button is held. */
fun scrubStep(repeatCount: Int): Long = when {
    repeatCount < 8 -> 10_000L
    repeatCount < 24 -> 30_000L
    else -> 60_000L
}

/**
 * A file's seek previews, fetched once they exist. Sheets are kept as JPEG
 * bytes (a few hundred KB each) and only the tile on screen is decoded: a
 * whole sheet decoded would be 23 MB, too much for a TV stick.
 */
class ScrubPreviews {
    var info by mutableStateOf<PreviewInfo?>(null)
        internal set
    private val decoders = mutableMapOf<String, BitmapRegionDecoder>()

    internal fun add(sheet: String, jpeg: ByteArray) {
        @Suppress("DEPRECATION") // The ByteArray overload without it arrived in Android 12.
        decoders[sheet] = BitmapRegionDecoder.newInstance(jpeg, 0, jpeg.size, false) ?: return
    }

    internal fun decode(tile: PreviewTile): ImageBitmap? {
        val info = info ?: return null
        val decoder = decoders[tile.sheet] ?: return null
        return synchronized(decoder) {
            decoder.decodeRegion(Rect(tile.x, tile.y, tile.x + info.width, tile.y + info.height), null)?.asImageBitmap()
        }
    }
}

@Composable
fun rememberScrubPreviews(api: NuvoraApi, fileId: Long): ScrubPreviews {
    val previews = remember(fileId) { ScrubPreviews() }
    LaunchedEffect(api, fileId) {
        var info: PreviewInfo? = null
        while (info == null) {
            info = runCatching { api.previews(fileId) }.getOrNull()
            if (info == null) delay(ASK_AGAIN_MS)
        }
        previews.info = info
        for (sheet in info.sheets) {
            runCatching { api.bytes(sheet) }.getOrNull()?.let { jpeg -> withContext(Dispatchers.Default) { previews.add(sheet, jpeg) } }
        }
    }
    return previews
}

/** While scrubbing: where the jump will land, with the frame there once previews exist. */
@Composable
fun ScrubBar(target: Long, duration: Long, previews: ScrubPreviews) {
    val info = previews.info
    val tile = info?.tileAt(target)
    var frame by remember { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(tile) {
        // The last frame stays up until the next is decoded, so moving never flashes empty.
        if (tile != null) withContext(Dispatchers.Default) { previews.decode(tile) }?.let { frame = it }
    }
    val fraction = if (duration > 0) (target.toFloat() / duration).coerceIn(0f, 1f) else 0f

    Box(Modifier.fillMaxSize()) {
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(320.dp).background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.85f)))))
        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 56.dp, vertical = 40.dp)) {
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val image = frame.takeIf { info != null }
                val cardWidth = if (image != null) FRAME_WIDTH else 96.dp
                val x = (maxWidth * fraction - cardWidth / 2).coerceIn(0.dp, maxWidth - cardWidth)
                Column(
                    Modifier.offset(x = x).width(cardWidth).clip(RoundedCornerShape(10.dp)).background(Nv.surface).padding(5.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    if (image != null && info != null) {
                        Canvas(Modifier.fillMaxWidth().height(FRAME_WIDTH * info.height / info.width).clip(RoundedCornerShape(6.dp))) {
                            drawImage(
                                image,
                                dstSize = IntSize(size.width.roundToInt(), size.height.roundToInt()),
                                filterQuality = FilterQuality.Medium,
                            )
                        }
                        Spacer(Modifier.height(4.dp))
                    }
                    Text(clockText(target), style = MaterialTheme.typography.titleSmall, color = Nv.textPrimary)
                }
            }
            Spacer(Modifier.height(12.dp))
            Box(Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(Color.White.copy(alpha = 0.25f))) {
                Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(Nv.accent))
            }
            Spacer(Modifier.height(10.dp))
            Row {
                Spacer(Modifier.weight(1f))
                Text("−" + clockText((duration - target).coerceAtLeast(0)), style = MaterialTheme.typography.titleSmall, color = Nv.textSecondary)
            }
        }
    }
}
