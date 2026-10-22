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

use axum::Json;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, request::Parts};
use axum::response::{IntoResponse, Response};
use nuvora_library::{CatalogRev, Library, ScanStatus};
use nuvora_media::DeviceProfile;
use serde::{Deserialize, Serialize};
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
-> tower_http::compression::CompressionLayer<impl tower_http::compression::Predicate + Clone> {
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
        let _ = parts;
        todo!(
            "serde_json::from_slice::<DeviceProfileWire> -> DeviceProfile (codec from_name); hash"
        )
    }
}

#[utoipa::path(get, path = "/api/home", responses((status = 200, body = HomeResponse), (status = 304)))]
async fn get_home(State(lib): State<Library>, headers: HeaderMap) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev())) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let home = lib.home().await.map_err(ApiError::internal)?;
    let _ = home;
    todo!("json_with_etag(&rev_etag(home.rev), &HomeResponse::from(home))")
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
    let _ = (lib, row, query, headers);
    todo!(
        "304 on rev; Cursor::parse(row.into(), after) -> 400; lib.row(..); json_with_etag(&rev_etag(page.rev), ..)"
    )
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
    let _ = (lib, id, device, headers);
    todo!(
        "304 on item_etag; lib.item(ItemId::from_raw(id), device.profile) -> 404 | json_with_etag + `Vary: Nuvora-Device`"
    )
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
    let _ = (lib, id, size);
    todo!()
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
    let _ = (lib, id, req);
    todo!()
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
    let _ = status;
    todo!()
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
    let _ = (request_headers, etag);
    todo!()
}

/// 200 with `ETag: etag` and `Cache-Control: no-cache` (the client always
/// revalidates; an unchanged catalog costs one small 304).
fn json_with_etag<T: Serialize>(etag: &str, body: &T) -> Response {
    let _ = (etag, body);
    todo!()
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    Internal(String),
}

impl ApiError {
    fn internal(e: impl std::fmt::Display) -> ApiError {
        tracing::error!("{e}");
        ApiError::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m).into_response(),
            ApiError::NotFound => StatusCode::NOT_FOUND.into_response(),
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
