package app.nuvora.tv.data

import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

/**
 * The addresses to try for what someone typed with a remote. People type
 * `192.168.1.20`, `192.168.1.20:8640`, `nuvora.example.com` or a full URL;
 * a bare host is tried on Nuvora's port first, then as a website.
 */
fun serverCandidates(typed: String): List<String> {
    val input = typed.trim().trimEnd('/').replace(" ", "")
    if (input.isEmpty()) return emptyList()
    if (input.startsWith("http://", ignoreCase = true) || input.startsWith("https://", ignoreCase = true)) {
        return listOfNotNull(normalize(input))
    }
    val hostPart = input.substringBefore('/')
    val hasPort = hostPart.substringAfterLast(':', "").let { it.isNotEmpty() && it.all(Char::isDigit) }
    val candidates = if (hasPort) {
        listOf("http://$input", "https://$input")
    } else {
        listOf("http://$hostPart:8640", "https://$input", "http://$input")
    }
    return candidates.mapNotNull(::normalize).distinct()
}

private fun normalize(url: String): String? {
    val parsed = url.toHttpUrlOrNull() ?: return null
    return parsed.toString().trimEnd('/')
}

/**
 * Finds a Nuvora server among [serverCandidates]: the first that answers
 * `/api/health` like one. Returns its URL and name.
 */
suspend fun findServer(store: SessionStore, typed: String): Result<SavedServer> {
    val candidates = serverCandidates(typed)
    if (candidates.isEmpty()) return Result.failure(ApiException(0, "empty", "Type your server's address, like 192.168.1.20:8640."))
    var last: ApiException? = null
    for (url in candidates) {
        try {
            val api = store.guest(url)
            val health = api.health()
            if (health.status != "ok") continue
            val state = api.authState()
            if (state.setupRequired) {
                return Result.failure(
                    ApiException(0, "setup_required", "This server isn't set up yet. Open $url in a browser to create the first account."),
                )
            }
            return Result.success(SavedServer(url, state.serverName, state.serverId.ifEmpty { null }))
        } catch (e: ApiException) {
            last = e
        }
    }
    val message = when {
        last == null || last.status == 0 -> "Couldn't reach a Nuvora server at ${typed.trim()}. Check the address and that the server is running."
        last.status == 404 -> "Something answered at ${typed.trim()}, but it isn't a Nuvora server."
        else -> last.message ?: "Couldn't connect."
    }
    return Result.failure(ApiException(last?.status ?: 0, "unreachable", message))
}
