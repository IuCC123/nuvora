package app.nuvora.tv.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.SavedServer
import app.nuvora.tv.data.SessionStore
import app.nuvora.tv.data.findServer
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.launch

/** The first screen: which Nuvora server to use. */
@Composable
fun ConnectScreen(store: SessionStore, onFound: (SavedServer) -> Unit) {
    val scope = rememberCoroutineScope()
    val leave = rememberLeaveTextScreen()
    val recent = remember { store.servers() }
    var address by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val focus = rememberInitialFocus()

    fun connect(typed: String) {
        if (busy) return
        busy = true
        error = null
        scope.launch {
            findServer(store, typed)
                .onSuccess { leave { onFound(it) } }
                .onFailure { error = (it as? ApiException)?.message ?: "Couldn't connect." }
            busy = false
        }
    }

    AuthLayout {
        Text("Connect to your server", style = MaterialTheme.typography.displaySmall, color = Nv.textPrimary)
        Spacer(Modifier.height(12.dp))
        Text(
            "Type the address of your Nuvora server: the installer prints it, and it's in your browser's address bar, " +
                "for example 192.168.1.20:8640.",
            style = MaterialTheme.typography.bodyLarge,
            color = Nv.textSecondary,
        )
        Spacer(Modifier.height(28.dp))
        NvField(
            value = address,
            onValueChange = { address = it },
            label = "Server address",
            placeholder = "192.168.1.20:8640",
            keyboardType = KeyboardType.Uri,
            imeAction = ImeAction.Go,
            onIme = { connect(address) },
            modifier = Modifier.initialFocus(focus),
        )
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(18.dp)) {
            NvButton(if (busy) "Connecting…" else "Connect", { connect(address) }, primary = true, enabled = !busy)
            error?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.danger, modifier = Modifier.weight(1f)) }
        }
        if (recent.isNotEmpty()) {
            Spacer(Modifier.height(40.dp))
            Text("RECENT", style = MaterialTheme.typography.labelSmall, color = Nv.textMuted)
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                recent.take(4).forEach { server ->
                    NvButton("${server.name} · ${server.url.removePrefix("http://").removePrefix("https://")}", { connect(server.url) }, icon = NvIcons.Server)
                }
            }
        }
    }
}

/** Signing in to the server just found or remembered. */
@Composable
fun SignInScreen(store: SessionStore, server: SavedServer, onSignedIn: () -> Unit, onChangeServer: () -> Unit) {
    val scope = rememberCoroutineScope()
    val leave = rememberLeaveTextScreen()
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val focus = rememberInitialFocus()

    fun signIn() {
        if (busy) return
        if (username.isBlank() || password.isEmpty()) {
            error = "Enter your username and password."
            return
        }
        busy = true
        error = null
        scope.launch {
            try {
                val session = store.guest(server.url).signIn(username.trim(), password)
                leave {
                    store.signedIn(server, session)
                    onSignedIn()
                }
            } catch (e: ApiException) {
                error = when (e.status) {
                    401 -> "That username and password don't match an account on ${server.name}."
                    else -> e.message
                }
            }
            busy = false
        }
    }

    AuthLayout {
        Text("Sign in", style = MaterialTheme.typography.displaySmall, color = Nv.textPrimary)
        Spacer(Modifier.height(10.dp))
        Text(
            "to ${server.name} at ${server.url.removePrefix("http://").removePrefix("https://")}",
            style = MaterialTheme.typography.bodyLarge,
            color = Nv.textSecondary,
        )
        Spacer(Modifier.height(28.dp))
        NvField(username, { username = it }, "Username", keyboardType = KeyboardType.Text, imeAction = ImeAction.Next, modifier = Modifier.initialFocus(focus))
        Spacer(Modifier.height(14.dp))
        NvField(password, { password = it }, "Password", keyboardType = KeyboardType.Password, imeAction = ImeAction.Go, onIme = ::signIn, password = true)
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            NvButton(if (busy) "Signing in…" else "Sign in", ::signIn, primary = true, enabled = !busy)
            NvButton("Use another server", { leave(onChangeServer) })
        }
        error?.let {
            Spacer(Modifier.height(14.dp))
            Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.danger)
        }
    }
}

/** The wordmark on the left, the form on the right, over a warm wash. */
@Composable
private fun AuthLayout(content: @Composable () -> Unit) {
    Box(
        Modifier.fillMaxSize().background(
            Brush.radialGradient(listOf(Color(0xFF2A2410), Nv.canvas), center = androidx.compose.ui.geometry.Offset(220f, 0f), radius = 1400f),
        ),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 96.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(0.8f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(NvIcons.Play, null, tint = Nv.accent, modifier = Modifier.size(48.dp))
                    Spacer(Modifier.width(10.dp))
                    Text("nuvora", fontSize = 56.sp, fontWeight = FontWeight.Bold, color = Nv.textPrimary)
                }
                Spacer(Modifier.height(10.dp))
                Text("Your movies and shows, on the big screen.", style = MaterialTheme.typography.titleMedium, color = Nv.textSecondary)
            }
            Column(Modifier.weight(1.2f)) { content() }
        }
    }
}

@Composable
private fun NvField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    keyboardType: KeyboardType = KeyboardType.Text,
    imeAction: ImeAction = ImeAction.Next,
    onIme: () -> Unit = {},
    password: Boolean = false,
) {
    val focusManager = LocalFocusManager.current
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        label = { androidx.compose.material3.Text(label) },
        placeholder = placeholder?.let { { androidx.compose.material3.Text(it, color = Nv.textMuted) } },
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction, autoCorrectEnabled = false),
        // Next moves to the field below; the last field's action submits.
        keyboardActions = KeyboardActions(onAny = { if (imeAction == ImeAction.Next) focusManager.moveFocus(FocusDirection.Down) else onIme() }),
        visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        shape = RoundedCornerShape(10.dp),
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = Nv.textPrimary,
            unfocusedTextColor = Nv.textPrimary,
            focusedBorderColor = Nv.accent,
            unfocusedBorderColor = Nv.hairline,
            focusedLabelColor = Nv.accent,
            unfocusedLabelColor = Nv.textSecondary,
            cursorColor = Nv.accent,
            focusedContainerColor = Nv.surface,
            unfocusedContainerColor = Nv.surface,
        ),
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * Closes the on-screen keyboard and lets go of the text field before
 * [then] changes screen. Leaving with the keyboard up can leave the TV's
 * keyboard holding on to the remote's arrow keys.
 */
@Composable
private fun rememberLeaveTextScreen(): (() -> Unit) -> Unit {
    val keyboard = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    return { then ->
        keyboard?.hide()
        focusManager.clearFocus(force = true)
        then()
    }
}
