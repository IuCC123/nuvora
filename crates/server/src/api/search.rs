//! Search across every library.
//!
//! | method | path              | access |
//! |--------|-------------------|--------|
//! | GET    | /api/search?q=... | member |

use axum::Json;
use axum::extract::{Query, State};
use nuvora_library::Library;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, ErrorBody, Image, PosterCard};
use crate::auth::SignedIn;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// What was typed. Matching is per word, from the start of each word,
    /// ignoring case and accents. At most 200 characters.
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct SearchResponse {
    /// Movies and shows, best match first.
    pub titles: Vec<PosterCard>,
    /// Episodes whose own title matches.
    pub episodes: Vec<SearchEpisode>,
    /// Actors and voice actors, each with what they are in.
    pub people: Vec<SearchPerson>,
}

#[derive(Serialize, ToSchema)]
pub struct SearchEpisode {
    pub id: i64,
    pub show_id: i64,
    pub show_title: String,
    pub season: u32,
    pub episode: u32,
    pub title: String,
    /// The show's poster.
    pub poster: Option<Image>,
}

#[derive(Serialize, ToSchema)]
pub struct SearchPerson {
    pub name: String,
    pub image: Option<Image>,
    pub roles: Vec<SearchRole>,
}

#[derive(Serialize, ToSchema)]
pub struct SearchRole {
    pub card: PosterCard,
    /// Who they play or voice there.
    pub character: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/search",
    params(SearchQuery),
    responses((status = 200, body = SearchResponse), (status = 400, body = ErrorBody)),
)]
pub(super) async fn search(
    State(lib): State<Library>,
    me: SignedIn,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, ApiError> {
    if query.q.chars().count() > 200 {
        return Err(ApiError::validation(
            "q",
            "Search for at most 200 characters",
        ));
    }
    let found = lib
        .search(query.q, me.user.id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SearchResponse {
        titles: found.titles.into_iter().map(PosterCard::from).collect(),
        episodes: found
            .episodes
            .into_iter()
            .map(|e| SearchEpisode {
                id: e.id.raw(),
                show_id: e.show.raw(),
                show_title: e.show_title,
                season: e.season,
                episode: e.episode,
                title: e.title,
                poster: e.poster.map(Image::from),
            })
            .collect(),
        people: found
            .people
            .into_iter()
            .map(|p| SearchPerson {
                name: p.name,
                image: p.image.map(Image::from),
                roles: p
                    .roles
                    .into_iter()
                    .map(|r| SearchRole {
                        card: PosterCard::from(r.card),
                        character: r.character,
                    })
                    .collect(),
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::super::test_app::TestApp;

    #[tokio::test(flavor = "multi_thread")]
    async fn search_finds_titles_by_word_prefix() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        app.add_movies(&["Big Buck Bunny (2008)", "Sintel (2010)", "Amélie (2001)"])
            .await;
        let titles = async |q: &str| -> Vec<String> {
            let reply = app.get(&format!("/api/search?q={q}"), &admin).await;
            assert_eq!(reply.status, StatusCode::OK);
            reply.json()["titles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["title"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(titles("big%20bun").await, ["Big Buck Bunny"]);
        assert_eq!(titles("BUCK").await, ["Big Buck Bunny"]);
        assert_eq!(titles("amelie").await, ["Amélie"]);
        assert_eq!(titles("sin").await, ["Sintel"]);
        assert!(titles("nothing%20here").await.is_empty());
        assert!(titles("%20%20").await.is_empty());
        let long = "a".repeat(201);
        let reply = app.get(&format!("/api/search?q={long}"), &admin).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }
}
