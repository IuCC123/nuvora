//! Shows and the library page: the routes a TV library and the per-library
//! grid need on top of home and the item detail.
//!
//! | method | path                          | screen / purpose                          |
//! |--------|-------------------------------|-------------------------------------------|
//! | GET    | /api/libraries                | Nav rail: every library with its type     |
//! | GET    | /api/libraries/{id}/items     | Library page grid, keyset paged           |
//! | GET    | /api/libraries/{id}/index     | Library page group headers and jump bar   |
//! | GET    | /api/items/{id}/episodes      | One season's episodes on the show page    |
//!
//! Every route answers for the signed-in user: watch state, the watch filter
//! and the show counts are theirs.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use nuvora_library::Library;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::admin::LibraryKind;
use super::{ApiError, Device, ErrorBody, ErrorCode, Image, PosterCard, Version, watch};
use crate::auth::SignedIn;

/// A library as every member sees it, for the nav rail and the library page header.
#[derive(Serialize, ToSchema)]
pub struct LibrarySummary {
    pub id: i64,
    pub name: String,
    pub kind: LibraryKind,
    /// Movies, or shows for a TV library, with at least one file present.
    pub item_count: u32,
}

/// A show's page. `next` is what Play starts.
#[derive(Serialize, ToSchema)]
pub struct ShowDetail {
    pub id: i64,
    pub title: String,
    /// `"2024"`, or `"2019–2024"` once episodes carry years. `None` when no
    /// year is known.
    pub year_range: Option<String>,
    pub overview: Option<String>,
    pub poster: Option<Image>,
    pub backdrop: Option<Image>,
    /// Numbered seasons in order, then Specials (season 0) last.
    pub seasons: Vec<SeasonSummary>,
    /// The first unwatched episode after the last watched one, in order.
    /// S1 E1 when nothing is watched; `None` when every episode is watched.
    /// Specials never come next.
    pub next: Option<EpisodeRef>,
    /// The signed-in user's watched episodes, out of `episode_count`.
    pub watched_count: u32,
    /// Episodes with a file present, specials included.
    pub episode_count: u32,
}

#[derive(Serialize, ToSchema)]
pub struct SeasonSummary {
    pub id: i64,
    /// 0 for Specials.
    pub index: u32,
    /// `"Season 1"` or `"Specials"`.
    pub title: String,
    pub episode_count: u32,
    pub watched_count: u32,
}

/// `GET /api/items/{id}` of a season id. Its episodes come from
/// `GET /api/items/{id}/episodes`.
#[derive(Serialize, ToSchema)]
pub struct SeasonDetail {
    pub id: i64,
    pub show_id: i64,
    pub show_title: String,
    pub index: u32,
    pub title: String,
    /// The show's.
    pub poster: Option<Image>,
    pub backdrop: Option<Image>,
    pub episode_count: u32,
    pub watched_count: u32,
}

/// A pointer to one episode, for "Play S1 E5" buttons and "next episode".
#[derive(Serialize, ToSchema)]
pub struct EpisodeRef {
    pub id: i64,
    pub season_id: i64,
    /// Season number, 0 for Specials.
    pub season: u32,
    /// Episode number within the season.
    pub index: u32,
    /// From the file name, else `"Episode 5"` (`"Special 2"` in Specials).
    pub title: String,
    /// `"S1 · E5"`, or `"Special 2"`.
    pub label: String,
    /// Where the signed-in user stopped; `None` unless in progress.
    pub position_ms: Option<u64>,
    /// 16:9 frame at 10% of the episode, SDR only.
    pub thumb: Option<Image>,
}

/// One row of a season's episode list.
#[derive(Serialize, ToSchema)]
pub struct Episode {
    pub id: i64,
    pub index: u32,
    /// From the file name, else `"Episode 5"` (`"Special 2"` in Specials).
    pub title: String,
    /// `"S1 · E5"`, or `"Special 2"`.
    pub label: String,
    pub runtime_ms: Option<u64>,
    /// 16:9 frame at 10% of the episode, SDR only.
    pub thumb: Option<Image>,
    pub watch: watch::WatchState,
    /// Best first, each with its playback decision for the requesting device,
    /// as on `MovieDetail`. Empty when every file of the episode is missing.
    pub versions: Vec<Version>,
}

/// `GET /api/items/{id}` of an episode id: what the player and an episode
/// page need, with the show around it.
#[derive(Serialize, ToSchema)]
pub struct EpisodeDetail {
    pub id: i64,
    pub show_id: i64,
    pub season_id: i64,
    pub show_title: String,
    pub season: u32,
    pub index: u32,
    pub title: String,
    pub label: String,
    pub runtime_ms: Option<u64>,
    pub overview: Option<String>,
    /// The show's.
    pub poster: Option<Image>,
    /// The show's.
    pub backdrop: Option<Image>,
    pub thumb: Option<Image>,
    pub versions: Vec<Version>,
    pub watch: watch::WatchState,
    /// The episode after this one in order, for "Next episode" in the player.
    /// `None` after the last one.
    pub next: Option<EpisodeRef>,
}

/// Library page sort. Default `title`.
#[derive(Deserialize, Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySort {
    /// By sort title (`The Matrix` under M), folded for accents.
    Title,
    /// Release year; unknown years last.
    Year,
    /// When the movie, or the show's newest episode, was added.
    Added,
    /// When the signed-in user last finished it; never watched last.
    Watched,
    /// Runtime (a show's total); unknown last.
    Runtime,
}

/// Default `asc` for `title`, `desc` for every other sort.
#[derive(Deserialize, Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Asc,
    Desc,
}

/// Watch filter, for the signed-in user. A show is `watched` when every
/// episode is, `in_progress` when some are watched or started but not all,
/// `unwatched` when none is.
#[derive(Deserialize, Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchFilter {
    All,
    Unwatched,
    InProgress,
    Watched,
}

/// A movie matches when any of its versions does. Ignored for TV libraries.
#[derive(Deserialize, Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// 2160p and up.
    #[serde(rename = "4k")]
    UltraHd,
    #[serde(rename = "1080p")]
    FullHd,
    #[serde(rename = "720p")]
    Hd,
    /// Under 720p.
    #[serde(rename = "sd")]
    Sd,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[expect(dead_code, reason = "contract first; the implementation follows")]
pub struct LibraryItemsQuery {
    pub sort: Option<LibrarySort>,
    pub order: Option<SortOrder>,
    /// Default `all`.
    pub filter: Option<WatchFilter>,
    pub res: Option<Resolution>,
    /// `true`: only movies with an HDR version. Ignored for TV libraries.
    pub hdr: Option<bool>,
    /// `next` from the previous page, or a group's `after` from the index.
    /// Only valid with the same `sort` and `order`; anything else is 400.
    pub after: Option<String>,
    /// Default 60, clamped to 1..=200.
    pub limit: Option<u32>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[expect(dead_code, reason = "contract first; the implementation follows")]
pub struct LibraryIndexQuery {
    pub sort: Option<LibrarySort>,
    pub order: Option<SortOrder>,
    pub filter: Option<WatchFilter>,
    pub res: Option<Resolution>,
    pub hdr: Option<bool>,
}

/// One page of the library grid: movies, or shows for a TV library.
#[derive(Serialize, ToSchema)]
pub struct LibraryPage {
    /// Cards matching the filters, across every page.
    pub total: u32,
    pub items: Vec<PosterCard>,
    /// Cursor for `?after=`; `None` on the last page.
    pub next: Option<String>,
}

/// The library grid's group headers, in grid order. Only groups with items.
/// Sorts without groups (`watched`, `runtime`) answer with no groups.
#[derive(Serialize, ToSchema)]
pub struct LibraryIndex {
    pub groups: Vec<IndexGroup>,
}

#[derive(Serialize, ToSchema)]
pub struct IndexGroup {
    /// Stable: `"#"` and `"A"` to `"Z"` for the title sort, `"1990"` (the
    /// decade) or `"unknown"` for year, `"2026-10"` or `"unknown"` for added.
    pub key: String,
    /// For the header: `"A"`, `"1990s"`, `"October 2026"`, `"Unknown year"`.
    pub label: String,
    pub count: u32,
    /// Pass as `?after=` (with the same sort, order and filters) to load the
    /// page that starts with this group. `None` for the first group.
    pub after: Option<String>,
}

/// Every library, in the order they were added. Any member may list them.
#[utoipa::path(get, path = "/api/libraries", responses((status = 200, body = Vec<LibrarySummary>)))]
pub(super) async fn get_libraries(State(_lib): State<Library>, _me: SignedIn) -> Response {
    not_yet()
}

/// The library page grid. Per user, with the home ETag
/// (`"<catalog_rev>.<watch_rev>"`). A malformed cursor, or one minted for
/// another sort or order, is 400.
#[utoipa::path(
    get,
    path = "/api/libraries/{id}/items",
    params(("id" = i64, Path), LibraryItemsQuery),
    responses(
        (status = 200, body = LibraryPage),
        (status = 304),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_library_items(
    State(_lib): State<Library>,
    _me: SignedIn,
    Path(_id): Path<i64>,
    Query(_query): Query<LibraryItemsQuery>,
    _headers: HeaderMap,
) -> Response {
    not_yet()
}

/// Group headers for the library page: letters for the title sort (`#`,
/// then A to Z), decades for year, months for added. Same filters and ETag
/// as the items.
#[utoipa::path(
    get,
    path = "/api/libraries/{id}/index",
    params(("id" = i64, Path), LibraryIndexQuery),
    responses(
        (status = 200, body = LibraryIndex),
        (status = 304),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_library_index(
    State(_lib): State<Library>,
    _me: SignedIn,
    Path(_id): Path<i64>,
    Query(_query): Query<LibraryIndexQuery>,
    _headers: HeaderMap,
) -> Response {
    not_yet()
}

/// A season's episodes in order, with the signed-in user's watch state and a
/// playback decision per version for the requesting device. 404 unless `id`
/// is a season. ETag as `GET /api/items/{id}`.
#[utoipa::path(
    get,
    path = "/api/items/{id}/episodes",
    params(
        ("id" = i64, Path),
        ("Nuvora-Device" = Option<String>, Header, description = "JSON DeviceProfile"),
    ),
    responses((status = 200, body = Vec<Episode>), (status = 304), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_episodes(
    State(_lib): State<Library>,
    _me: SignedIn,
    Path(_id): Path<i64>,
    _device: Device,
    _headers: HeaderMap,
) -> Response {
    not_yet()
}

fn not_yet() -> Response {
    use axum::response::IntoResponse;
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        ErrorCode::Internal,
        "Not implemented yet",
    )
    .into_response()
}
