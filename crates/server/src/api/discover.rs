//! Discovery on the wire: the home hero's titles, genre tiles and genre pages.
//!
//! | method | path                 | purpose                                   |
//! |--------|----------------------|-------------------------------------------|
//! | GET    | /api/genres/{genre}  | One genre's movies and shows, newest first |

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use nuvora_library::{HOME_ROW_LIMIT, Library};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::browse::EpisodeRef;
use super::{ApiError, Image, PosterCard, if_none_match, json_with_etag, rev_etag};
use crate::auth::SignedIn;

/// A title in the home hero.
#[derive(Serialize, ToSchema)]
pub struct FeaturedItem {
    /// The title's card; its `backdrop` is always set.
    pub card: PosterCard,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    /// Display names, at most four.
    pub genres: Vec<String>,
    /// 0 to 10.
    pub rating: Option<f32>,
    /// On movies.
    pub runtime_ms: Option<u64>,
    /// On shows: numbered seasons.
    pub season_count: Option<u32>,
    /// On shows: the episode Play starts for the signed-in user.
    pub next: Option<EpisodeRef>,
}

/// A genre on home. `key` opens `GET /api/genres/{key}`.
#[derive(Serialize, ToSchema)]
pub struct GenreTile {
    pub key: String,
    pub name: String,
    /// Movies and shows in the genre.
    pub count: u32,
    /// The backdrop of one of them.
    pub art: Option<Image>,
}

/// A page of one genre.
#[derive(Serialize, ToSchema)]
pub struct GenrePage {
    pub name: String,
    pub total: u32,
    pub items: Vec<PosterCard>,
    /// Pass back as `?after=` for the next page; `None` at the end.
    pub next: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GenreQuery {
    /// `next` from the previous page.
    pub after: Option<String>,
    /// Default 40, clamped to 1..=100.
    pub limit: Option<u32>,
}

impl From<nuvora_library::Featured> for FeaturedItem {
    fn from(f: nuvora_library::Featured) -> Self {
        FeaturedItem {
            card: f.card.into(),
            overview: f.overview,
            tagline: f.tagline,
            genres: f.genres.into_iter().take(4).collect(),
            rating: f.rating,
            runtime_ms: f.runtime_ms,
            season_count: f.season_count,
            next: f.next.map(EpisodeRef::from),
        }
    }
}

impl From<nuvora_library::GenreTile> for GenreTile {
    fn from(t: nuvora_library::GenreTile) -> Self {
        GenreTile {
            key: t.key,
            name: t.name,
            count: t.count,
            art: t.art.map(Image::from),
        }
    }
}

/// Per user (cards carry watch state), with home's ETag. A genre no title
/// has is 404.
#[utoipa::path(
    get,
    path = "/api/genres/{genre}",
    params(("genre" = String, Path), GenreQuery),
    responses((status = 200, body = GenrePage), (status = 304), (status = 404)),
)]
pub(super) async fn get_genre(
    State(lib): State<Library>,
    me: SignedIn,
    Path(genre): Path<String>,
    Query(query): Query<GenreQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if if_none_match(&headers, &rev_etag(lib.rev(), me.watch_rev)) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let (rev, page) = lib
        .genre(
            genre,
            query.after,
            query.limit.unwrap_or(HOME_ROW_LIMIT),
            me.user.id,
        )
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::not_found())?;
    let body = GenrePage {
        name: page.name,
        total: page.total,
        items: page.cards.into_iter().map(PosterCard::from).collect(),
        next: page.next,
    };
    Ok(json_with_etag(&rev_etag(rev, me.watch_rev), &body))
}
