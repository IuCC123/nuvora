package app.nuvora.tv

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import app.nuvora.tv.ui.NuvoraRoot
import app.nuvora.tv.ui.theme.NuvoraTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            NuvoraTheme {
                NuvoraRoot(store)
            }
        }
    }
}
