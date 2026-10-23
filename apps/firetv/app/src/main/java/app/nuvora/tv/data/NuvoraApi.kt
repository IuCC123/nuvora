package app.nuvora.tv.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

val json = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    coerceInputValues = true
}

/** A request the server answered with an error. [status] 0 means it never answered. */
class ApiException(val status: Int, val code: String, message: String) : Exception(message) {
    val unauthorized get() = status == 401
}

private val JSON_TYPE = "application/json".toMediaType()

/**
 * The Nuvora HTTP API, one server and one session. Every request carries the
 * session token and this device's decoding profile, so the server decides
 * per request what plays directly and what it converts.
 */
class NuvoraApi(
    val baseUrl: String,
    val token: String?,
    private val deviceProfile: String,
    val http: OkHttpClient,
) {
    private val base: HttpUrl = baseUrl.toHttpUrl()

    suspend fun health(): Health = get("/api/health", Health.serializer())
    suspend fun authState(): AuthState = get("/api/auth/state", AuthState.serializer())
    suspend fun signIn(username: String, password: String): AppSession =
        send("POST", "/api/auth/token", AppSession.serializer(), LoginRequest(username, password), LoginRequest.serializer())

    suspend fun signOut() {
        runCatching { sendNoContent("POST", "/api/auth/logout") }
    }

    suspend fun home(): HomeResponse = get("/api/home", HomeResponse.serializer())
    suspend fun libraries(): List<LibrarySummary> = get("/api/libraries", ListSerializer(LibrarySummary.serializer()))

    suspend fun libraryItems(id: Long, sort: String, order: String?, filter: String, after: String?, limit: Int = 60): LibraryPage =
        get(
            "/api/libraries/$id/items",
            LibraryPage.serializer(),
            buildMap {
                put("sort", sort)
                if (order != null) put("order", order)
                put("filter", filter)
                if (after != null) put("after", after)
                put("limit", limit.toString())
            },
        )

    suspend fun search(q: String): SearchResponse = get("/api/search", SearchResponse.serializer(), mapOf("q" to q))

    suspend fun item(id: Long): ItemDetail {
        val obj = get("/api/items/$id", JsonObject.serializer())
        return when (obj["kind"]?.jsonPrimitive?.content) {
            "movie" -> json.decodeFromJsonElement(MovieDetail.serializer(), obj)
            "show" -> json.decodeFromJsonElement(ShowDetail.serializer(), obj)
            "episode" -> json.decodeFromJsonElement(EpisodeDetail.serializer(), obj)
            else -> OtherItem
        }
    }

    suspend fun episodes(seasonId: Long): List<Episode> =
        get("/api/items/$seasonId/episodes", ListSerializer(Episode.serializer()))

    suspend fun reportProgress(itemId: Long, body: ProgressRequest) {
        sendNoContent("PUT", "/api/items/$itemId/progress", json.encodeToString(ProgressRequest.serializer(), body))
    }

    suspend fun setWatched(itemId: Long, watched: Boolean) {
        sendNoContent(if (watched) "PUT" else "DELETE", "/api/items/$itemId/watched")
    }

    // ------------------------------------------------------- watch together

    /** Everyone else on the server, who can be invited. */
    /** Null while the server is still making them; asking moves the file to the front of its queue. */
    suspend fun previews(fileId: Long): PreviewInfo? = try {
        get("/api/files/$fileId/previews", PreviewInfo.serializer())
    } catch (e: ApiException) {
        if (e.status == 404) null else throw e
    }

    suspend fun people(): List<PartyPerson> = get("/api/people", ListSerializer(PartyPerson.serializer()))

    suspend fun invite(userId: Long): PartyView =
        send("POST", "/api/party/invites", PartyView.serializer(), InviteRequest(userId), InviteRequest.serializer())

    suspend fun cancelInvite(userId: Long): PartyView = execute(request("DELETE", url("/api/party/invites/$userId")), PartyView.serializer())

    suspend fun joinParty(partyId: String): PartyView = execute(request("POST", url("/api/party/$partyId/join")), PartyView.serializer())

    suspend fun declineParty(partyId: String) = sendNoContent("POST", "/api/party/$partyId/decline")

    suspend fun leaveParty() = sendNoContent("POST", "/api/party/leave")

    /** The live connection's request: `/api/live` with this session. */
    fun liveRequest(): Request = request("GET", url("/api/live"))

    // ------------------------------------------------------------ account

    /** Who is signed in, as the server has it now. */
    suspend fun me(): User? = authState().user

    /** The account's subtitle setting; a null [language] means the TV's own. */
    suspend fun setSubtitlePrefs(mode: String, language: String?): User {
        val body = JsonObject(mapOf("subtitle_mode" to JsonPrimitive(mode), "subtitle_language" to (language?.let(::JsonPrimitive) ?: JsonNull)))
        return execute(request("PATCH", url("/api/auth/me"), body.toString().toRequestBody(JSON_TYPE)), User.serializer())
    }

    suspend fun scanStatus(): ScanStatus = get("/api/scan", ScanStatus.serializer())

    /** Scans every library (admins only); joins a scan already running. */
    suspend fun startScan(): ScanStatus = execute(request("POST", url("/api/scan")), ScanStatus.serializer())

    /** An artwork URL; images are immutable, so caches may keep them forever. */
    fun imageUrl(image: Image, size: String = "small"): String = "$baseUrl/api/images/${image.id}/$size"

    /** The bytes of a server path the API returned, such as a preview sheet. */
    suspend fun bytes(path: String): ByteArray = withContext(Dispatchers.IO) {
        call(request("GET", resolve(path).toHttpUrl())).use { response ->
            if (!response.isSuccessful) throw error(response.code, response.body.string())
            response.body.bytes()
        }
    }

    /** A server-relative URL the API returned (`playback.url`), made absolute. */
    fun resolve(url: String): String = if (url.startsWith("http")) url else baseUrl + url

    fun subtitleUrl(fileId: Long, index: Int, format: String = "vtt") = "$baseUrl/api/files/$fileId/subtitles/$index?format=$format"

    // -------------------------------------------------------------- plumbing

    private fun url(path: String, query: Map<String, String> = emptyMap()): HttpUrl {
        val builder = base.newBuilder()
        path.trimStart('/').split('/').forEach { builder.addPathSegment(it) }
        query.forEach { (k, v) -> builder.addQueryParameter(k, v) }
        return builder.build()
    }

    private fun request(method: String, url: HttpUrl, body: RequestBody? = null): Request {
        val builder = Request.Builder().url(url).header("Nuvora-Device", deviceProfile)
        token?.let { builder.header("Authorization", "Bearer $it") }
        // State-changing requests must say they are JSON, even when empty.
        val sent = body ?: if (method == "GET") null else ByteArray(0).toRequestBody(JSON_TYPE)
        return builder.method(method, sent).build()
    }

    private suspend fun <T> get(path: String, serializer: KSerializer<T>, query: Map<String, String> = emptyMap()): T =
        execute(request("GET", url(path, query)), serializer)

    private suspend fun <B, T> send(method: String, path: String, serializer: KSerializer<T>, body: B, bodySerializer: KSerializer<B>): T =
        execute(request(method, url(path), json.encodeToString(bodySerializer, body).toRequestBody(JSON_TYPE)), serializer)

    private suspend fun sendNoContent(method: String, path: String, body: String? = null) {
        val req = request(method, url(path), body?.toRequestBody(JSON_TYPE))
        withContext(Dispatchers.IO) {
            call(req).use { response -> if (!response.isSuccessful) throw error(response.code, response.body.string()) }
        }
    }

    private suspend fun <T> execute(req: Request, serializer: KSerializer<T>): T = withContext(Dispatchers.IO) {
        call(req).use { response ->
            val text = response.body.string()
            if (!response.isSuccessful) throw error(response.code, text)
            try {
                json.decodeFromString(serializer, text)
            } catch (e: Exception) {
                throw ApiException(response.code, "unexpected", "The server sent something this app doesn't understand.")
            }
        }
    }

    private fun call(req: Request) = try {
        http.newCall(req).execute()
    } catch (e: IOException) {
        throw ApiException(0, "unreachable", "Can't reach the server. Check that it's running and on the same network.")
    }

    private fun error(status: Int, text: String): ApiException {
        val body = runCatching { json.decodeFromString(ErrorBody.serializer(), text) }.getOrNull()
        val message = body?.message?.takeIf { it.isNotBlank() } ?: when (status) {
            401 -> "Your session has ended. Sign in again."
            404 -> "That isn't on the server any more."
            else -> "The server had a problem ($status)."
        }
        return ApiException(status, body?.code ?: "", message)
    }
}
