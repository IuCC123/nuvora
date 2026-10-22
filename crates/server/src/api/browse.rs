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

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::VARY;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use nuvora_library::{
    CardNote, EpisodeNumber, GroupKey, ItemId, Library, LibraryCursor, LibraryId, LibraryQuery,
    Order, Sort,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::admin::LibraryKind;
use super::{
    ApiError, Device, ErrorBody, Image, PosterCard, Version, if_none_match, item_etag,
    json_with_etag, rev_etag, watch,
};
use crate::auth::SignedIn;

/// A library as every member sees it, for the nav rail and the library page header.
#[derive(Serialize, ToSchema)]
pub struct LibrarySummary {
    pub id: i64,
    pub name: String,
    pub kind: LibraryKind,
    /// Movies, or shows for a TV library, with at least one file present.
    pub item_count: u32,
    /// A preset icon name; `None` means the default for its kind.
    pub icon: Option<String>,
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
    pub details: super::metadata::Details,
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
pub(super) async fn get_libraries(
    State(lib): State<Library>,
    _me: SignedIn,
) -> Result<Json<Vec<LibrarySummary>>, ApiError> {
    let libraries = lib.libraries().await.map_err(ApiError::internal)?;
    Ok(Json(
        libraries
            .into_iter()
            .map(|l| LibrarySummary {
                id: l.id.raw(),
                name: l.name,
                kind: l.kind.into(),
                item_count: l.item_count,
                icon: l.icon,
            })
            .collect(),
    ))
}

/// Library pages answer 60 cards unless asked otherwise.
const LIBRARY_PAGE: u32 = 60;

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
    State(lib): State<Library>,
    me: SignedIn,
    Path(id): Path<i64>,
    Query(query): Query<LibraryItemsQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev(), me.watch_rev)) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let domain = library_query(query.sort, query.order, query.filter, query.res, query.hdr);
    let after = query
        .after
        .as_deref()
        .map(|raw| LibraryCursor::parse(&domain, raw))
        .transpose()
        .map_err(|_| ApiError::bad_request("invalid cursor for this sort"))?;
    let page = lib
        .library_items(
            LibraryId::from_raw(id),
            domain,
            after,
            query.limit.unwrap_or(LIBRARY_PAGE),
            me.user.id,
        )
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::not_found)?;
    let body = LibraryPage {
        total: page.total,
        items: page.cards.into_iter().map(PosterCard::from).collect(),
        next: page.next.map(|c| c.to_string()),
    };
    Ok(json_with_etag(&rev_etag(page.rev, me.watch_rev), &body))
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
    State(lib): State<Library>,
    me: SignedIn,
    Path(id): Path<i64>,
    Query(query): Query<LibraryIndexQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev(), me.watch_rev)) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let domain = library_query(query.sort, query.order, query.filter, query.res, query.hdr);
    let sort = domain.sort;
    let (rev, groups) = lib
        .library_index(LibraryId::from_raw(id), domain, me.user.id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::not_found)?;
    let body = LibraryIndex {
        groups: groups
            .into_iter()
            .map(|g| {
                let (key, label) = group_names(g.key, sort);
                IndexGroup {
                    key,
                    label,
                    count: g.count,
                    after: g.after.map(|c| c.to_string()),
                }
            })
            .collect(),
    };
    Ok(json_with_etag(&rev_etag(rev, me.watch_rev), &body))
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
        let episodes = lib
            .episodes(ItemId::from_raw(id), device.profile, me.user.id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(ApiError::not_found)?;
        let body: Vec<Episode> = episodes.into_iter().map(Episode::from).collect();
        json_with_etag(&etag, &body)
    };
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Nuvora-Device"));
    Ok(response)
}

pub(super) fn library_query(
    sort: Option<LibrarySort>,
    order: Option<SortOrder>,
    filter: Option<WatchFilter>,
    res: Option<Resolution>,
    hdr: Option<bool>,
) -> LibraryQuery {
    let sort = match sort.unwrap_or(LibrarySort::Title) {
        LibrarySort::Title => Sort::Title,
        LibrarySort::Year => Sort::Year,
        LibrarySort::Added => Sort::Added,
        LibrarySort::Watched => Sort::Watched,
        LibrarySort::Runtime => Sort::Runtime,
    };
    LibraryQuery {
        sort,
        order: match order {
            Some(SortOrder::Asc) => Order::Asc,
            Some(SortOrder::Desc) => Order::Desc,
            None => Order::default_for(sort),
        },
        filter: match filter.unwrap_or(WatchFilter::All) {
            WatchFilter::All => nuvora_library::WatchFilter::All,
            WatchFilter::Unwatched => nuvora_library::WatchFilter::Unwatched,
            WatchFilter::InProgress => nuvora_library::WatchFilter::InProgress,
            WatchFilter::Watched => nuvora_library::WatchFilter::Watched,
        },
        resolution: res.map(|r| match r {
            Resolution::UltraHd => nuvora_library::Resolution::UltraHd,
            Resolution::FullHd => nuvora_library::Resolution::FullHd,
            Resolution::Hd => nuvora_library::Resolution::Hd,
            Resolution::Sd => nuvora_library::Resolution::Sd,
        }),
        hdr: hdr.unwrap_or(false),
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `(key, label)` of a group header.
fn group_names(key: GroupKey, sort: Sort) -> (String, String) {
    match key {
        GroupKey::Letter(c) => (c.to_string(), c.to_string()),
        GroupKey::Decade(d) => (d.to_string(), format!("{d}s")),
        GroupKey::Month { year, month } => (
            format!("{year}-{month:02}"),
            format!("{} {year}", MONTHS[month as usize - 1]),
        ),
        GroupKey::Unknown => (
            "unknown".into(),
            match sort {
                Sort::Year => "Unknown year".into(),
                _ => "Unknown".into(),
            },
        ),
    }
}

// ----------------------------------------------------------------- mapping

/// `"S1 · E5"`, or `"Special 2"` in season 0.
fn episode_label(number: EpisodeNumber) -> String {
    match number.season {
        0 => format!("Special {}", number.episode),
        season => format!("S{season} · E{}", number.episode),
    }
}

/// The file name's episode title, else `"Episode 5"` (`"Special 2"`).
fn episode_title(title: Option<String>, number: EpisodeNumber) -> String {
    title.unwrap_or_else(|| match number.season {
        0 => format!("Special {}", number.episode),
        _ => format!("Episode {}", number.episode),
    })
}

fn season_title(number: u32) -> String {
    match number {
        0 => "Specials".into(),
        n => format!("Season {n}"),
    }
}

/// A card's second line.
pub(super) fn card_subtitle(note: CardNote) -> Option<String> {
    match note {
        CardNote::None => None,
        CardNote::Episode(number) => Some(episode_label(number)),
        CardNote::NextUp(number) => Some(format!("Next episode · {}", episode_label(number))),
        CardNote::NewEpisodes(1) => Some("1 new episode".into()),
        CardNote::NewEpisodes(n) => Some(format!("{n} new episodes")),
    }
}

impl From<nuvora_library::EpisodeRef> for EpisodeRef {
    fn from(e: nuvora_library::EpisodeRef) -> Self {
        EpisodeRef {
            id: e.id.raw(),
            season_id: e.season_id.raw(),
            season: e.number.season,
            index: e.number.episode,
            label: episode_label(e.number),
            title: episode_title(e.title, e.number),
            position_ms: e.watch.resume.map(|r| r.position_ms),
            thumb: e.thumb.map(Image::from),
        }
    }
}

impl From<nuvora_library::Show> for ShowDetail {
    fn from(show: nuvora_library::Show) -> Self {
        ShowDetail {
            id: show.id.raw(),
            title: show.title,
            year_range: show.year.map(|y| y.to_string()),
            overview: show.overview,
            poster: show.poster.map(Image::from),
            backdrop: show.backdrop.map(Image::from),
            seasons: show
                .seasons
                .into_iter()
                .map(|s| SeasonSummary {
                    id: s.id.raw(),
                    index: s.number,
                    title: season_title(s.number),
                    episode_count: s.episode_count,
                    watched_count: s.watched_count,
                })
                .collect(),
            next: show.next.map(EpisodeRef::from),
            watched_count: show.watched_count,
            episode_count: show.episode_count,
            details: show.extras.into(),
        }
    }
}

impl From<nuvora_library::Season> for SeasonDetail {
    fn from(season: nuvora_library::Season) -> Self {
        SeasonDetail {
            id: season.id.raw(),
            show_id: season.show.raw(),
            show_title: season.show_title,
            index: season.number,
            title: season_title(season.number),
            poster: season.poster.map(Image::from),
            backdrop: season.backdrop.map(Image::from),
            episode_count: season.episode_count,
            watched_count: season.watched_count,
        }
    }
}

impl From<nuvora_library::Episode> for EpisodeDetail {
    fn from(e: nuvora_library::Episode) -> Self {
        EpisodeDetail {
            id: e.id.raw(),
            show_id: e.show.raw(),
            season_id: e.season_id.raw(),
            runtime_ms: e.runtime_ms(),
            show_title: e.show_title,
            season: e.number.season,
            index: e.number.episode,
            label: episode_label(e.number),
            title: episode_title(e.title, e.number),
            overview: e.overview,
            poster: e.poster.map(Image::from),
            backdrop: e.backdrop.map(Image::from),
            thumb: e.thumb.map(Image::from),
            versions: e.versions.into_iter().map(Version::from).collect(),
            watch: e.watch.into(),
            next: e.next.map(EpisodeRef::from),
        }
    }
}

impl From<nuvora_library::Episode> for Episode {
    fn from(e: nuvora_library::Episode) -> Self {
        Episode {
            id: e.id.raw(),
            index: e.number.episode,
            runtime_ms: e.runtime_ms(),
            label: episode_label(e.number),
            title: episode_title(e.title, e.number),
            thumb: e.thumb.map(Image::from),
            watch: e.watch.into(),
            versions: e.versions.into_iter().map(Version::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::header::{COOKIE, ETAG, IF_NONE_MATCH};
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};

    use super::super::test_app::TestApp;

    const TOWER: &str = "tv/[Ñ] Tower of God 2 - Ouji no Kikan [1080p]";

    /// An admin, a member, the config's empty Movies library and a TV
    /// library holding three Tower of God episodes and two of Dark.
    async fn tv() -> (TestApp, String, String, i64) {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let episodes: Vec<String> = (1..=3)
            .map(|n| {
                format!(
                    "{TOWER}/[Ñ] Tower of God 2 - Ouji no Kikan - {n:02} [1080p] [ABCD123{n}].mp4"
                )
            })
            .chain([
                "tv/Dark/Season 1/Dark - S01E01 - Secrets.mp4".to_owned(),
                "tv/Dark/Season 1/Dark - S01E02 - Lies.mp4".to_owned(),
            ])
            .collect();
        app.clips(&episodes.iter().map(String::as_str).collect::<Vec<_>>());
        let created = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({"name": "Anime", "kind": "shows", "paths": [app.path("tv")]})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
        assert_eq!(created.json()["kind"], "shows");
        app.scan().await;
        let created_user = app
            .call(
                "POST",
                "/api/admin/users",
                Some(&admin),
                Some(json!({"username": "bo", "display_name": "Bo", "password": "password bo", "role": "member"})),
            )
            .await;
        assert_eq!(created_user.status, StatusCode::CREATED);
        let member = app.login("bo", "password bo").await.cookie();
        (app, admin, member, created.json()["id"].as_i64().unwrap())
    }

    fn titles(page: &Value) -> Vec<&str> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["title"].as_str().unwrap())
            .collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_show_page_lists_seasons_episodes_and_what_plays_next() {
        let (app, admin, member, library) = tv().await;
        let libraries = app.get("/api/libraries", &member).await.json();
        assert_eq!(
            libraries,
            json!([
                {"id": 1, "name": "Movies", "kind": "movies", "item_count": 0, "icon": null},
                {"id": library, "name": "Anime", "kind": "shows", "item_count": 2, "icon": null},
            ])
        );
        let page = app
            .get(&format!("/api/libraries/{library}/items"), &admin)
            .await
            .json();
        assert_eq!(titles(&page), ["Dark", "Tower of God 2 - Ouji no Kikan"]);
        let tower = &page["items"][1];
        assert_eq!(
            (&tower["kind"], &tower["unwatched_count"], &tower["watched"]),
            (&json!("show"), &json!(3), &json!(false))
        );

        let show = app
            .get(&format!("/api/items/{}", tower["id"]), &admin)
            .await
            .json();
        assert_eq!(show["kind"], "show");
        assert_eq!(show["title"], "Tower of God 2 - Ouji no Kikan");
        assert_eq!(
            show["seasons"],
            json!([{"id": show["seasons"][0]["id"], "index": 1, "title": "Season 1", "episode_count": 3, "watched_count": 0}])
        );
        assert_eq!(
            (
                &show["next"]["label"],
                &show["next"]["title"],
                &show["next"]["index"]
            ),
            (&json!("S1 · E1"), &json!("Episode 1"), &json!(1))
        );
        assert!(show["backdrop"]["id"].is_string(), "a frame of episode 1");

        let season = show["seasons"][0]["id"].as_i64().unwrap();
        let episodes = app
            .get(&format!("/api/items/{season}/episodes"), &admin)
            .await;
        assert_eq!(episodes.status, StatusCode::OK);
        let episodes = episodes.json();
        assert_eq!(episodes.as_array().unwrap().len(), 3);
        let e1 = &episodes[0];
        assert_eq!((&e1["index"], &e1["label"]), (&json!(1), &json!("S1 · E1")));
        assert!(e1["thumb"]["id"].is_string());
        assert_eq!(e1["versions"][0]["playback"]["kind"], "direct_play");
        assert_eq!(
            app.get(&format!("/api/items/{}/episodes", tower["id"]), &admin)
                .await
                .status,
            StatusCode::NOT_FOUND,
            "only a season has episodes"
        );

        let watched = app
            .call(
                "PUT",
                &format!("/api/items/{}/watched", e1["id"]),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(watched.status, StatusCode::NO_CONTENT);
        let home = app.get("/api/home", &admin).await.json();
        let next_up = &home["rows"][0];
        assert_eq!(next_up["kind"], "continue_watching");
        assert_eq!(
            (
                &next_up["items"][0]["title"],
                &next_up["items"][0]["subtitle"],
                &next_up["items"][0]["show_id"],
                &next_up["items"][0]["id"],
            ),
            (
                &json!("Tower of God 2 - Ouji no Kikan"),
                &json!("Next episode · S1 · E2"),
                &tower["id"],
                &episodes[1]["id"],
            )
        );
        let episode = app
            .get(&format!("/api/items/{}", episodes[1]["id"]), &admin)
            .await
            .json();
        assert_eq!(
            (
                &episode["kind"],
                &episode["show_title"],
                &episode["next"]["label"]
            ),
            (
                &json!("episode"),
                &json!("Tower of God 2 - Ouji no Kikan"),
                &json!("S1 · E3")
            )
        );
        let season_detail = app
            .get(&format!("/api/items/{season}"), &admin)
            .await
            .json();
        assert_eq!(
            (&season_detail["kind"], &season_detail["watched_count"]),
            (&json!("season"), &json!(1))
        );
        let theirs = app.get("/api/home", &member).await.json();
        assert_ne!(
            theirs["rows"][0]["kind"], "continue_watching",
            "the member has no Next Up"
        );

        let marked = app
            .call(
                "PUT",
                &format!("/api/items/{}/watched", tower["id"]),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(marked.status, StatusCode::NO_CONTENT);
        let show = app
            .get(&format!("/api/items/{}", tower["id"]), &admin)
            .await
            .json();
        assert_eq!(
            (&show["watched_count"], &show["next"]),
            (&json!(3), &Value::Null)
        );
        let recent = &app.get("/api/home", &admin).await.json()["rows"][0];
        assert_eq!(recent["kind"], "recently_added");
        let subtitles: Vec<&Value> = recent["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| &c["subtitle"])
            .collect();
        assert!(
            subtitles.contains(&&json!("3 new episodes")),
            "{subtitles:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_library_page_sorts_filters_pages_and_indexes() {
        let (app, admin, member, library) = tv().await;
        let uri = |q: &str| format!("/api/libraries/{library}/items?{q}");
        let first = app.get(&uri("sort=title&limit=1"), &admin).await;
        let etag = first.headers.get(ETAG).unwrap().clone();
        let first = first.json();
        assert_eq!((titles(&first), &first["total"]), (vec!["Dark"], &json!(2)));
        let next = first["next"].as_str().unwrap();
        let second = app
            .get(&uri(&format!("sort=title&limit=1&after={next}")), &admin)
            .await
            .json();
        assert_eq!(
            (titles(&second), &second["next"]),
            (vec!["Tower of God 2 - Ouji no Kikan"], &Value::Null)
        );
        assert_eq!(
            app.get(&uri(&format!("sort=year&after={next}")), &admin)
                .await
                .status,
            StatusCode::BAD_REQUEST,
            "a cursor belongs to its sort"
        );
        assert_eq!(
            app.get("/api/libraries/999/items", &admin).await.status,
            StatusCode::NOT_FOUND
        );
        let revalidated = app
            .send(
                Request::builder()
                    .uri(uri("sort=title&limit=1"))
                    .header(COOKIE, &admin)
                    .header(IF_NONE_MATCH, etag)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(revalidated.status, StatusCode::NOT_MODIFIED);

        let index = app
            .get(
                &format!("/api/libraries/{library}/index?sort=title"),
                &member,
            )
            .await
            .json();
        assert_eq!(
            index,
            json!({"groups": [
                {"key": "D", "label": "D", "count": 1, "after": null},
                {"key": "T", "label": "T", "count": 1, "after": next},
            ]})
        );
        let months = app
            .get(
                &format!("/api/libraries/{library}/index?sort=added"),
                &member,
            )
            .await
            .json();
        assert_eq!(months["groups"].as_array().unwrap().len(), 1);
        assert!(
            months["groups"][0]["label"]
                .as_str()
                .unwrap()
                .ends_with(" 2026"),
            "{months}"
        );

        let dark =
            app.get(&uri("sort=title&limit=1"), &admin).await.json()["items"][0]["id"].clone();
        app.call(
            "PUT",
            &format!("/api/items/{dark}/watched"),
            Some(&admin),
            None,
        )
        .await;
        let watched = app.get(&uri("filter=watched"), &admin).await.json();
        assert_eq!(titles(&watched), ["Dark"]);
        let unwatched = app.get(&uri("filter=unwatched"), &admin).await.json();
        assert_eq!(titles(&unwatched), ["Tower of God 2 - Ouji no Kikan"]);
        let theirs = app.get(&uri("filter=watched"), &member).await.json();
        assert_eq!(theirs["total"], 0, "the admin's marks are not the member's");

        let row = app
            .get(
                &format!("/api/rows/library?library={library}&limit=1"),
                &admin,
            )
            .await
            .json();
        assert_eq!(titles(&row), ["Dark"]);
        assert_eq!(
            app.get("/api/rows/library", &admin).await.status,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inspecting_a_folder_suggests_its_type() {
        let (app, admin, member, _) = tv().await;
        app.clips(&["films/Heat (1995)/Heat (1995).mp4"]);
        let inspect =
            |path: std::path::PathBuf| format!("/api/admin/fs/inspect?path={}", path.display());
        let tv = app.get(&inspect(app.path("tv")), &admin).await.json();
        assert_eq!(
            tv,
            json!({"video_files": 5, "episode_like": 5, "suggested_kind": "shows", "truncated": false})
        );
        let films = app.get(&inspect(app.path("films")), &admin).await.json();
        assert_eq!(
            (&films["video_files"], &films["suggested_kind"]),
            (&json!(1), &json!("movies"))
        );
        assert_eq!(
            app.get(&inspect(app.path("nope")), &admin).await.status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            app.get(&inspect(app.path("tv")), &member).await.status,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn changing_a_library_type_rebuilds_it() {
        let (app, admin, _, library) = tv().await;
        let changed = app
            .call(
                "PATCH",
                &format!("/api/admin/libraries/{library}"),
                Some(&admin),
                Some(json!({"kind": "movies"})),
            )
            .await;
        assert_eq!(changed.status, StatusCode::OK);
        assert_eq!(
            (&changed.json()["kind"], &changed.json()["item_count"]),
            (&json!("movies"), &json!(5))
        );
        let page = app
            .get(&format!("/api/libraries/{library}/items"), &admin)
            .await
            .json();
        assert!(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["kind"] == "movie")
        );
    }
}
