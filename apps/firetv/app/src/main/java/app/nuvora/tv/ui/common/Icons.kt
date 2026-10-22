package app.nuvora.tv.ui.common

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp

/** The web app's icon set (24-unit grid, round strokes), from the same SVG paths. */
object NvIcons {
    val Home = stroke("M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8", "M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z")
    val Search = stroke("M18 11a7 7 0 1 1-14 0a7 7 0 1 1 14 0z", "m20 20-3.5-3.5")
    val Film = stroke("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", "M7 3v18M3 7.5h4M3 12h18M3 16.5h4M17 3v18M17 7.5h4M17 16.5h4")
    val Tv = stroke("M4 4h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M8 21h8M12 18v3")
    val Settings = stroke(
        "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
        "M15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0z",
    )
    val Play = fill("M7 4.5v15a1 1 0 0 0 1.52.85l12.2-7.5a1 1 0 0 0 0-1.7L8.52 3.65A1 1 0 0 0 7 4.5z")
    val Pause = fill("M6.7 4h2.1A1.2 1.2 0 0 1 10 5.2v13.6A1.2 1.2 0 0 1 8.8 20H6.7a1.2 1.2 0 0 1-1.2-1.2V5.2A1.2 1.2 0 0 1 6.7 4z", "M15.2 4h2.1a1.2 1.2 0 0 1 1.2 1.2v13.6a1.2 1.2 0 0 1-1.2 1.2h-2.1a1.2 1.2 0 0 1-1.2-1.2V5.2A1.2 1.2 0 0 1 15.2 4z")
    val Check = stroke("M20 6 9 17l-5-5")
    val SkipNext = fill("M4 5.2v13.6a1 1 0 0 0 1.53.85l10.3-6.8a1 1 0 0 0 0-1.7L5.53 4.35A1 1 0 0 0 4 5.2z", "M18.6 4h.4a1.1 1.1 0 0 1 1.1 1.1v13.8a1.1 1.1 0 0 1-1.1 1.1h-.4a1.1 1.1 0 0 1-1.1-1.1V5.1A1.1 1.1 0 0 1 18.6 4z")
    val Restart = stroke("M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5")
    val Subtitles = stroke("M5.5 5h13A2.5 2.5 0 0 1 21 7.5v9a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 16.5v-9A2.5 2.5 0 0 1 5.5 5z", "M6.75 13h3.5M13.25 13h4M6.75 16.2h6.5M16.25 16.2h1")
    val Audio = stroke("M11 5 6 9H2v6h4l5 4V5z", "M15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14")
    val Server = stroke("M4 2h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z", "M4 14h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z", "M6 6h.01M6 18h.01")
    val SignOut = stroke("M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9")
    val User = stroke("M17 8a5 5 0 1 1-10 0a5 5 0 1 1 10 0z", "M20 21a8 8 0 0 0-16 0")
    val Back = stroke("m12 19-7-7 7-7M19 12H5")
    val Rewind = stroke("M11 19 2 12l9-7v14z", "M22 19l-9-7 9-7v14z")
    val Forward = stroke("M13 19l9-7-9-7v14z", "M2 19l9-7-9-7v14z")

    private fun stroke(vararg paths: String) = icon(paths, filled = false)
    private fun fill(vararg paths: String) = icon(paths, filled = true)

    private fun icon(paths: Array<out String>, filled: Boolean): ImageVector {
        val builder = ImageVector.Builder(defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f)
        for (d in paths) {
            val nodes = PathParser().parsePathString(d).toNodes()
            if (filled) {
                builder.addPath(nodes, fill = SolidColor(Color.White))
            } else {
                builder.addPath(
                    nodes,
                    stroke = SolidColor(Color.White),
                    strokeLineWidth = 1.9f,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Round,
                )
            }
        }
        return builder.build()
    }
}
