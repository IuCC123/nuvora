package app.nuvora.tv.ui.common

import android.graphics.Bitmap
import android.util.Base64
import android.util.LruCache
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * ThumbHash (https://evanw.github.io/thumbhash/) to a tiny bitmap: the blurred
 * stand-in every artwork shows until it loads. A port of the reference
 * decoder; the server sends the hash as base64 with each image.
 */
object ThumbHash {
    private val cache = LruCache<String, Bitmap>(400)

    fun bitmap(id: String, base64: String): Bitmap? {
        if (base64.isEmpty()) return null
        cache.get(id)?.let { return it }
        val decoded = runCatching { decode(Base64.decode(base64, Base64.DEFAULT)) }.getOrNull() ?: return null
        cache.put(id, decoded)
        return decoded
    }

    private fun decode(hash: ByteArray): Bitmap {
        fun b(i: Int) = hash[i].toInt() and 0xFF
        val header24 = b(0) or (b(1) shl 8) or (b(2) shl 16)
        val header16 = b(3) or (b(4) shl 8)
        val lDc = (header24 and 63) / 63.0
        val pDc = ((header24 shr 6) and 63) / 31.5 - 1
        val qDc = ((header24 shr 12) and 63) / 31.5 - 1
        val lScale = ((header24 shr 18) and 31) / 31.0
        val hasAlpha = (header24 shr 23) != 0
        val pScale = ((header16 shr 3) and 63) / 63.0
        val qScale = ((header16 shr 9) and 63) / 63.0
        val isLandscape = (header16 shr 15) != 0
        val lx = max(3, if (isLandscape) (if (hasAlpha) 5 else 7) else header16 and 7)
        val ly = max(3, if (isLandscape) header16 and 7 else if (hasAlpha) 5 else 7)
        val aDc = if (hasAlpha) (b(5) and 15) / 15.0 else 1.0
        val aScale = (b(5) shr 4) / 15.0

        val acStart = if (hasAlpha) 6 else 5
        var acIndex = 0
        fun channel(nx: Int, ny: Int, scale: Double): DoubleArray {
            val ac = ArrayList<Double>()
            for (cy in 0 until ny) {
                var cx = if (cy > 0) 0 else 1
                while (cx * ny < nx * (ny - cy)) {
                    val v = (b(acStart + (acIndex shr 1)) shr ((acIndex and 1) shl 2)) and 15
                    acIndex++
                    ac.add((v / 7.5 - 1) * scale)
                    cx++
                }
            }
            return ac.toDoubleArray()
        }
        val lAc = channel(lx, ly, lScale)
        val pAc = channel(3, 3, pScale * 1.25)
        val qAc = channel(3, 3, qScale * 1.25)
        val aAc = if (hasAlpha) channel(5, 5, aScale) else DoubleArray(0)

        val ratio = aspect(hash)
        val w = (if (ratio > 1) 32.0 else 32 * ratio).roundToInt().coerceAtLeast(1)
        val h = (if (ratio > 1) 32 / ratio else 32.0).roundToInt().coerceAtLeast(1)
        val pixels = IntArray(w * h)
        val fx = DoubleArray(max(lx, if (hasAlpha) 5 else 3))
        val fy = DoubleArray(max(ly, if (hasAlpha) 5 else 3))
        for (y in 0 until h) {
            for (x in 0 until w) {
                var l = lDc
                var p = pDc
                var q = qDc
                var a = aDc
                for (cx in fx.indices) fx[cx] = cos(PI / w * (x + 0.5) * cx)
                for (cy in fy.indices) fy[cy] = cos(PI / h * (y + 0.5) * cy)
                var j = 0
                for (cy in 0 until ly) {
                    var cx = if (cy > 0) 0 else 1
                    val fy2 = fy[cy] * 2
                    while (cx * ly < lx * (ly - cy)) {
                        l += lAc[j++] * fx[cx] * fy2
                        cx++
                    }
                }
                j = 0
                for (cy in 0 until 3) {
                    var cx = if (cy > 0) 0 else 1
                    val fy2 = fy[cy] * 2
                    while (cx < 3 - cy) {
                        val f = fx[cx] * fy2
                        p += pAc[j] * f
                        q += qAc[j] * f
                        j++
                        cx++
                    }
                }
                if (hasAlpha) {
                    j = 0
                    for (cy in 0 until 5) {
                        var cx = if (cy > 0) 0 else 1
                        val fy2 = fy[cy] * 2
                        while (cx < 5 - cy) {
                            a += aAc[j++] * fx[cx] * fy2
                            cx++
                        }
                    }
                }
                val blue = l - 2.0 / 3.0 * p
                val red = (3 * l - blue + q) / 2
                val green = red - q
                fun c(v: Double) = (255 * min(1.0, v)).coerceAtLeast(0.0).toInt()
                pixels[y * w + x] = (c(a) shl 24) or (c(red) shl 16) or (c(green) shl 8) or c(blue)
            }
        }
        return Bitmap.createBitmap(pixels, w, h, Bitmap.Config.ARGB_8888)
    }

    private fun aspect(hash: ByteArray): Double {
        val header = hash[3].toInt() and 0xFF
        val hasAlpha = (hash[2].toInt() and 0x80) != 0
        val isLandscape = (hash[4].toInt() and 0x80) != 0
        val lx = if (isLandscape) (if (hasAlpha) 5 else 7) else header and 7
        val ly = if (isLandscape) header and 7 else if (hasAlpha) 5 else 7
        return lx.toDouble() / ly.coerceAtLeast(1)
    }
}
