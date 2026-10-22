package app.nuvora.tv.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

/** A server this TV has connected to, for the list on the connect screen. */
@Serializable
data class SavedServer(val url: String, val name: String)

/** Signed in to one server. */
data class Session(val server: SavedServer, val token: String, val user: User)

/**
 * Who is signed in where, kept across restarts in SharedPreferences, and the
 * one HTTP client every request and image goes through.
 */
class SessionStore(context: Context, profile: DeviceProfile) {
    private val prefs = context.getSharedPreferences("nuvora", Context.MODE_PRIVATE)
    val deviceProfile: String = json.encodeToString(DeviceProfile.serializer(), profile)

    val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(8, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    /** Watch Together's live connection, one for the app. */
    val party = PartyLink()

    private val _session = MutableStateFlow(load())
    val session: StateFlow<Session?> = _session

    /** The API for the signed-in session; null when signed out. */
    fun api(): NuvoraApi? = _session.value?.let { NuvoraApi(it.server.url, it.token, deviceProfile, http) }

    /** An API for a server before signing in. */
    fun guest(url: String) = NuvoraApi(url, null, deviceProfile, http)

    fun servers(): List<SavedServer> = prefs.getString("servers", null)
        ?.let { runCatching { json.decodeFromString(ListSerializer(SavedServer.serializer()), it) }.getOrNull() }
        .orEmpty()

    fun signedIn(server: SavedServer, session: AppSession) {
        val servers = listOf(server) + servers().filter { it.url != server.url }
        prefs.edit()
            .putString("servers", json.encodeToString(ListSerializer(SavedServer.serializer()), servers.take(8)))
            .putString("server_url", server.url)
            .putString("server_name", server.name)
            .putString("token", session.token)
            .putString("user", json.encodeToString(User.serializer(), session.user))
            .apply()
        _session.value = Session(server, session.token, session.user)
    }

    fun forget(server: SavedServer) {
        prefs.edit()
            .putString("servers", json.encodeToString(ListSerializer(SavedServer.serializer()), servers().filter { it.url != server.url }))
            .apply()
    }

    fun signOut() {
        prefs.edit().remove("token").remove("user").apply()
        party.stop()
        _session.value = null
    }

    /** The signed-in account changed on the server (a setting saved). */
    fun updateUser(user: User) {
        val current = _session.value ?: return
        prefs.edit().putString("user", json.encodeToString(User.serializer(), user)).apply()
        _session.value = current.copy(user = user)
    }

    // ------------------------------------------------------------ player

    var subtitleStyle: SubtitleStyle
        get() = prefs.getString("subtitle_style", null)
            ?.let { runCatching { json.decodeFromString(SubtitleStyle.serializer(), it) }.getOrNull() }
            ?: SubtitleStyle()
        set(value) = prefs.edit().putString("subtitle_style", json.encodeToString(SubtitleStyle.serializer(), value)).apply()

    /** The viewer's last subtitle choice for [scope] (`"show:12"`, `"movie:4"`); per user, so two people on one TV keep their own. */
    fun subtitleChoice(userId: Long, scope: String): RememberedSubtitle? =
        prefs.getString("subtitles.$userId.$scope", null)
            ?.let { runCatching { json.decodeFromString(RememberedSubtitle.serializer(), it) }.getOrNull() }

    fun saveSubtitleChoice(userId: Long, scope: String, choice: RememberedSubtitle) {
        prefs.edit().putString("subtitles.$userId.$scope", json.encodeToString(RememberedSubtitle.serializer(), choice)).apply()
    }

    private fun load(): Session? {
        val url = prefs.getString("server_url", null) ?: return null
        val token = prefs.getString("token", null) ?: return null
        val user = prefs.getString("user", null)
            ?.let { runCatching { json.decodeFromString(User.serializer(), it) }.getOrNull() } ?: return null
        return Session(SavedServer(url, prefs.getString("server_name", null) ?: "Nuvora"), token, user)
    }
}
