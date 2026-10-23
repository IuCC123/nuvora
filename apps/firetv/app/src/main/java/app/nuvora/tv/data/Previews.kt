package app.nuvora.tv.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * A file's seek previews (`/api/files/{id}/previews`): JPEG sheets of
 * [columns] x [rows] frames, each [width] x [height] pixels; frame `i` shows
 * the video at `i * intervalMs`, left to right, then top to bottom.
 */
@Serializable
data class PreviewInfo(
    @SerialName("interval_ms") val intervalMs: Long,
    val width: Int,
    val height: Int,
    val columns: Int,
    val rows: Int,
    val count: Int,
    /** Paths on the server, in order. */
    val sheets: List<String>,
)

/** Where one preview is: its sheet, and its top-left corner in the sheet's pixels. */
data class PreviewTile(val sheet: String, val x: Int, val y: Int)

/** The preview for [positionMs]; before the start is the first, past the end the last. */
fun PreviewInfo.tileAt(positionMs: Long): PreviewTile? {
    val index = (positionMs.coerceAtLeast(0) / intervalMs).coerceAtMost(count - 1L).toInt()
    val perSheet = columns * rows
    val sheet = sheets.getOrNull(index / perSheet) ?: return null
    val within = index % perSheet
    return PreviewTile(sheet, (within % columns) * width, (within / columns) * height)
}
