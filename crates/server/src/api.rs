//! The HTTP API: wire types, handlers, and the OpenAPI document generated from
//! them. Wire types exist only in this file; they are built from the catalog's
//! domain read models, and the TypeScript client is generated from the spec.
//!
//! One request per screen:
//!
//! | method | path                        | screen / purpose                         |
//! |--------|-----------------------------|------------------------------------------|
//! | GET    | /api/home                   | Home (first page of each row)            |
//! | GET    | /api/rows/{row}             | Later pages of one home row              |
//! | GET    | /api/items/{id}             | Movie detail (and the Player)            |
//! | GET    | /api/images/{id}/{size}     | Artwork, immutable                       |
//! | GET    | /api/files/{id}/stream      | Direct play, HTTP Range                  |
//! | GET    | /api/scan                   | Scan status                              |
//! | POST   | /api/scan                   | Request a scan (idempotent, coalescing)  |
//! | GET    | /api/openapi.json           | The spec                                 |

use std::time::SystemTime;

use axum::Json;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode, request::Parts};
use axum::response::{IntoResponse, Response};
use nuvora_library::{
    ArtworkId, CatalogRev, Cursor, FileId, HOME_ROW_LIMIT, ItemId, Library, ScanPhase, ScanState,
    ScanStatus, VersionMedia,
};
use nuvora_media::{
    AudioCodec, AudioSupport, Container, DeviceProfile, HdrFormat, Incompatibility, MediaInfo,
    PlaybackDecision, SubtitleCodec, VideoCodec, VideoSupport,
};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::ServeFile;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(
    info(title = "Nuvora", version = "1"),
    components(schemas(DeviceProfileWire, ImageSize))
)]
struct ApiDoc;

/// All API routes plus the spec built from the same `routes!` calls, so a
/// route cannot exist without being documented (no hand-kept path list).
pub fn router() -> (axum::Router<Library>, utoipa::openapi::OpenApi) {
    let (router, spec) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_home))
        .routes(routes!(get_row))
        .routes(routes!(get_item))
        .routes(routes!(get_image))
        .routes(routes!(stream_file))
        .routes(routes!(get_scan, post_scan))
        .split_for_parts();
    let spec_json = spec.clone();
    (
        router.route(
            "/api/openapi.json",
            axum::routing::get(move || async move { Json(spec_json) }),
        ),
        spec,
    )
}

/// Gzip at the fastest level: brotli at its default level costs more time than
/// the 50 ms home budget allows. Never `video/*` (it would break Range requests)
/// or `image/*` (WebP is already compressed).
pub fn compression()
-> tower_http::compression::CompressionLayer<impl tower_http::compression::Predicate> {
    use tower_http::CompressionLevel;
    use tower_http::compression::predicate::{NotForContentType, Predicate, SizeAbove};
    tower_http::compression::CompressionLayer::new()
        .quality(CompressionLevel::Fastest)
        .compress_when(
            SizeAbove::new(1024)
                .and(NotForContentType::const_new("video/"))
                .and(NotForContentType::const_new("image/")),
        )
}

// ---------------------------------------------------------------- wire types

#[derive(Serialize, ToSchema)]
pub struct HomeResponse {
    /// Render in order. Clients render unknown row kinds like any other row.
    pub rows: Vec<HomeRow>,
}

#[derive(Serialize, ToSchema)]
pub struct HomeRow {
    pub kind: RowKind,
    pub title: String,
    /// Cards in the whole row, not just `items`.
    pub total: u32,
    /// The first page, at most 40 cards.
    pub items: Vec<PosterCard>,
    /// Opaque cursor for `GET /api/rows/{row}?after=`. `None` when `items` is the whole row.
    pub next: Option<String>,
}

/// A later page of one row.
#[derive(Serialize, ToSchema)]
pub struct RowPage {
    pub items: Vec<PosterCard>,
    pub next: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    RecentlyAdded,
    AllMovies,
}

impl From<RowKind> for nuvora_library::RowKind {
    fn from(k: RowKind) -> Self {
        match k {
            RowKind::RecentlyAdded => nuvora_library::RowKind::RecentlyAdded,
            RowKind::AllMovies => nuvora_library::RowKind::AllMovies,
        }
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RowQuery {
    /// `next` from the previous page; absent = from the start.
    pub after: Option<String>,
    /// Default 40, clamped to 1..=100.
    pub limit: Option<u32>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Movie,
}

#[derive(Serialize, ToSchema)]
pub struct PosterCard {
    pub id: i64,
    pub kind: ItemKind,
    pub title: String,
    pub year: Option<u16>,
    pub poster: Option<Image>,
}

/// An artwork. URL: `/api/images/{id}/{size}`. Immutable: cache forever.
#[derive(Serialize, ToSchema)]
pub struct Image {
    /// 32 hex chars, content-addressed.
    pub id: String,
    /// width / height, for layout before the image loads.
    pub aspect_ratio: f32,
    /// ThumbHash, base64. Decode client-side into a blurred placeholder.
    pub placeholder: String,
}

/// Sizes a client can request. Pixel widths are a server decision.
#[derive(Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ImageSize {
    /// Cards and rows.
    Small,
    /// Detail hero.
    Large,
}

impl From<ImageSize> for nuvora_media::ImageSize {
    fn from(s: ImageSize) -> Self {
        match s {
            ImageSize::Small => nuvora_media::ImageSize::Small,
            ImageSize::Large => nuvora_media::ImageSize::Large,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemDetail {
    Movie(MovieDetail),
}

#[derive(Serialize, ToSchema)]
pub struct MovieDetail {
    pub id: i64,
    pub title: String,
    pub year: Option<u16>,
    pub runtime_ms: Option<u64>,
    pub overview: Option<String>,
    pub poster: Option<Image>,
    pub backdrop: Option<Image>,
    /// Best first. Empty when every file of the movie is currently missing.
    pub versions: Vec<Version>,
}

#[derive(Serialize, ToSchema)]
pub struct Version {
    pub file_id: i64,
    pub file_name: String,
    pub size_bytes: u64,
    /// One line for the version picker and detail header, built server-side so
    /// every client says the same thing: `"4K · HEVC Main 10 · Dolby Vision · TrueHD 7.1"`.
    pub summary: String,
    /// `None` when the file could not be read.
    pub media: Option<MediaDetails>,
    pub playback: Playback,
}

#[derive(Serialize, ToSchema)]
pub struct MediaDetails {
    pub container: String,
    pub duration_ms: Option<u64>,
    pub bitrate: Option<u64>,
    pub video: Vec<VideoTrack>,
    pub audio: Vec<AudioTrack>,
    pub subtitles: Vec<SubtitleTrack>,
}

#[derive(Serialize, ToSchema)]
pub struct VideoTrack {
    /// Display name: `"HEVC"`, `"H.264"`, `"AV1"`.
    pub codec: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub bit_depth: Option<u8>,
    pub frame_rate: Option<f64>,
    /// `None` for SDR; `"HDR10"`, `"HLG"`, `"Dolby Vision P8"`.
    pub hdr: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AudioTrack {
    pub index: u32,
    pub codec: String,
    pub channels: u8,
    /// `"5.1"`, `"Stereo"`.
    pub layout: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
}

#[derive(Serialize, ToSchema)]
pub struct SubtitleTrack {
    pub index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
}

/// What pressing Play does on the requesting device. Step 3 adds `hls`.
/// Clients must treat an unknown `kind` like `needs_transcode`.
#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Playback {
    DirectPlay {
        url: String,
        mime_type: String,
    },
    NeedsTranscode {
        /// Human-readable: `"HEVC video isn't supported on this device"`.
        reasons: Vec<String>,
    },
    /// The file could not be read (ffprobe failed); nothing will play it.
    Unavailable {
        reason: String,
    },
}

#[derive(Serialize, ToSchema)]
#[schema(as = ScanStatus)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScanStatusResponse {
    Idle {
        last: Option<ScanSummaryWire>,
    },
    Running {
        phase: ScanPhaseWire,
        done: u32,
        total: u32,
        queued: bool,
        last: Option<ScanSummaryWire>,
    },
}

#[derive(Serialize, ToSchema)]
#[schema(as = ScanPhase)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhaseWire {
    Discovering,
    Reading,
    Artwork,
    CleaningUp,
}

#[derive(Serialize, ToSchema)]
#[schema(as = ScanSummary)]
pub struct ScanSummaryWire {
    /// Unix ms.
    pub finished_at: i64,
    pub duration_ms: u64,
    pub added: u32,
    pub changed: u32,
    pub moved: u32,
    pub removed: u32,
    pub unchanged: u32,
    pub failed: u32,
    pub issues: Vec<ScanIssueWire>,
}

#[derive(Serialize, ToSchema)]
#[schema(as = ScanIssue)]
pub struct ScanIssueWire {
    pub path: String,
    pub message: String,
}

/// What a device decodes natively. Sent as compact JSON in the `Nuvora-Device`
/// header. Codec and container names are the server's canonical names
/// (`h264`, `hevc`, `av1`, `vp9`; `aac`, `eac3`, `opus`; `mp4`, `mkv`, `webm`).
/// Unknown names are ignored, never errors, so old servers accept new clients.
#[derive(Deserialize, ToSchema)]
#[schema(as = DeviceProfile)]
pub struct DeviceProfileWire {
    pub containers: Vec<String>,
    pub video: Vec<VideoSupportWire>,
    pub audio: Vec<AudioSupportWire>,
}

#[derive(Deserialize, ToSchema)]
#[schema(as = VideoSupport)]
pub struct VideoSupportWire {
    pub codec: String,
    pub max_width: u32,
    pub max_height: u32,
    pub max_bit_depth: u8,
    /// Subset of `hdr10`, `hlg`, `dolby_vision`.
    pub hdr: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
#[schema(as = AudioSupport)]
pub struct AudioSupportWire {
    pub codec: String,
    pub max_channels: u8,
}

// ------------------------------------------------------------------ handlers

/// Extracts the `Nuvora-Device` header into a domain profile. Absent =>
/// `DeviceProfile::baseline()`. Malformed => 400 (a client bug, surfaced loudly).
pub struct Device {
    pub profile: DeviceProfile,
    /// blake3 of the raw header bytes (empty when absent), first 8 bytes as hex.
    /// The item ETag includes it because the item body depends on the device.
    pub hash: String,
}

impl<S: Send + Sync> FromRequestParts<S> for Device {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("nuvora-device")
            .map(HeaderValue::as_bytes)
            .unwrap_or_default();
        let hash = blake3::hash(raw).to_hex()[..16].to_owned();
        let profile = if raw.is_empty() {
            DeviceProfile::baseline()
        } else {
            let wire: DeviceProfileWire = serde_json::from_slice(raw).map_err(|e| {
                ApiError::BadRequest(format!("Nuvora-Device header is not a device profile: {e}"))
            })?;
            DeviceProfile::from(wire)
        };
        Ok(Device { profile, hash })
    }
}

#[utoipa::path(get, path = "/api/home", responses((status = 200, body = HomeResponse), (status = 304)))]
async fn get_home(State(lib): State<Library>, headers: HeaderMap) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev())) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let home = lib.home().await.map_err(ApiError::internal)?;
    Ok(json_with_etag(
        &rev_etag(home.rev),
        &HomeResponse::from(home),
    ))
}

/// Keyset paging on the row's sort key. A malformed cursor, or one from
/// another row, is 400.
#[utoipa::path(
    get,
    path = "/api/rows/{row}",
    params(("row" = RowKind, Path), RowQuery),
    responses((status = 200, body = RowPage), (status = 304), (status = 400)),
)]
async fn get_row(
    State(lib): State<Library>,
    Path(row): Path<RowKind>,
    Query(query): Query<RowQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev())) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let kind: nuvora_library::RowKind = row.into();
    let after = query
        .after
        .as_deref()
        .map(|raw| Cursor::parse(kind, raw))
        .transpose()
        .map_err(|_| ApiError::BadRequest("invalid cursor for this row".into()))?;
    let page = lib
        .row(kind, after, query.limit.unwrap_or(HOME_ROW_LIMIT))
        .await
        .map_err(ApiError::internal)?;
    let body = RowPage {
        items: page.cards.into_iter().map(PosterCard::from).collect(),
        next: page.next.map(|c| c.to_string()),
    };
    Ok(json_with_etag(&rev_etag(page.rev), &body))
}

#[utoipa::path(
    get,
    path = "/api/items/{id}",
    params(
        ("id" = i64, Path),
        ("Nuvora-Device" = Option<String>, Header, description = "JSON DeviceProfile"),
    ),
    responses((status = 200, body = ItemDetail), (status = 304), (status = 404)),
)]
async fn get_item(
    State(lib): State<Library>,
    Path(id): Path<i64>,
    device: Device,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let etag = item_etag(lib.rev(), &device);
    let mut response = if if_none_match(&headers, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let item = lib
            .item(ItemId::from_raw(id), device.profile)
            .await
            .map_err(ApiError::internal)?;
        json_with_etag(&etag, &ItemDetail::from(item.ok_or(ApiError::NotFound)?))
    };
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Nuvora-Device"));
    Ok(response)
}

/// Reads the WebP from the store (tens of KB) and answers with
/// `Cache-Control: public, max-age=31536000, immutable`. Bad ids are 404, and
/// ids are parsed to 16 bytes before any path is built.
#[utoipa::path(
    get,
    path = "/api/images/{id}/{size}",
    params(("id" = String, Path), ("size" = ImageSize, Path)),
    responses((status = 200, content_type = "image/webp", body = Vec<u8>), (status = 404)),
)]
async fn get_image(
    State(lib): State<Library>,
    Path((id, size)): Path<(String, ImageSize)>,
) -> Result<Response, ApiError> {
    let id: ArtworkId = id.parse().map_err(|_| ApiError::NotFound)?;
    let path = lib
        .artwork_path(id, size.into())
        .ok_or(ApiError::NotFound)?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    let headers = [
        (CONTENT_TYPE, "image/webp"),
        (CACHE_CONTROL, "public, max-age=31536000, immutable"),
    ];
    Ok((headers, bytes).into_response())
}

/// Direct play. Delegates to `tower_http::services::ServeFile` with the
/// container's MIME type, which implements `Range` (206 + `Content-Range`),
/// `If-Range`, `Last-Modified` and `HEAD`, and streams from disk without
/// buffering. First-frame latency is then disk + network + decoder.
#[utoipa::path(
    get,
    path = "/api/files/{id}/stream",
    params(("id" = i64, Path), ("Range" = Option<String>, Header)),
    responses((status = 200, body = Vec<u8>), (status = 206, body = Vec<u8>), (status = 404), (status = 416)),
)]
async fn stream_file(
    State(lib): State<Library>,
    Path(id): Path<i64>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let source = lib
        .stream_source(FileId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::NotFound)?;
    let mime: mime::Mime = source
        .container
        .mime_type()
        .parse()
        .expect("container MIME types are well-formed");
    let response = ServeFile::new_with_mime(source.path, &mime)
        .oneshot(req)
        .await
        .map_err(ApiError::internal)?;
    Ok(response.into_response())
}

#[utoipa::path(get, path = "/api/scan", responses((status = 200, body = ScanStatusResponse)))]
async fn get_scan(State(lib): State<Library>) -> Json<ScanStatusResponse> {
    Json(scan_status_wire(lib.scan_status()))
}

/// Idempotent: while a scan runs, repeated calls queue at most one more.
#[utoipa::path(post, path = "/api/scan", responses((status = 202, body = ScanStatusResponse)))]
async fn post_scan(State(lib): State<Library>) -> (StatusCode, Json<ScanStatusResponse>) {
    (
        StatusCode::ACCEPTED,
        Json(scan_status_wire(lib.request_scan())),
    )
}

// ----------------------------------------------------------------- mapping

/// Row titles are display strings, so they live here, not in the catalog.
fn row_title(kind: nuvora_library::RowKind) -> &'static str {
    match kind {
        nuvora_library::RowKind::RecentlyAdded => "Recently Added",
        nuvora_library::RowKind::AllMovies => "All Movies",
    }
}

fn scan_status_wire(status: ScanStatus) -> ScanStatusResponse {
    let last = status.last.map(|s| ScanSummaryWire {
        finished_at: unix_ms(s.finished_at),
        duration_ms: s
            .finished_at
            .duration_since(s.started_at)
            .unwrap_or_default()
            .as_millis() as u64,
        added: s.added,
        changed: s.changed,
        moved: s.moved,
        removed: s.removed,
        unchanged: s.unchanged,
        failed: s.failed,
        issues: s
            .issues
            .into_iter()
            .map(|i| ScanIssueWire {
                path: i.path,
                message: i.message,
            })
            .collect(),
    });
    match status.state {
        ScanState::Idle => ScanStatusResponse::Idle { last },
        ScanState::Running { progress, queued } => ScanStatusResponse::Running {
            phase: match progress.phase {
                ScanPhase::Discovering => ScanPhaseWire::Discovering,
                ScanPhase::Reading => ScanPhaseWire::Reading,
                ScanPhase::Artwork => ScanPhaseWire::Artwork,
                ScanPhase::CleaningUp => ScanPhaseWire::CleaningUp,
            },
            done: progress.done,
            total: progress.total,
            queued,
            last,
        },
    }
}

fn unix_ms(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl From<nuvora_library::Home> for HomeResponse {
    fn from(home: nuvora_library::Home) -> Self {
        let rows = home
            .rows
            .into_iter()
            .map(|row| HomeRow {
                kind: row.kind.into(),
                title: row_title(row.kind).to_owned(),
                total: row.total,
                items: row.cards.into_iter().map(PosterCard::from).collect(),
                next: row.next.map(|c| c.to_string()),
            })
            .collect();
        HomeResponse { rows }
    }
}

impl From<nuvora_library::RowKind> for RowKind {
    fn from(k: nuvora_library::RowKind) -> Self {
        match k {
            nuvora_library::RowKind::RecentlyAdded => RowKind::RecentlyAdded,
            nuvora_library::RowKind::AllMovies => RowKind::AllMovies,
        }
    }
}

impl From<nuvora_library::Card> for PosterCard {
    fn from(card: nuvora_library::Card) -> Self {
        PosterCard {
            id: card.id.raw(),
            kind: match card.kind {
                nuvora_library::ItemKind::Movie => ItemKind::Movie,
            },
            title: card.title,
            year: card.year,
            poster: card.poster.map(Image::from),
        }
    }
}

impl From<nuvora_library::Artwork> for Image {
    fn from(art: nuvora_library::Artwork) -> Self {
        Image {
            id: art.id.to_string(),
            aspect_ratio: art.width as f32 / art.height.max(1) as f32,
            placeholder: base64(&art.placeholder.0),
        }
    }
}

impl From<nuvora_library::Item> for ItemDetail {
    fn from(item: nuvora_library::Item) -> Self {
        match item {
            nuvora_library::Item::Movie(movie) => ItemDetail::Movie(MovieDetail {
                id: movie.id.raw(),
                runtime_ms: movie.runtime_ms(),
                title: movie.title,
                year: movie.year,
                overview: movie.overview,
                poster: movie.poster.map(Image::from),
                backdrop: movie.backdrop.map(Image::from),
                versions: movie.versions.into_iter().map(Version::from).collect(),
            }),
        }
    }
}

impl From<nuvora_library::Version> for Version {
    fn from(v: nuvora_library::Version) -> Self {
        let summary = version_summary(&v.media);
        let (media, playback) = match v.media {
            VersionMedia::Probed { info, playback } => {
                let playback = match playback {
                    PlaybackDecision::DirectPlay => Playback::DirectPlay {
                        url: format!("/api/files/{}/stream", v.file_id.raw()),
                        mime_type: info.container.mime_type().to_owned(),
                    },
                    PlaybackDecision::NeedsTranscode(reasons) => Playback::NeedsTranscode {
                        reasons: reasons.iter().map(reason_text).collect(),
                    },
                };
                (Some(MediaDetails::from(info)), playback)
            }
            VersionMedia::Unreadable { reason } => (None, Playback::Unavailable { reason }),
        };
        Version {
            file_id: v.file_id.raw(),
            file_name: v.file_name,
            size_bytes: v.size_bytes,
            summary,
            media,
            playback,
        }
    }
}

impl From<MediaInfo> for MediaDetails {
    fn from(info: MediaInfo) -> Self {
        MediaDetails {
            container: container_name(info.container).to_owned(),
            duration_ms: info.duration_ms,
            bitrate: info.bitrate,
            video: info
                .video
                .into_iter()
                .map(|v| VideoTrack {
                    codec: video_codec_name(&v.codec),
                    profile: v.profile,
                    width: v.width,
                    height: v.height,
                    bit_depth: v.bit_depth,
                    frame_rate: v.frame_rate,
                    hdr: hdr_name(v.hdr),
                })
                .collect(),
            audio: info
                .audio
                .into_iter()
                .map(|a| AudioTrack {
                    index: a.index,
                    codec: audio_codec_name(&a.codec),
                    channels: a.channels,
                    layout: (a.channels > 0).then(|| channel_layout(a.channels)),
                    language: a.language,
                    title: a.title,
                    is_default: a.is_default,
                })
                .collect(),
            subtitles: info
                .subtitles
                .into_iter()
                .map(|s| SubtitleTrack {
                    index: s.index,
                    codec: subtitle_codec_name(&s.codec),
                    language: s.language,
                    title: s.title,
                    is_default: s.is_default,
                    is_forced: s.is_forced,
                })
                .collect(),
        }
    }
}

impl From<DeviceProfileWire> for DeviceProfile {
    /// Unknown codec, container and HDR names are dropped, never errors.
    fn from(wire: DeviceProfileWire) -> Self {
        DeviceProfile {
            containers: wire
                .containers
                .iter()
                .map(|c| Container::from_name(c))
                .filter(|c| *c != Container::Other)
                .collect(),
            video: wire
                .video
                .into_iter()
                .filter_map(|v| {
                    let codec = VideoCodec::from_name(&v.codec);
                    if matches!(codec, VideoCodec::Other(_)) {
                        return None;
                    }
                    let hdr = v
                        .hdr
                        .iter()
                        .filter(|h| matches!(h.as_str(), "hdr10" | "hlg" | "dolby_vision"))
                        .map(|h| HdrFormat::from_name(h, None))
                        .collect();
                    Some(VideoSupport {
                        codec,
                        max_width: v.max_width,
                        max_height: v.max_height,
                        max_bit_depth: v.max_bit_depth,
                        hdr,
                    })
                })
                .collect(),
            audio: wire
                .audio
                .into_iter()
                .filter_map(|a| match AudioCodec::from_name(&a.codec) {
                    AudioCodec::Other(_) => None,
                    codec => Some(AudioSupport {
                        codec,
                        max_channels: a.max_channels,
                    }),
                })
                .collect(),
        }
    }
}

/// `"4K · HEVC Main 10 · HDR10 · TrueHD 7.1"`: resolution, codec (with its
/// profile when it says more than the codec, i.e. above 8 bits), HDR, then the
/// default audio track.
fn version_summary(media: &VersionMedia) -> String {
    let VersionMedia::Probed { info, .. } = media else {
        return "Unreadable file".to_owned();
    };
    let mut parts = Vec::with_capacity(4);
    if let Some(v) = info.video.first() {
        parts.push(match v.height {
            h if h >= 2160 => "4K".to_owned(),
            h if h >= 1080 => "1080p".to_owned(),
            h if h >= 720 => "720p".to_owned(),
            0 => String::new(),
            h => format!("{h}p"),
        });
        let mut codec = video_codec_name(&v.codec);
        if let (Some(profile), true) = (&v.profile, v.bit_depth.is_some_and(|b| b > 8)) {
            codec = format!("{codec} {profile}");
        }
        parts.push(codec);
        parts.extend(hdr_name(v.hdr));
    }
    if let Some(a) = info.default_audio() {
        let layout = if a.channels > 0 {
            format!(" {}", channel_layout(a.channels))
        } else {
            String::new()
        };
        parts.push(format!("{}{layout}", audio_codec_name(&a.codec)));
    }
    parts.retain(|p| !p.is_empty());
    parts.join(" · ")
}

fn reason_text(why: &Incompatibility) -> String {
    match why {
        Incompatibility::Container(c) => format!(
            "{} files aren't supported on this device",
            container_name(*c)
        ),
        Incompatibility::VideoCodec(c) => format!(
            "{} video isn't supported on this device",
            video_codec_name(c)
        ),
        Incompatibility::Resolution { width, height } => {
            format!("{width}×{height} video is larger than this device can decode")
        }
        Incompatibility::BitDepth(bits) => {
            format!("{bits}-bit video isn't supported on this device")
        }
        Incompatibility::Hdr(hdr) => {
            format!(
                "{} isn't supported on this device",
                hdr_name(*hdr).unwrap_or_else(|| "HDR".to_owned())
            )
        }
        Incompatibility::AudioCodec(c) => format!(
            "{} audio isn't supported on this device",
            audio_codec_name(c)
        ),
        Incompatibility::AudioChannels(n) => {
            format!(
                "{} audio has more channels than this device plays",
                channel_layout(*n)
            )
        }
        Incompatibility::Unreadable => "The file could not be read".to_owned(),
    }
}

fn container_name(c: Container) -> &'static str {
    match c {
        Container::Mp4 => "MP4",
        Container::Mov => "QuickTime",
        Container::Matroska => "MKV",
        Container::WebM => "WebM",
        Container::MpegTs => "MPEG-TS",
        Container::Avi => "AVI",
        Container::Other => "Unknown",
    }
}

fn video_codec_name(c: &VideoCodec) -> String {
    match c {
        VideoCodec::H264 => "H.264".into(),
        VideoCodec::Hevc => "HEVC".into(),
        VideoCodec::Av1 => "AV1".into(),
        VideoCodec::Vp9 => "VP9".into(),
        VideoCodec::Vp8 => "VP8".into(),
        VideoCodec::Mpeg2 => "MPEG-2".into(),
        VideoCodec::Mpeg4 => "MPEG-4".into(),
        VideoCodec::Vc1 => "VC-1".into(),
        VideoCodec::Other(name) => name.to_uppercase(),
    }
}

fn audio_codec_name(c: &AudioCodec) -> String {
    match c {
        AudioCodec::Aac => "AAC".into(),
        AudioCodec::Mp3 => "MP3".into(),
        AudioCodec::Opus => "Opus".into(),
        AudioCodec::Vorbis => "Vorbis".into(),
        AudioCodec::Flac => "FLAC".into(),
        AudioCodec::Ac3 => "Dolby Digital".into(),
        AudioCodec::Eac3 => "Dolby Digital Plus".into(),
        AudioCodec::TrueHd => "TrueHD".into(),
        AudioCodec::Dts => "DTS".into(),
        AudioCodec::Pcm => "PCM".into(),
        AudioCodec::Other(name) => name.to_uppercase(),
    }
}

fn subtitle_codec_name(c: &SubtitleCodec) -> String {
    match c {
        SubtitleCodec::Srt => "SRT".into(),
        SubtitleCodec::Ass => "ASS".into(),
        SubtitleCodec::WebVtt => "WebVTT".into(),
        SubtitleCodec::MovText => "MP4 text".into(),
        SubtitleCodec::Pgs => "PGS".into(),
        SubtitleCodec::VobSub => "VobSub".into(),
        SubtitleCodec::Other(name) => name.to_uppercase(),
    }
}

fn hdr_name(hdr: HdrFormat) -> Option<String> {
    match hdr {
        HdrFormat::Sdr => None,
        HdrFormat::Hdr10 => Some("HDR10".into()),
        HdrFormat::Hlg => Some("HLG".into()),
        HdrFormat::DolbyVision { profile, .. } => Some(format!("Dolby Vision P{profile}")),
    }
}

fn channel_layout(channels: u8) -> String {
    match channels {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

/// Standard alphabet with padding, as the shared data layer decodes it.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `"<rev>"`: the ETag of home and row pages.
fn rev_etag(rev: CatalogRev) -> String {
    format!("\"{}\"", rev.get())
}

/// `"<rev>-<profile hash>"`: an item's body depends on the catalog and the device.
fn item_etag(rev: CatalogRev, device: &Device) -> String {
    format!("\"{}-{}\"", rev.get(), device.hash)
}

/// True when the request's `If-None-Match` lists `etag`; the caller answers 304
/// before running any query.
fn if_none_match(request_headers: &HeaderMap, etag: &str) -> bool {
    request_headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|tag| tag.trim())
        .map(|tag| tag.strip_prefix("W/").unwrap_or(tag))
        .any(|tag| tag == etag || tag == "*")
}

/// 200 with `ETag: etag` and `Cache-Control: no-cache` (the client always
/// revalidates; an unchanged catalog costs one small 304).
fn json_with_etag<T: Serialize>(etag: &str, body: &T) -> Response {
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        ETAG,
        HeaderValue::from_str(etag).expect("etags are quoted ascii"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    /// Logged where it happened; the client gets a bare 500.
    Internal,
}

impl ApiError {
    fn internal(e: impl std::fmt::Display) -> ApiError {
        tracing::error!("{e}");
        ApiError::Internal
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m).into_response(),
            ApiError::NotFound => StatusCode::NOT_FOUND.into_response(),
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use nuvora_library::{LibraryConfig, LibrarySpec};
    use nuvora_media::{AudioStream, ColorInfo, MediaTools, VideoStream};
    use std::num::NonZeroUsize;
    use std::time::Duration;

    /// `packages/core/openapi.json` is generated, committed, and the input to the
    /// TypeScript generator. This fails when Rust changed and `bun run gen:api`
    /// was not run, so the client can never silently drift from the server.
    #[test]
    fn committed_openapi_is_current() {
        let (_, spec) = super::router();
        let generated = spec.to_pretty_json().unwrap();
        let committed = include_str!("../../../packages/core/openapi.json");
        assert_eq!(committed.trim(), generated.trim(), "run `bun run gen:api`");
    }

    #[test]
    fn if_none_match_accepts_lists_weak_tags_and_star() {
        let mut headers = HeaderMap::new();
        assert!(!if_none_match(&headers, "\"3\""));
        headers.insert(IF_NONE_MATCH, HeaderValue::from_static("\"1\", W/\"3\""));
        assert!(if_none_match(&headers, "\"3\""));
        assert!(!if_none_match(&headers, "\"2\""));
        headers.insert(IF_NONE_MATCH, HeaderValue::from_static("*"));
        assert!(if_none_match(&headers, "\"anything\""));
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn device_header_maps_names_and_drops_unknown_ones() {
        let wire: DeviceProfileWire = serde_json::from_str(
            r#"{"containers":["mp4","mkv","zip"],
                "video":[{"codec":"hevc","max_width":3840,"max_height":2160,"max_bit_depth":10,"hdr":["hdr10","vivid"]},
                         {"codec":"prores","max_width":1,"max_height":1,"max_bit_depth":8,"hdr":[]}],
                "audio":[{"codec":"aac","max_channels":6},{"codec":"midi","max_channels":1}]}"#,
        )
        .unwrap();
        let profile = DeviceProfile::from(wire);
        assert_eq!(profile.containers, [Container::Mp4, Container::Matroska]);
        assert_eq!(profile.video.len(), 1);
        assert_eq!(
            (profile.video[0].codec.clone(), profile.video[0].hdr.clone()),
            (VideoCodec::Hevc, vec![HdrFormat::Hdr10])
        );
        assert_eq!(profile.audio.len(), 1);
    }

    #[test]
    fn summaries_read_like_a_version_picker() {
        let info = MediaInfo {
            container: Container::Matroska,
            duration_ms: Some(1),
            bitrate: None,
            video: vec![VideoStream {
                index: 0,
                codec: VideoCodec::Hevc,
                profile: Some("Main 10".into()),
                level: None,
                width: 3840,
                height: 2160,
                bit_depth: Some(10),
                frame_rate: None,
                hdr: HdrFormat::Hdr10,
                color: ColorInfo::default(),
                language: None,
                title: None,
                is_default: true,
            }],
            audio: vec![AudioStream {
                index: 1,
                codec: AudioCodec::TrueHd,
                profile: None,
                channels: 8,
                channel_layout: None,
                sample_rate: None,
                language: None,
                title: None,
                is_default: true,
            }],
            subtitles: vec![],
        };
        let media = VersionMedia::Probed {
            info,
            playback: PlaybackDecision::DirectPlay,
        };
        assert_eq!(
            version_summary(&media),
            "4K · HEVC Main 10 · HDR10 · TrueHD 7.1"
        );
        assert_eq!(
            version_summary(&VersionMedia::Unreadable { reason: "x".into() }),
            "Unreadable file"
        );
    }

    async fn app() -> (tempfile::TempDir, axum::Router) {
        let dir = tempfile::tempdir().unwrap();
        let movies = dir.path().join("movies");
        std::fs::create_dir_all(&movies).unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        let library = Library::open(LibraryConfig {
            data_dir: dir.path().join("data"),
            libraries: vec![LibrarySpec {
                key: "movies".into(),
                name: "Movies".into(),
                kind: nuvora_library::LibraryKind::Movies,
                roots: vec![movies],
            }],
            tools,
            probe_concurrency: NonZeroUsize::new(1).unwrap(),
            settle_window: Duration::ZERO,
        })
        .await
        .unwrap();
        let (api, _) = router();
        (
            dir,
            api.with_state(library)
                .fallback(crate::web::serve_app)
                .layer(compression()),
        )
    }

    async fn get(
        app: &axum::Router,
        uri: &str,
        headers: &[(&str, &str)],
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut request = Request::builder().uri(uri);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (parts.status, parts.headers, bytes)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn home_revalidates_with_the_catalog_rev() {
        let (_dir, app) = app().await;
        let (status, headers, body) = get(&app, "/api/home", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get(header::ETAG).unwrap(), "\"0\"");
        assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-cache");
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["rows"][0]["kind"], "recently_added");
        assert_eq!(json["rows"][1]["title"], "All Movies");
        assert_eq!(json["rows"][1]["total"], 0);
        let (status, _, body) = get(&app, "/api/home", &[("if-none-match", "\"0\"")]).await;
        assert_eq!((status, body.len()), (StatusCode::NOT_MODIFIED, 0));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn errors_and_fallbacks() {
        let (_dir, app) = app().await;
        let (status, _, _) = get(&app, "/api/items/1", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _, _) = get(&app, "/api/items/1", &[("nuvora-device", "{not json")]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, _) = get(&app, "/api/rows/all_movies?after=garbage", &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, _) = get(&app, "/api/images/not-an-id/small", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _, _) = get(&app, "/api/files/9/stream", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, headers, body) = get(&app, "/api/nope", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            headers
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "{\"error\":\"not found\"}"
        );
        let (status, _, body) = get(&app, "/movie/1", &[]).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "web app not built in this checkout"
        );
        assert!(String::from_utf8(body).unwrap().contains("bun run build"));
        let (status, _, body) = get(&app, "/api/openapi.json", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert!(serde_json::from_slice::<serde_json::Value>(&body).is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scan_endpoints_report_and_coalesce() {
        let (_dir, app) = app().await;
        let (status, _, body) = get(&app, "/api/scan", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["state"],
            "idle"
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/scan")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let first: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            matches!(first["state"].as_str(), Some("idle" | "running")),
            "{first}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
        let (_, _, body) = get(&app, "/api/scan", &[]).await;
        let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(status["state"], "idle");
        assert_eq!(status["last"]["added"], 0);
        assert!(status["last"]["finished_at"].as_i64().unwrap() > 0);
    }
}
