//! The HTTP API: wire types, handlers, and the OpenAPI document generated from
//! them. Wire types exist only in this module and its children (`accounts`,
//! `admin`); they are built from the catalog's domain read models, and the
//! TypeScript client is generated from the spec.
//!
//! One request per screen:
//!
//! | method | path                        | screen / purpose                         |
//! |--------|-----------------------------|------------------------------------------|
//! | GET    | /api/home                   | Home (first page of each row)            |
//! | GET    | /api/rows/{row}             | Later pages of one home row              |
//! | GET    | /api/items/{id}             | Movie detail (and the Player)            |
//! | PUT    | /api/items/{id}/progress    | Report where playback is                 |
//! | PUT    | /api/items/{id}/watched     | Mark watched                             |
//! | DELETE | /api/items/{id}/watched     | Mark unwatched                           |
//! | GET    | /api/images/{id}/{size}     | Artwork, immutable                       |
//! | GET    | /api/files/{id}/stream      | Direct play, HTTP Range                  |
//! | GET    | /api/scan                   | Scan status                              |
//! | POST   | /api/scan                   | Request a scan (admin; coalescing)       |
//! | GET    | /api/openapi.json           | The spec                                 |
//!
//! Accounts, admin and watch routes live in [`accounts`], [`admin`] and
//! [`watch`]. Access is set by the group a route is registered in
//! ([`Access`]), which decides both the middleware and the spec's `security`,
//! so the two cannot disagree.
//!
//! Every error body is [`ErrorBody`]. State-changing requests (anything but
//! GET/HEAD/OPTIONS) must send `Content-Type: application/json`, even with an
//! empty body, and when they carry an `Origin` it must match the `Host`.

mod accounts;
mod admin;
#[cfg(test)]
mod test_app;
mod watch;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use axum::Json;
use axum::extract::{FromRef, FromRequest, FromRequestParts, Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode, request::Parts};
use axum::response::{IntoResponse, Response};
use nuvora_library::{
    ArtworkId, CatalogRev, Cursor, FileId, HOME_ROW_LIMIT, ItemId, Library, ScanPhase, ScanState,
    ScanStatus, VersionMedia, WatchRev,
};
use nuvora_media::{
    AudioCodec, AudioSupport, Container, DeviceProfile, HdrFormat, Incompatibility, MediaInfo,
    MediaTools, PlaybackDecision, SubtitleCodec, VideoCodec, VideoSupport,
};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::ServeFile;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityRequirement, SecurityScheme};
use utoipa::openapi::{ContentBuilder, Ref, ResponseBuilder};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::auth::SignedIn;

#[derive(OpenApi)]
#[openapi(
    info(title = "Nuvora", version = "1"),
    components(schemas(DeviceProfileWire, ImageSize, ErrorBody))
)]
struct ApiDoc;

/// The session cookie. HttpOnly, so scripts never see the token.
pub const SESSION_COOKIE: &str = "nuvora_session";

/// Who may call a route. Public routes are health, auth state, login and setup.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Member,
    Admin,
}

/// What every handler can reach.
#[derive(Clone)]
pub struct AppState {
    pub library: Library,
    pub auth: Arc<crate::auth::Auth>,
    pub server: Arc<ServerSetup>,
}

/// Facts about this process for the admin panel's General page.
pub struct ServerSetup {
    pub data_dir: PathBuf,
    pub bind: SocketAddr,
    pub tools: MediaTools,
    pub started: Instant,
}

impl FromRef<AppState> for Library {
    fn from_ref(state: &AppState) -> Library {
        state.library.clone()
    }
}

/// All API routes, guarded by their access group, plus `/api/openapi.json`.
pub fn router(state: AppState) -> axum::Router {
    let (router, spec) = build(Some(&state));
    router
        .route(
            "/api/openapi.json",
            axum::routing::get(move || async move { Json(spec) }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::auth::csrf,
        ))
        .with_state(state)
}

/// The spec, built from the same `routes!` calls as the router, so a route
/// cannot exist without being documented (no hand-kept path list).
pub fn openapi() -> utoipa::openapi::OpenApi {
    build(None).1
}

/// `state` is `None` only when printing the spec: the guards need it to run,
/// not to be documented.
fn build(state: Option<&AppState>) -> (axum::Router<AppState>, utoipa::openapi::OpenApi) {
    let public = OpenApiRouter::new()
        .routes(routes!(accounts::health))
        .routes(routes!(accounts::auth_state))
        .routes(routes!(accounts::setup))
        .routes(routes!(accounts::login));
    let mut member = OpenApiRouter::new()
        .routes(routes!(get_home))
        .routes(routes!(get_row))
        .routes(routes!(get_item))
        .routes(routes!(watch::put_progress))
        .routes(routes!(watch::put_watched, watch::delete_watched))
        .routes(routes!(get_image))
        .routes(routes!(stream_file))
        .routes(routes!(get_scan))
        .routes(routes!(accounts::logout))
        .routes(routes!(accounts::update_me));
    let mut admin = OpenApiRouter::new()
        .routes(routes!(admin::list_libraries, admin::create_library))
        .routes(routes!(admin::update_library, admin::delete_library))
        .routes(routes!(admin::scan_library))
        .routes(routes!(post_scan))
        .routes(routes!(admin::list_dirs))
        .routes(routes!(admin::list_users, admin::create_user))
        .routes(routes!(admin::update_user, admin::delete_user))
        .routes(routes!(admin::get_server, admin::update_server));
    document_access(member.get_openapi_mut(), Access::Member);
    document_access(admin.get_openapi_mut(), Access::Admin);
    if let Some(state) = state {
        use axum::middleware::from_fn_with_state;
        member = member.route_layer(from_fn_with_state(
            state.clone(),
            crate::auth::require_member,
        ));
        admin = admin.route_layer(from_fn_with_state(
            state.clone(),
            crate::auth::require_admin,
        ));
    }
    let (router, mut spec) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(public)
        .merge(member)
        .merge(admin)
        .split_for_parts();
    let components = spec.components.get_or_insert_with(Default::default);
    components.add_security_scheme(
        "session",
        SecurityScheme::ApiKey(utoipa::openapi::security::ApiKey::Cookie(
            utoipa::openapi::security::ApiKeyValue::new(SESSION_COOKIE),
        )),
    );
    components.add_security_scheme(
        "bearer",
        SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
    );
    (router, spec)
}

/// Marks every operation in `spec` as needing a session (cookie or bearer) and
/// documents the 401, plus the 403 for admin routes.
fn document_access(spec: &mut utoipa::openapi::OpenApi, access: Access) {
    let error = |description: &str| {
        ResponseBuilder::new()
            .description(description)
            .content(
                "application/json",
                ContentBuilder::new()
                    .schema(Some(Ref::from_schema_name("ErrorBody")))
                    .build(),
            )
            .build()
    };
    for item in spec.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.post,
            &mut item.put,
            &mut item.patch,
            &mut item.delete,
        ];
        for op in operations.into_iter().flatten() {
            op.security = Some(vec![
                SecurityRequirement::new("session", Vec::<String>::new()),
                SecurityRequirement::new("bearer", Vec::<String>::new()),
            ]);
            let responses = &mut op.responses.responses;
            responses.insert("401".into(), error("No valid session").into());
            if access == Access::Admin {
                responses.insert("403".into(), error("Not an admin").into());
            }
        }
    }
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
    /// The signed-in user's movies in progress, most recently watched first.
    /// On home only when it has items.
    ContinueWatching,
    RecentlyAdded,
    AllMovies,
}

impl From<RowKind> for nuvora_library::RowKind {
    fn from(k: RowKind) -> Self {
        match k {
            RowKind::ContinueWatching => nuvora_library::RowKind::ContinueWatching,
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
    /// How far the signed-in user is into the movie, 0 to 1. `None` unless
    /// it is in progress.
    pub progress: Option<f32>,
    /// The signed-in user has finished the movie at least once.
    pub watched: bool,
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
    pub watch: watch::WatchState,
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
    /// Files the scan could not read. Empty unless the signed-in user is an admin.
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
                ApiError::bad_request(format!("Nuvora-Device header is not a device profile: {e}"))
            })?;
            DeviceProfile::from(wire)
        };
        Ok(Device { profile, hash })
    }
}

/// Per user: cards carry the signed-in user's progress, and the ETag is
/// `"<catalog_rev>.<watch_rev>"`, so one user's progress never makes another
/// user's copy stale.
#[utoipa::path(get, path = "/api/home", responses((status = 200, body = HomeResponse), (status = 304)))]
async fn get_home(
    State(lib): State<Library>,
    me: SignedIn,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev(), me.watch_rev)) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let home = lib.home(me.user.id).await.map_err(ApiError::internal)?;
    Ok(json_with_etag(
        &rev_etag(home.rev, me.watch_rev),
        &HomeResponse::from(home),
    ))
}

/// Keyset paging on the row's sort key. A malformed cursor, or one from
/// another row, is 400. Per user, with the same ETag as home.
#[utoipa::path(
    get,
    path = "/api/rows/{row}",
    params(("row" = RowKind, Path), RowQuery),
    responses((status = 200, body = RowPage), (status = 304), (status = 400)),
)]
async fn get_row(
    State(lib): State<Library>,
    me: SignedIn,
    Path(row): Path<RowKind>,
    Query(query): Query<RowQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev(), me.watch_rev)) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let kind: nuvora_library::RowKind = row.into();
    let after = query
        .after
        .as_deref()
        .map(|raw| Cursor::parse(kind, raw))
        .transpose()
        .map_err(|_| ApiError::bad_request("invalid cursor for this row"))?;
    let page = lib
        .row(
            kind,
            after,
            query.limit.unwrap_or(HOME_ROW_LIMIT),
            me.user.id,
        )
        .await
        .map_err(ApiError::internal)?;
    let body = RowPage {
        items: page.cards.into_iter().map(PosterCard::from).collect(),
        next: page.next.map(|c| c.to_string()),
    };
    Ok(json_with_etag(&rev_etag(page.rev, me.watch_rev), &body))
}

/// Per user: `watch` is the signed-in user's state. The ETag is
/// `"<catalog_rev>.<watch_rev>-<device profile hash>"`.
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
    me: SignedIn,
    Path(id): Path<i64>,
    device: Device,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let etag = item_etag(lib.rev(), me.watch_rev, &device);
    let mut response = if if_none_match(&headers, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let item = lib
            .item(ItemId::from_raw(id), device.profile, me.user.id)
            .await
            .map_err(ApiError::internal)?;
        json_with_etag(&etag, &ItemDetail::from(item.ok_or(ApiError::not_found())?))
    };
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Nuvora-Device"));
    Ok(response)
}

/// Reads the WebP from the store (tens of KB) and answers with
/// `Cache-Control: private, max-age=31536000, immutable` (private: it needs a
/// session, so a shared cache must not hand it to anyone else). Bad ids are 404, and
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
    let id: ArtworkId = id.parse().map_err(|_| ApiError::not_found())?;
    let path = lib
        .artwork_path(id, size.into())
        .ok_or(ApiError::not_found())?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::not_found())?;
    let headers = [
        (CONTENT_TYPE, "image/webp"),
        (CACHE_CONTROL, "private, max-age=31536000, immutable"),
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
        .ok_or(ApiError::not_found())?;
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

/// `issues` name server paths, so only admins see them.
#[utoipa::path(get, path = "/api/scan", responses((status = 200, body = ScanStatusResponse)))]
async fn get_scan(State(lib): State<Library>, me: SignedIn) -> Json<ScanStatusResponse> {
    let mut status = scan_status_wire(lib.scan_status());
    if me.user.role != nuvora_library::Role::Admin {
        let (ScanStatusResponse::Idle { last } | ScanStatusResponse::Running { last, .. }) =
            &mut status;
        if let Some(last) = last {
            last.issues.clear();
        }
    }
    Json(status)
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
        nuvora_library::RowKind::ContinueWatching => "Continue Watching",
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
            nuvora_library::RowKind::ContinueWatching => RowKind::ContinueWatching,
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
            progress: card
                .watch
                .resume
                .map(|r| (r.position_ms as f64 / r.duration_ms.max(1) as f64).min(1.0) as f32),
            watched: card.watch.watched(),
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
                watch: movie.watch.into(),
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
                        .map(|h| HdrFormat::from_name(h, None, None))
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

/// `"<catalog rev>.<watch rev>"`: the ETag of home and row pages. The watch
/// rev is the signed-in user's, read with their session, so it is never newer
/// than the body it labels.
fn rev_etag(rev: CatalogRev, watch: WatchRev) -> String {
    format!("\"{}.{}\"", rev.get(), watch.get())
}

/// `"<catalog rev>.<watch rev>-<profile hash>"`: an item's body also depends
/// on the device.
fn item_etag(rev: CatalogRev, watch: WatchRev, device: &Device) -> String {
    format!("\"{}.{}-{}\"", rev.get(), watch.get(), device.hash)
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

/// Every API error answers with this body. `code` is stable; `message` is
/// written for people and may change.
#[derive(Serialize, ToSchema, Debug)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    /// The request field the error is about, for inline form validation.
    pub field: Option<String>,
}

/// Clients must treat an unknown code like `bad_request`.
#[derive(Serialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Validation,
    Unauthorized,
    BadCredentials,
    Forbidden,
    NotFound,
    SetupDone,
    UsernameTaken,
    LastAdmin,
    RateLimited,
    Internal,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
    retry_after_s: Option<u64>,
}

impl ApiError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> ApiError {
        ApiError {
            status,
            body: ErrorBody {
                code,
                message: message.into(),
                field: None,
            },
            retry_after_s: None,
        }
    }

    fn bad_request(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message)
    }

    fn validation(field: &str, message: impl Into<String>) -> ApiError {
        let mut error = ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::Validation, message);
        error.body.field = Some(field.to_owned());
        error
    }

    pub(crate) fn not_found() -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "Not found")
    }

    fn conflict(code: ErrorCode, message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::CONFLICT, code, message)
    }

    /// Logged where it happened; the client gets a generic message.
    pub(crate) fn internal(e: impl std::fmt::Display) -> ApiError {
        tracing::error!("{e}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "Something went wrong on the server",
        )
    }

    pub(crate) fn unauthorized() -> ApiError {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthorized,
            "Sign in to continue",
        )
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::FORBIDDEN, ErrorCode::Forbidden, message)
    }

    fn bad_credentials() -> ApiError {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::BadCredentials,
            "Wrong username or password",
        )
    }

    fn rate_limited(retry_after_s: u64) -> ApiError {
        let wait = match retry_after_s {
            0..=1 => "a second".to_owned(),
            2..=90 => format!("{retry_after_s} seconds"),
            s => format!("{} minutes", s.div_ceil(60)),
        };
        let mut error = ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            format!("Too many failed sign-ins. Try again in {wait}."),
        );
        error.retry_after_s = Some(retry_after_s);
        error
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some(seconds) = self.retry_after_s {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

/// `axum::Json` whose rejections (bad JSON, wrong fields) answer with [`ErrorBody`].
pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = ApiError;
    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, ApiError> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(body)| JsonBody(body))
            .map_err(|e| ApiError::bad_request(e.body_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::test_app::TestApp;
    use super::*;
    use axum::http::header;
    use nuvora_media::{AudioStream, ColorInfo, VideoStream};
    use std::time::Duration;

    /// `packages/core/openapi.json` is generated, committed, and the input to the
    /// TypeScript generator. This fails when Rust changed and `bun run gen:api`
    /// was not run, so the client can never silently drift from the server.
    #[test]
    fn committed_openapi_is_current() {
        let generated = super::openapi().to_pretty_json().unwrap();
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

    #[tokio::test(flavor = "multi_thread")]
    async fn home_revalidates_with_the_catalog_rev() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let reply = app.get("/api/home", &cookie).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.headers.get(header::ETAG).unwrap(), "\"0.0\"");
        assert_eq!(
            reply.headers.get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );
        let json = reply.json();
        assert_eq!(json["rows"][0]["kind"], "recently_added");
        assert_eq!(json["rows"][1]["title"], "All Movies");
        assert_eq!(json["rows"][1]["total"], 0);
        let request = axum::http::Request::builder()
            .uri("/api/home")
            .header(header::COOKIE, &cookie)
            .header(header::IF_NONE_MATCH, "\"0.0\"");
        let reply = app
            .send(request.body(axum::body::Body::empty()).unwrap())
            .await;
        assert_eq!(
            (reply.status, reply.body.len()),
            (StatusCode::NOT_MODIFIED, 0)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn errors_and_fallbacks() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let status = |uri: &'static str| {
            let (app, cookie) = (&app, &cookie);
            async move { app.get(uri, cookie).await.status }
        };
        assert_eq!(status("/api/items/1").await, StatusCode::NOT_FOUND);
        let reply = app
            .send(
                axum::http::Request::builder()
                    .uri("/api/items/1")
                    .header(header::COOKIE, &cookie)
                    .header("nuvora-device", "{not json")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.json()["code"], "bad_request");
        assert_eq!(
            status("/api/rows/all_movies?after=garbage").await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status("/api/images/not-an-id/small").await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(status("/api/files/9/stream").await, StatusCode::NOT_FOUND);
        let reply = app.get("/api/nope", &cookie).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert!(
            reply
                .headers
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
        assert_eq!(
            String::from_utf8(reply.body).unwrap(),
            "{\"code\":\"not_found\",\"message\":\"Not found\",\"field\":null}"
        );
        let reply = app.get("/movie/1", &cookie).await;
        let body = String::from_utf8(reply.body).unwrap();
        // The embedded dist depends on whether `bun run build` ran before this test binary compiled.
        match reply.status {
            StatusCode::OK => {
                assert!(body.starts_with("<!doctype html>"))
            }
            StatusCode::SERVICE_UNAVAILABLE => assert!(body.contains("bun run build")),
            other => panic!("client route answered {other}"),
        }
        let reply = app.call("GET", "/api/openapi.json", None, None).await;
        assert_eq!(reply.status, StatusCode::OK);
        reply.json();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scan_endpoints_report_and_coalesce() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let reply = app.get("/api/scan", &cookie).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json()["state"], "idle");
        let reply = app.call("POST", "/api/scan", Some(&cookie), None).await;
        assert_eq!(reply.status, StatusCode::ACCEPTED);
        let first = reply.json();
        assert!(
            matches!(first["state"].as_str(), Some("idle" | "running")),
            "{first}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
        let status = app.get("/api/scan", &cookie).await.json();
        assert_eq!(status["state"], "idle");
        assert_eq!(status["last"]["added"], 0);
        assert!(status["last"]["finished_at"].as_i64().unwrap() > 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_admins_scan_or_see_scan_issues() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let created = app
            .call(
                "POST",
                "/api/admin/users",
                Some(&admin),
                Some(serde_json::json!({"username": "bo", "display_name": "Bo", "password": "password bo", "role": "member"})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED);
        let member = app.login("bo", "password bo").await.cookie();
        let junk = app.path("movies/Junk (2001)/Junk (2001).mp4");
        std::fs::create_dir_all(junk.parent().unwrap()).unwrap();
        std::fs::write(&junk, b"not a movie").unwrap();

        let refused = app.call("POST", "/api/scan", Some(&member), None).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        assert_eq!(refused.json()["code"], "forbidden");
        app.add_movies(&["Big Buck (2008)"]).await;

        let seen = app.get("/api/scan", &admin).await.json();
        assert_eq!(seen["last"]["failed"], 1);
        assert_eq!(
            seen["last"]["issues"][0]["path"],
            "Junk (2001)/Junk (2001).mp4"
        );
        let hidden = app.get("/api/scan", &member).await.json();
        assert_eq!(hidden["last"]["failed"], 1);
        assert_eq!(hidden["last"]["issues"], serde_json::json!([]));
    }
}
