package app.nuvora.tv.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.BuildConfig
import app.nuvora.tv.data.Session
import app.nuvora.tv.data.probeDeviceProfile
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.home.Section
import app.nuvora.tv.ui.home.Shell
import app.nuvora.tv.ui.theme.Nv

@Composable
fun SettingsScreen(nav: NavHostController, session: Session?, onSignOut: () -> Unit, onChangeServer: () -> Unit) {
    val context = LocalContext.current
    val profile = remember { probeDeviceProfile(context) }
    val focus = rememberInitialFocus()
    Shell(nav, Section.Settings) {
        Column(Modifier.fillMaxSize().padding(horizontal = 56.dp, vertical = 44.dp)) {
            Text("Settings", style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary)
            Spacer(Modifier.height(28.dp))
            Panel {
                Label("SIGNED IN")
                Text(session?.user?.displayName.orEmpty(), style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary)
                Text(
                    "${session?.server?.name.orEmpty()} · ${session?.server?.url.orEmpty().removePrefix("http://").removePrefix("https://")}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Nv.textSecondary,
                )
                Spacer(Modifier.height(18.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    NvButton("Sign out", onSignOut, Modifier.initialFocus(focus), icon = NvIcons.SignOut)
                    NvButton("Use another server", onChangeServer, icon = NvIcons.Server)
                }
            }
            Spacer(Modifier.height(22.dp))
            Panel {
                Label("THIS TV PLAYS")
                Text(
                    profile.video.joinToString("   ") { v ->
                        val name = when (v.codec) { "h264" -> "H.264"; "hevc" -> "HEVC"; "av1" -> "AV1"; "vp9" -> "VP9"; else -> v.codec }
                        val res = if (v.maxHeight >= 2160) "4K" else "${v.maxHeight}p"
                        val hdr = if (v.hdr.isNotEmpty()) " " + v.hdr.joinToString("/") { it.uppercase().replace("_", " ") } else ""
                        "$name $res${if (v.maxBitDepth > 8) " 10-bit" else ""}$hdr"
                    },
                    style = MaterialTheme.typography.bodyLarge,
                    color = Nv.textPrimary,
                )
                Text(
                    "Audio: " + profile.audio.joinToString(", ") { it.codec.uppercase().replace("EAC3", "E-AC-3").replace("AC3", "AC-3") },
                    style = MaterialTheme.typography.bodyMedium,
                    color = Nv.textSecondary,
                )
                Text(
                    "Anything else plays too: the server converts just the part this TV can't decode.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Nv.textMuted,
                )
            }
            Spacer(Modifier.weight(1f))
            Text("Nuvora for TV ${BuildConfig.VERSION_NAME}", style = MaterialTheme.typography.bodySmall, color = Nv.textMuted)
        }
    }
}

@Composable
private fun Panel(content: @Composable () -> Unit) {
    Column(Modifier.width(860.dp).clip(RoundedCornerShape(14.dp)).background(Nv.surface).padding(24.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        content()
    }
}

@Composable
private fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.labelSmall, color = Nv.textMuted)
    Spacer(Modifier.height(4.dp))
}
