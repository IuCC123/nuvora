package app.nuvora.tv.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// The server's API types (see packages/core/openapi.json), as much as the TV
// app reads. Unknown fields are ignored, so a newer server never breaks it.

@Serializable
data class Health(val status: String, val version: String = "")

@Serializable
data class AuthState(
    @SerialName("server_name") val serverName: String = "Nuvora",
    @SerialName("setup_required") val setupRequired: Boolean = false,
    val user: User? = null,
)

@Serializable
data class User(
    val id: Long,
    val username: String,
    @SerialName("display_name") val displayName: String,
    val role: String = "member",
    @SerialName("subtitle_mode") val subtitleMode: String = "auto",
    @SerialName("subtitle_language") val subtitleLanguage: String? = null,
)

@Serializable
data class LoginRequest(val username: String, val password: String)

@Serializable
data class AppSession(val token: String, val user: User)

@Serializable
data class ErrorBody(val code: String = "", val message: String = "")

@Serializable
data class Image(
    val id: String,
    @SerialName("aspect_ratio") val aspectRatio: Float = 2f / 3f,
    val placeholder: String = "",
)

@Serializable
data class PosterCard(
    val id: Long,
    val kind: String,
    val title: String,
    val year: Int? = null,
    val poster: Image? = null,
    val progress: Float? = null,
    val watched: Boolean = false,
    val subtitle: String? = null,
    @SerialName("show_id") val showId: Long? = null,
    @SerialName("unwatched_count") val unwatchedCount: Int? = null,
)

@Serializable
data class HomeResponse(val rows: List<HomeRow> = emptyList())

@Serializable
data class HomeRow(
    val kind: String,
    val title: String,
    val total: Int = 0,
    val items: List<PosterCard> = emptyList(),
    @SerialName("library_id") val libraryId: Long? = null,
)

@Serializable
data class LibrarySummary(
    val id: Long,
    val name: String,
    val kind: String,
    @SerialName("item_count") val itemCount: Int = 0,
    val icon: String? = null,
)

@Serializable
data class LibraryPage(
    val total: Int = 0,
    val items: List<PosterCard> = emptyList(),
    val next: String? = null,
)

@Serializable
data class SearchResponse(
    val titles: List<PosterCard> = emptyList(),
    val episodes: List<SearchEpisode> = emptyList(),
    val people: List<SearchPerson> = emptyList(),
)

@Serializable
data class SearchPerson(val name: String, val image: Image? = null, val roles: List<SearchRole> = emptyList())

/** One title a person is in, and who they play or voice there. */
@Serializable
data class SearchRole(val card: PosterCard, val character: String? = null)

@Serializable
data class SearchEpisode(
    val id: Long,
    @SerialName("show_id") val showId: Long,
    @SerialName("show_title") val showTitle: String,
    val season: Int,
    val episode: Int,
    val title: String,
    val poster: Image? = null,
)

@Serializable
data class WatchState(
    @SerialName("position_ms") val positionMs: Long? = null,
    @SerialName("file_id") val fileId: Long? = null,
    val watched: Boolean = false,
)

@Serializable
data class Details(
    val genres: List<String> = emptyList(),
    val rating: Float? = null,
    val studio: String? = null,
    val tagline: String? = null,
    val cast: List<CastMember> = emptyList(),
    val source: String? = null,
)

@Serializable
data class CastMember(
    val kind: String = "cast",
    val name: String,
    val character: String? = null,
    val language: String? = null,
    val image: Image? = null,
    @SerialName("character_image") val characterImage: Image? = null,
)

@Serializable
data class Version(
    @SerialName("file_id") val fileId: Long,
    @SerialName("file_name") val fileName: String = "",
    val summary: String = "",
    @SerialName("size_bytes") val sizeBytes: Long = 0,
    val media: MediaDetails? = null,
    val playback: Playback,
)

/** What pressing Play does on this device. Unknown kinds read as [kind] with no URL. */
@Serializable
data class Playback(
    val kind: String,
    val url: String? = null,
    val reasons: List<String> = emptyList(),
    val reason: String? = null,
    @SerialName("converts_video") val convertsVideo: Boolean = false,
) {
    val playable get() = url != null && (kind == "direct_play" || kind == "hls")
}

@Serializable
data class MediaDetails(
    val container: String = "",
    @SerialName("duration_ms") val durationMs: Long? = null,
    val bitrate: Long? = null,
    val audio: List<AudioTrack> = emptyList(),
    val subtitles: List<SubtitleTrack> = emptyList(),
)

@Serializable
data class AudioTrack(
    val index: Int,
    val codec: String = "",
    val channels: Int = 2,
    val layout: String? = null,
    val language: String? = null,
    val title: String? = null,
    @SerialName("is_default") val isDefault: Boolean = false,
)

@Serializable
data class SubtitleTrack(
    val index: Int,
    val codec: String = "",
    val format: String = "other",
    val renderable: Boolean = false,
    val language: String? = null,
    val title: String? = null,
    @SerialName("is_default") val isDefault: Boolean = false,
    @SerialName("is_forced") val isForced: Boolean = false,
    @SerialName("is_hearing_impaired") val isHearingImpaired: Boolean = false,
    val external: Boolean = false,
)

@Serializable
data class EpisodeRef(
    val id: Long,
    @SerialName("season_id") val seasonId: Long,
    val season: Int,
    val index: Int,
    val title: String,
    val label: String,
    val thumb: Image? = null,
    @SerialName("position_ms") val positionMs: Long? = null,
)

@Serializable
data class SeasonSummary(
    val id: Long,
    val index: Int,
    val title: String,
    @SerialName("episode_count") val episodeCount: Int = 0,
    @SerialName("watched_count") val watchedCount: Int = 0,
)

@Serializable
data class Episode(
    val id: Long,
    val index: Int,
    val title: String,
    val label: String,
    val thumb: Image? = null,
    @SerialName("runtime_ms") val runtimeMs: Long? = null,
    val watch: WatchState = WatchState(),
    val versions: List<Version> = emptyList(),
)

/** `GET /api/items/{id}`: one of these, by `kind`. */
sealed interface ItemDetail

@Serializable
data class MovieDetail(
    val id: Long,
    val title: String,
    val year: Int? = null,
    val overview: String? = null,
    val poster: Image? = null,
    val backdrop: Image? = null,
    @SerialName("runtime_ms") val runtimeMs: Long? = null,
    val versions: List<Version> = emptyList(),
    val watch: WatchState = WatchState(),
    val details: Details = Details(),
) : ItemDetail

@Serializable
data class ShowDetail(
    val id: Long,
    val title: String,
    @SerialName("year_range") val yearRange: String? = null,
    val overview: String? = null,
    val poster: Image? = null,
    val backdrop: Image? = null,
    val seasons: List<SeasonSummary> = emptyList(),
    val next: EpisodeRef? = null,
    @SerialName("episode_count") val episodeCount: Int = 0,
    @SerialName("watched_count") val watchedCount: Int = 0,
    val details: Details = Details(),
) : ItemDetail

@Serializable
data class EpisodeDetail(
    val id: Long,
    @SerialName("show_id") val showId: Long,
    @SerialName("show_title") val showTitle: String,
    val season: Int,
    val index: Int,
    val title: String,
    val label: String,
    val overview: String? = null,
    val backdrop: Image? = null,
    val thumb: Image? = null,
    val versions: List<Version> = emptyList(),
    val watch: WatchState = WatchState(),
    val next: EpisodeRef? = null,
) : ItemDetail

/** A season id, or any kind this build doesn't show on its own. */
data object OtherItem : ItemDetail

@Serializable
data class ProgressRequest(
    @SerialName("file_id") val fileId: Long,
    @SerialName("position_ms") val positionMs: Long,
    @SerialName("duration_ms") val durationMs: Long,
)

/** What the device decodes, sent as the `Nuvora-Device` header. */
@Serializable
data class DeviceProfile(
    val containers: List<String>,
    val video: List<VideoSupport>,
    val audio: List<AudioSupport>,
)

@Serializable
data class VideoSupport(
    val codec: String,
    @SerialName("max_width") val maxWidth: Int,
    @SerialName("max_height") val maxHeight: Int,
    @SerialName("max_bit_depth") val maxBitDepth: Int,
    val hdr: List<String>,
)

@Serializable
data class AudioSupport(
    val codec: String,
    @SerialName("max_channels") val maxChannels: Int,
)

// ------------------------------------------------------------ watch together

@Serializable
data class PartyPerson(val id: Long, @SerialName("display_name") val displayName: String)

@Serializable
data class PartyMember(val id: Long, @SerialName("display_name") val displayName: String, val online: Boolean = true)

/**
 * The party's shared clock: at server time [atMs] the position is
 * [positionMs]; while [playing] it advances in real time from there.
 */
@Serializable
data class PartyPlayback(
    @SerialName("item_id") val itemId: Long,
    @SerialName("file_id") val fileId: Long,
    /** `"movie"` or `"episode"`. */
    val kind: String,
    val title: String,
    val subtitle: String? = null,
    val playing: Boolean,
    @SerialName("position_ms") val positionMs: Long,
    @SerialName("at_ms") val atMs: Long,
    val seq: Long,
    val by: PartyPerson? = null,
    /** `"start"`, `"play"`, `"pause"` or `"seek"`. */
    val action: String,
)

@Serializable
data class PartyView(
    val id: String,
    @SerialName("host_id") val hostId: Long,
    val members: List<PartyMember> = emptyList(),
    val invited: List<PartyPerson> = emptyList(),
    val playback: PartyPlayback? = null,
)

@Serializable
data class PartyInvite(
    @SerialName("party_id") val partyId: String,
    val from: PartyPerson,
    val members: List<PartyPerson> = emptyList(),
    @SerialName("sent_at_ms") val sentAtMs: Long,
) {
    val key get() = "$partyId:$sentAtMs"
}

@Serializable
data class PartyStatus(val party: PartyView? = null, val invites: List<PartyInvite> = emptyList())

@Serializable
data class InviteRequest(@SerialName("user_id") val userId: Long)

// ------------------------------------------------------------------- scans

@Serializable
data class ScanSummary(
    @SerialName("finished_at") val finishedAt: Long = 0,
    val added: Int = 0,
    val changed: Int = 0,
    val removed: Int = 0,
    val failed: Int = 0,
)

/** `GET /api/scan`: idle (with the last scan's result) or running. */
@Serializable
data class ScanStatus(
    val state: String = "idle",
    val phase: String? = null,
    val done: Int = 0,
    val total: Int = 0,
    val last: ScanSummary? = null,
) {
    val running get() = state == "running"
}
