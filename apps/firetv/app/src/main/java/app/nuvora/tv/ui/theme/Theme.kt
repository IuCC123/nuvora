package app.nuvora.tv.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Typography
import androidx.tv.material3.darkColorScheme

/** The web app's palette (packages/tokens), so both look like one product. */
object Nv {
    val canvas = Color(0xFF0F1012)
    val surface = Color(0xFF1A1C1F)
    val surfaceRaised = Color(0xFF24272B)
    val hairline = Color(0xFF2C2F34)
    val textPrimary = Color(0xFFF2F2F2)
    val textSecondary = Color(0xFFA6A8AB)
    val textMuted = Color(0xFF85888D)
    val accent = Color(0xFFE5A00D)
    val accentText = Color(0xFF141414)
    val danger = Color(0xFFE5534B)
}

private val colors = darkColorScheme(
    primary = Nv.accent,
    onPrimary = Nv.accentText,
    secondary = Nv.surfaceRaised,
    onSecondary = Nv.textPrimary,
    background = Nv.canvas,
    onBackground = Nv.textPrimary,
    surface = Nv.surface,
    onSurface = Nv.textPrimary,
    surfaceVariant = Nv.surfaceRaised,
    onSurfaceVariant = Nv.textSecondary,
    border = Nv.hairline,
    error = Nv.danger,
)

private val type = Typography(
    displaySmall = TextStyle(fontSize = 40.sp, lineHeight = 46.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.5).sp),
    headlineMedium = TextStyle(fontSize = 30.sp, lineHeight = 36.sp, fontWeight = FontWeight.SemiBold),
    headlineSmall = TextStyle(fontSize = 24.sp, lineHeight = 30.sp, fontWeight = FontWeight.SemiBold),
    titleLarge = TextStyle(fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.SemiBold),
    titleMedium = TextStyle(fontSize = 17.sp, lineHeight = 22.sp, fontWeight = FontWeight.SemiBold),
    titleSmall = TextStyle(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    bodyLarge = TextStyle(fontSize = 17.sp, lineHeight = 25.sp),
    bodyMedium = TextStyle(fontSize = 15.sp, lineHeight = 21.sp),
    bodySmall = TextStyle(fontSize = 13.sp, lineHeight = 18.sp),
    labelLarge = TextStyle(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.SemiBold),
    labelMedium = TextStyle(fontSize = 13.sp, lineHeight = 18.sp, fontWeight = FontWeight.Medium),
    labelSmall = TextStyle(fontSize = 11.sp, lineHeight = 14.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.6.sp),
)

@Composable
fun NuvoraTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = colors, typography = type, content = content)
}
