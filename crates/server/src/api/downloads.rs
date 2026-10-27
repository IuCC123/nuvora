//! Downloads on the wire, for admins: find a movie on the indexers, download
//! it, and watch it land in a library ([`crate::downloads`]).
//!
//! | method | path                                | purpose                                       |
//! |--------|-------------------------------------|-----------------------------------------------|
//! | GET    | /api/admin/downloads                | Every download, newest first (poll it)        |
//! | POST   | /api/admin/downloads                | Start one from a magnet or `.torrent` link    |
//! | POST   | /api/admin/downloads/{id}/pause     | Pause                                         |
//! | POST   | /api/admin/downloads/{id}/resume    | Resume, or try a failed one again             |
//! | DELETE | /api/admin/downloads/{id}           | Cancel and delete, or clear a finished one    |
//! | GET    | /api/admin/downloads/search?q=      | Movie releases from every indexer             |
//! | GET    | /api/admin/downloads/settings       | Indexers, target library, engine health       |
//! | PUT    | /api/admin/downloads/settings       | Choose the target library                     |
//! | POST   | /api/admin/downloads/indexers       | Add an indexer (it must answer first)         |
//! | DELETE | /api/admin/downloads/indexers/{id}  | Remove an indexer                             |

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use nuvora_media::{Content, ReleaseName};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, AppState, ErrorBody, JsonBody};
use crate::downloads::{self, AddError, ControlError, Snapshot, TorznabError};

#[derive(Serialize, ToSchema)]
pub struct Download {
    pub id: u64,
    /// The release's name.
    pub title: String,
    /// The movie the name points at, and its year.
    pub movie: String,
    pub year: Option<u16>,
    pub state: DownloadState,
    /// The library it goes into.
    pub library_id: i64,
    /// Unix seconds.
    pub added_at: i64,
    /// Fetched (or, while `checking`, verified) so far. Absent before the
    /// torrent's details arrive and after it is done.
    pub done_bytes: Option<u64>,
    /// The size of the files being fetched (videos and subtitles only).
    pub total_bytes: Option<u64>,
    /// Bytes per second.
    pub down_bps: u64,
    pub up_bps: u64,
    /// Peers connected now.
    pub peers: u32,
    /// Seconds left at the current speed.
    pub eta_s: Option<u64>,
    /// The movie's folder, once `done`.
    pub path: Option<String>,
    pub finished_at: Option<i64>,
    /// Why it stopped, when `failed`.
    pub error: Option<String>,
}

#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    /// Getting the torrent's details (from the indexer, or from peers for a
    /// magnet link).
    Starting,
    /// Verifying what is already on disk, after a restart or a retry.
    Checking,
    Downloading,
    Paused,
    /// Moving the files into the library.
    Moving,
    Done,
    Failed,
}

impl From<Snapshot> for Download {
    fn from(s: Snapshot) -> Self {
        use downloads::State;
        let name = movie_name(&s.title);
        let mut download = Download {
            id: s.id,
            movie: name.title,
            year: name.year,
            title: s.title,
            state: DownloadState::Starting,
            library_id: s.library_id,
            added_at: s.added_at,
            done_bytes: None,
            total_bytes: None,
            down_bps: 0,
            up_bps: 0,
            peers: 0,
            eta_s: None,
            path: None,
            finished_at: None,
            error: None,
        };
        let progress = match s.state {
            State::Starting => None,
            State::Checking(p) => {
                download.state = DownloadState::Checking;
                Some(p)
            }
            State::Downloading(p) => {
                download.state = DownloadState::Downloading;
                Some(p)
            }
            State::Paused(p) => {
                download.state = DownloadState::Paused;
                p
            }
            State::Moving => {
                download.state = DownloadState::Moving;
                None
            }
            State::Done { path, finished_at } => {
                download.state = DownloadState::Done;
                download.path = Some(path.to_string_lossy().into_owned());
                download.finished_at = Some(finished_at);
                None
            }
            State::Failed { error } => {
                download.state = DownloadState::Failed;
                download.error = Some(error);
                None
            }
        };
        if let Some(p) = progress {
            download.done_bytes = Some(p.done_bytes);
            download.total_bytes = Some(p.total_bytes);
            download.peers = p.peers;
            if download.state == DownloadState::Downloading {
                download.down_bps = p.down_bps;
                download.up_bps = p.up_bps;
                download.eta_s = (p.down_bps > 0)
                    .then(|| p.total_bytes.saturating_sub(p.done_bytes) / p.down_bps);
            }
        }
        download
    }
}

/// The movie or show a release name points at, and what else it says.
pub(super) fn movie_name(release: &str) -> ReleaseName {
    ReleaseName::parse(release)
}

#[derive(Deserialize, ToSchema)]
pub struct AddDownloadRequest {
    /// A `magnet:` link, or an `http(s)` link to a `.torrent` file (a
    /// search result's `link`).
    pub link: String,
    /// Shown until it lands; a magnet link's own name otherwise.
    pub title: Option<String>,
}

#[utoipa::path(get, path = "/api/admin/downloads", responses((status = 200, body = Vec<Download>)))]
pub(super) async fn list_downloads(State(state): State<AppState>) -> Json<Vec<Download>> {
    let list = state.downloads.list().await;
    Json(list.into_iter().map(Download::from).collect())
}

#[utoipa::path(
    post,
    path = "/api/admin/downloads",
    request_body = AddDownloadRequest,
    responses((status = 201, body = Download), (status = 400, body = ErrorBody)),
)]
pub(super) async fn add_download(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<AddDownloadRequest>,
) -> Result<(StatusCode, Json<Download>), ApiError> {
    let link = body.link.trim();
    if link.len() > 8 * 1024 || link.chars().any(char::is_control) {
        return Err(ApiError::validation("link", "That link is too long"));
    }
    let title = body.title.map(|t| t.chars().take(300).collect());
    let id = state
        .downloads
        .add(link, title)
        .await
        .map_err(|e| match e {
            AddError::BadLink => ApiError::validation("link", e.to_string()),
            AddError::NoEngine(_)
            | AddError::NoMovieLibrary
            | AddError::NoShowLibrary
            | AddError::NotWritable { .. } => ApiError::refused(e.to_string()),
            AddError::Save(e) => ApiError::internal(e),
        })?;
    let added = state
        .downloads
        .list()
        .await
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(ApiError::not_found)?;
    Ok((StatusCode::CREATED, Json(added.into())))
}

#[utoipa::path(
    post,
    path = "/api/admin/downloads/{id}/pause",
    params(("id" = u64, Path)),
    responses((status = 204), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)),
)]
pub(super) async fn pause_download(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    state.downloads.pause(id).await.map_err(control_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/admin/downloads/{id}/resume",
    params(("id" = u64, Path)),
    responses((status = 204), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)),
)]
pub(super) async fn resume_download(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    state.downloads.resume(id).await.map_err(control_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/admin/downloads/{id}",
    params(("id" = u64, Path)),
    responses((status = 204), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)),
)]
pub(super) async fn remove_download(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    state.downloads.remove(id).await.map_err(control_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn control_error(e: ControlError) -> ApiError {
    match e {
        ControlError::NotFound => ApiError::not_found(),
        ControlError::Finished | ControlError::Moving => ApiError::refused(e.to_string()),
        ControlError::Save(e) => ApiError::internal(e),
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReleaseQuery {
    /// A movie's name, and its year if you know it. At most 200 characters.
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct ReleaseSearch {
    /// Most seeders first.
    pub releases: Vec<ReleaseResult>,
    /// Indexers that didn't answer, and why.
    pub failures: Vec<IndexerFailure>,
}

#[derive(Serialize, ToSchema)]
pub struct ReleaseResult {
    /// The release's name, as the indexer has it.
    pub title: String,
    /// The movie or show the name points at, and its year.
    pub movie: String,
    pub year: Option<u16>,
    /// A movie, some episodes, whole seasons, or every season.
    pub content: ReleaseContent,
    /// ISO 639-1 codes of the audio the name names, in order. Empty when it
    /// names none, which usually means the original language only.
    pub audio_languages: Vec<String>,
    /// ISO 639-1 codes of the subtitles the name names; `"mul"` for many.
    pub subtitle_languages: Vec<String>,
    /// More than one audio track.
    pub dual_audio: bool,
    /// A dubbed soundtrack in place of the original.
    pub dubbed: bool,
    /// Subtitles burned into the picture: they can't be turned off.
    pub hardcoded_subs: bool,
    pub indexer: String,
    /// Pass to `POST /api/admin/downloads`.
    pub link: String,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    /// Unix seconds.
    pub published: Option<i64>,
    /// The release's page on the tracker.
    pub details_url: Option<String>,
    /// From the name: 2160, 1080, 720 or 480.
    pub resolution: Option<u16>,
    pub hdr: Option<ReleaseHdr>,
    pub codec: Option<ReleaseCodec>,
    pub source: Option<ReleaseSource>,
}

#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReleaseContent {
    Movie,
    /// Episodes `first` to `last`. `season` is `null` for episodes numbered
    /// across the whole show, as anime often is.
    Episodes {
        season: Option<u32>,
        first: u32,
        last: u32,
    },
    /// Whole seasons `first` to `last`.
    Seasons {
        first: u32,
        last: u32,
    },
    /// Every season.
    Complete,
}

impl From<Content> for ReleaseContent {
    fn from(c: Content) -> Self {
        match c {
            Content::Movie => ReleaseContent::Movie,
            Content::Episodes {
                season,
                first,
                last,
            } => ReleaseContent::Episodes {
                season,
                first,
                last,
            },
            Content::Seasons { first, last } => ReleaseContent::Seasons { first, last },
            Content::Complete => ReleaseContent::Complete,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseHdr {
    Hdr,
    DolbyVision,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCodec {
    H264,
    Hevc,
    Av1,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseSource {
    Remux,
    Bluray,
    Web,
    Hdtv,
    Dvd,
    /// Filmed in a cinema: poor picture and sound.
    Cam,
}

#[derive(Serialize, ToSchema)]
pub struct IndexerFailure {
    pub indexer: String,
    pub message: String,
}

impl From<downloads::Release> for ReleaseResult {
    fn from(r: downloads::Release) -> Self {
        let name = ReleaseName::parse(&r.title);
        let tags = name.tags;
        let codes = |list: &[&str]| list.iter().map(|&c| c.to_owned()).collect();
        ReleaseResult {
            content: name.content.into(),
            audio_languages: codes(&name.audio),
            subtitle_languages: codes(&name.subtitles),
            dual_audio: name.dual_audio,
            dubbed: name.dubbed,
            hardcoded_subs: name.hardcoded_subs,
            movie: name.title,
            year: name.year,
            title: r.title,
            indexer: r.indexer,
            link: r.link,
            size_bytes: r.size,
            seeders: r.seeders,
            leechers: r.leechers,
            published: r.published,
            details_url: r.details_url,
            resolution: tags.resolution,
            hdr: tags.hdr.map(|h| match h {
                nuvora_media::Hdr::Hdr => ReleaseHdr::Hdr,
                nuvora_media::Hdr::DolbyVision => ReleaseHdr::DolbyVision,
            }),
            codec: tags.codec.map(|c| match c {
                nuvora_media::Codec::H264 => ReleaseCodec::H264,
                nuvora_media::Codec::Hevc => ReleaseCodec::Hevc,
                nuvora_media::Codec::Av1 => ReleaseCodec::Av1,
            }),
            source: tags.source.map(|s| match s {
                nuvora_media::Source::Remux => ReleaseSource::Remux,
                nuvora_media::Source::BluRay => ReleaseSource::Bluray,
                nuvora_media::Source::Web => ReleaseSource::Web,
                nuvora_media::Source::Hdtv => ReleaseSource::Hdtv,
                nuvora_media::Source::Dvd => ReleaseSource::Dvd,
                nuvora_media::Source::Cam => ReleaseSource::Cam,
            }),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/downloads/search",
    params(ReleaseQuery),
    responses((status = 200, body = ReleaseSearch), (status = 400, body = ErrorBody)),
)]
pub(super) async fn search_releases(
    State(state): State<AppState>,
    Query(query): Query<ReleaseQuery>,
) -> Result<Json<ReleaseSearch>, ApiError> {
    let q = query.q.trim();
    if q.is_empty() || q.chars().count() > 200 {
        return Err(ApiError::validation("q", "Search for 1 to 200 characters"));
    }
    let found = state.downloads.search(q).await;
    Ok(Json(ReleaseSearch {
        releases: found
            .releases
            .into_iter()
            .map(ReleaseResult::from)
            .collect(),
        failures: found
            .failures
            .into_iter()
            .map(|(indexer, e)| IndexerFailure {
                indexer,
                message: e.to_string(),
            })
            .collect(),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct DownloadSettings {
    pub indexers: Vec<IndexerInfo>,
    /// The library movies go into. `null`: the first Movies library.
    pub library_id: Option<i64>,
    /// The library episodes and seasons go into. `null`: the first TV Shows
    /// library.
    pub shows_library_id: Option<i64>,
    /// Why this server can't download, when it can't.
    pub unavailable: Option<String>,
}

/// An indexer, without its API key: that never leaves the server.
#[derive(Serialize, ToSchema)]
pub struct IndexerInfo {
    pub id: u32,
    pub name: String,
    pub url: String,
}

/// Both choices, every time: a missing one goes back to the first library
/// of its kind.
#[derive(Deserialize, ToSchema)]
pub struct UpdateDownloadSettings {
    pub library_id: Option<i64>,
    pub shows_library_id: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddIndexerRequest {
    pub name: String,
    /// The Torznab feed: Jackett's "Copy Torznab Feed", or a Prowlarr
    /// indexer's `http://<host>:9696/<id>/api`.
    pub url: String,
    pub api_key: String,
}

#[utoipa::path(get, path = "/api/admin/downloads/settings", responses((status = 200, body = DownloadSettings)))]
pub(super) async fn get_download_settings(State(state): State<AppState>) -> Json<DownloadSettings> {
    let settings = state.downloads.settings().await;
    Json(DownloadSettings {
        indexers: settings
            .indexers
            .into_iter()
            .map(|i| IndexerInfo {
                id: i.id,
                name: i.name,
                url: i.url,
            })
            .collect(),
        library_id: settings.library_id,
        shows_library_id: settings.shows_library_id,
        unavailable: state.downloads.engine_error().map(str::to_owned),
    })
}

#[utoipa::path(
    put,
    path = "/api/admin/downloads/settings",
    request_body = UpdateDownloadSettings,
    responses((status = 200, body = DownloadSettings), (status = 400, body = ErrorBody)),
)]
pub(super) async fn put_download_settings(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<UpdateDownloadSettings>,
) -> Result<Json<DownloadSettings>, ApiError> {
    use nuvora_library::LibraryKind;
    for (field, id, kind, message) in [
        (
            "library_id",
            body.library_id,
            LibraryKind::Movies,
            "Pick a Movies library",
        ),
        (
            "shows_library_id",
            body.shows_library_id,
            LibraryKind::Shows,
            "Pick a TV Shows library",
        ),
    ] {
        if let Some(id) = id {
            let library = state
                .library
                .library(nuvora_library::LibraryId::from_raw(id))
                .await
                .map_err(ApiError::internal)?;
            if !library.is_some_and(|l| l.kind == kind) {
                return Err(ApiError::validation(field, message));
            }
        }
    }
    state
        .downloads
        .set_libraries(body.library_id, body.shows_library_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(get_download_settings(State(state)).await)
}

#[utoipa::path(
    post,
    path = "/api/admin/downloads/indexers",
    request_body = AddIndexerRequest,
    responses((status = 200, body = DownloadSettings), (status = 400, body = ErrorBody)),
)]
pub(super) async fn add_indexer(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<AddIndexerRequest>,
) -> Result<Json<DownloadSettings>, ApiError> {
    let name = body.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err(ApiError::validation(
            "name",
            "Name it in 1 to 60 characters",
        ));
    }
    let url = body.url.trim().to_owned();
    let parsed = reqwest::Url::parse(&url).ok();
    if url.len() > 2048 || !parsed.is_some_and(|u| matches!(u.scheme(), "http" | "https")) {
        return Err(ApiError::validation(
            "url",
            "Use the feed's full address, starting with http:// or https://",
        ));
    }
    let api_key = body.api_key.trim().to_owned();
    if api_key.len() > 512 || api_key.chars().any(char::is_control) {
        return Err(ApiError::validation(
            "api_key",
            "That doesn't look like an API key",
        ));
    }
    let checked = state
        .downloads
        .add_indexer(name, url, api_key)
        .await
        .map_err(ApiError::internal)?;
    if let Err(e) = checked {
        let field = if matches!(e, TorznabError::Refused(_)) {
            "api_key"
        } else {
            "url"
        };
        return Err(ApiError::validation(field, e.to_string()));
    }
    Ok(get_download_settings(State(state)).await)
}

#[utoipa::path(
    delete,
    path = "/api/admin/downloads/indexers/{id}",
    params(("id" = u32, Path)),
    responses((status = 200, body = DownloadSettings), (status = 404, body = ErrorBody)),
)]
pub(super) async fn remove_indexer(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<DownloadSettings>, ApiError> {
    let removed = state
        .downloads
        .remove_indexer(id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(ApiError::not_found());
    }
    Ok(get_download_settings(State(state)).await)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::api::test_app::TestApp;

    const RELEASE: &str = "Duck.and.Cover.1951.480p.WEB.x264-PD";

    /// A second engine seeds a release folder (a video and an `.nfo`); a
    /// one-route tracker points the server at it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_downloaded_movie_lands_in_the_library() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        app.clips(&[&format!("seed/{RELEASE}/{RELEASE}.mp4")]);
        std::fs::write(app.path(&format!("seed/{RELEASE}/{RELEASE}.nfo")), "notes").unwrap();

        let seeder = app.seed(&format!("seed/{RELEASE}")).await;

        let added = app
            .call(
                "POST",
                "/api/admin/downloads",
                Some(&admin),
                Some(json!({ "link": seeder.link, "title": RELEASE })),
            )
            .await;
        assert_eq!(added.status, StatusCode::CREATED);

        let started = Instant::now();
        let finished: Value = loop {
            let list = app.get("/api/admin/downloads", &admin).await.json();
            let download = list[0].clone();
            if matches!(download["state"].as_str(), Some("done" | "failed")) {
                break download;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "stuck: {download}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        };
        assert_eq!(finished["state"], "done", "{finished}");
        let folder = app.path("movies/Duck and Cover (1951)");
        assert_eq!(finished["path"], folder.to_str().unwrap());
        let landed: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(landed, [format!("{RELEASE}.mp4")]);
        assert!(!app.path("movies/.nuvora-downloads").exists());

        app.scan().await;
        let found = app.get("/api/search?q=duck", &admin).await.json();
        assert_eq!(found["titles"][0]["title"], "Duck and Cover");
    }

    /// A season goes to the TV Shows library, in the show's folder with the
    /// torrent's own season folder kept, and its episodes scan.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_downloaded_season_lands_in_the_shows_library() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        std::fs::create_dir_all(app.path("shows")).unwrap();
        let created = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({ "name": "TV", "kind": "shows", "paths": [app.path("shows")] })),
            )
            .await;
        assert_eq!(
            created.status,
            StatusCode::CREATED,
            "{:?}",
            String::from_utf8_lossy(&created.body)
        );
        let pack = "Show.Name.S01.1080p.WEB-TEST";
        app.clips(&[
            &format!("seed/{pack}/Show.Name.S01E01.mkv"),
            &format!("seed/{pack}/Show.Name.S01E02.mkv"),
        ]);
        let seeder = app.seed(&format!("seed/{pack}")).await;

        let added = app
            .call(
                "POST",
                "/api/admin/downloads",
                Some(&admin),
                Some(json!({ "link": seeder.link, "title": pack })),
            )
            .await;
        assert_eq!(added.status, StatusCode::CREATED);
        let started = Instant::now();
        let finished: Value = loop {
            let download = app.get("/api/admin/downloads", &admin).await.json()[0].clone();
            if matches!(download["state"].as_str(), Some("done" | "failed")) {
                break download;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "stuck: {download}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        };
        assert_eq!(finished["state"], "done", "{finished}");
        // The library keeps its folder's real path (`/private/var` on a Mac).
        let shows = std::fs::canonicalize(app.path("shows")).unwrap();
        assert_eq!(finished["path"], shows.join("Show Name").to_str().unwrap());
        assert!(shows.join("Show Name/Show.Name.S01E02.mkv").exists());
        assert!(!app.path("movies/Show Name").exists());

        app.scan().await;
        let found = app.get("/api/search?q=show", &admin).await.json();
        assert_eq!(found["titles"][0]["title"], "Show Name", "{found}");
    }

    /// A box named like a movie goes to TV Shows anyway when its videos are
    /// one show's episodes, in the show's folder, without the creditless opening.
    #[tokio::test(flavor = "multi_thread")]
    async fn episodes_named_like_a_movie_land_in_the_shows_library() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        std::fs::create_dir_all(app.path("shows")).unwrap();
        let created = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({ "name": "TV", "kind": "shows", "paths": [app.path("shows")] })),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED);
        let pack = "Lain BD Box 1080p";
        let episode =
            |n: &str| format!("[Kanavid] Serial Experiments Lain {n} [BD][1080p][AAC].mp4");
        app.clips(&[
            &format!("seed/{pack}/{}", episode("- 01")),
            &format!("seed/{pack}/{}", episode("- 02")),
            &format!("seed/{pack}/{}", episode("NCOP")),
        ]);
        let seeder = app.seed(&format!("seed/{pack}")).await;

        let added = app
            .call(
                "POST",
                "/api/admin/downloads",
                Some(&admin),
                Some(json!({ "link": seeder.link, "title": pack })),
            )
            .await;
        assert_eq!(added.status, StatusCode::CREATED);
        let started = Instant::now();
        let finished: Value = loop {
            let download = app.get("/api/admin/downloads", &admin).await.json()[0].clone();
            if matches!(download["state"].as_str(), Some("done" | "failed")) {
                break download;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "stuck: {download}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        };
        assert_eq!(finished["state"], "done", "{finished}");
        let shows = std::fs::canonicalize(app.path("shows")).unwrap();
        let folder = shows.join("Serial Experiments Lain");
        assert_eq!(finished["path"], folder.to_str().unwrap());
        assert_eq!(finished["library_id"], created.json()["id"]);
        let mut landed: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        landed.sort();
        assert_eq!(landed, [episode("- 01"), episode("- 02")]);
        assert!(!app.path("movies/Lain BD Box").exists());

        app.scan().await;
        let found = app.get("/api/search?q=lain", &admin).await.json();
        assert_eq!(
            found["titles"][0]["title"], "Serial Experiments Lain",
            "{found}"
        );
    }

    #[tokio::test]
    async fn downloads_are_for_admins_and_explain_a_missing_engine() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;

        let settings = app.get("/api/admin/downloads/settings", &admin).await;
        assert_eq!(settings.status, StatusCode::OK);
        assert_eq!(
            settings.json(),
            json!({
                "indexers": [],
                "library_id": null,
                "shows_library_id": null,
                "unavailable": "it is turned off"
            })
        );

        let added = app
            .call("POST", "/api/admin/downloads", Some(&admin), Some(json!({ "link": "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567" })))
            .await;
        assert_eq!(added.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            added.json()["message"],
            "Downloading isn't available: it is turned off"
        );

        let search = app.get("/api/admin/downloads/search?q=heat", &admin).await;
        assert_eq!(search.json(), json!({ "releases": [], "failures": [] }));

        let list = app.get("/api/admin/downloads", &admin).await;
        assert_eq!(list.json(), json!([]));
    }

    /// Docker mounts media read-only by default; a download Nuvora can't
    /// write is refused up front, naming the folder. A file where the staging
    /// folder goes stands in for read-only, because CI runs tests as root.
    #[tokio::test]
    async fn a_library_it_cannot_write_to_refuses_downloads() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        let movies = app.path("movies");
        std::fs::write(movies.join(".nuvora-downloads"), "in the way").unwrap();
        let reply = app
            .call(
                "POST",
                "/api/admin/downloads",
                Some(&admin),
                Some(json!({ "link": "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567" })),
            )
            .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(
            message.starts_with(&format!("Nuvora can't write to {}: ", movies.display())),
            "{message}"
        );
        assert_eq!(
            app.get("/api/admin/downloads", &admin).await.json(),
            json!([])
        );
    }

    #[tokio::test]
    async fn an_indexer_that_does_not_answer_is_not_saved() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let reply = app
            .call(
                "POST",
                "/api/admin/downloads/indexers",
                Some(&admin),
                Some(json!({ "name": "Jackett", "url": "http://127.0.0.1:9/torznab", "api_key": "k" })),
            )
            .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.json()["field"], "url");
        let settings = app.get("/api/admin/downloads/settings", &admin).await;
        assert_eq!(settings.json()["indexers"], json!([]));

        let bad = app
            .call(
                "POST",
                "/api/admin/downloads/indexers",
                Some(&admin),
                Some(json!({ "name": "x", "url": "ftp://example.com", "api_key": "" })),
            )
            .await;
        assert_eq!(bad.json()["field"], "url");
    }
}
