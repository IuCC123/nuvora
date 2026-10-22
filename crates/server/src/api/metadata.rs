//! Online metadata on the wire: the details block on movie and show pages,
//! and the admin settings for providers.
//!
//! | method | path                           | purpose                                   |
//! |--------|--------------------------------|-------------------------------------------|
//! | GET    | /api/admin/metadata            | Provider settings (is a TMDB key saved)   |
//! | PUT    | /api/admin/metadata            | Save or clear the TMDB key                |
//! | POST   | /api/admin/metadata/refresh    | Look every movie and show up again        |

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, AppState, ErrorBody, Image, JsonBody};

/// What AniList (anime), TVmaze (TV) or TMDB (movies) know about a title.
/// Every field is empty until the server has matched it online.
#[derive(Serialize, ToSchema)]
pub struct Details {
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    /// 0 to 10.
    pub rating: Option<f32>,
    pub studio: Option<String>,
    /// The title's page at the provider.
    pub external_url: Option<String>,
    /// `"AniList"`, `"TVmaze"` or `"TMDB"`.
    pub source: Option<String>,
    /// Billing order. Anime lists each character twice when both dubs are
    /// known: once with the Japanese voice actor, once with the English one.
    pub cast: Vec<CastMember>,
}

#[derive(Serialize, ToSchema)]
pub struct CastMember {
    pub kind: CreditKind,
    /// The actor or voice actor.
    pub name: String,
    pub character: Option<String>,
    /// For voice actors: the dub, e.g. `"Japanese"`.
    pub language: Option<String>,
    pub image: Option<Image>,
    pub character_image: Option<Image>,
}

#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum CreditKind {
    Cast,
    Voice,
}

impl From<nuvora_library::Extras> for Details {
    fn from(e: nuvora_library::Extras) -> Self {
        Details {
            original_title: e.original_title,
            tagline: e.tagline,
            genres: e.genres,
            rating: e.rating,
            studio: e.studio,
            external_url: e.external_url,
            source: e.source.map(str::to_owned),
            cast: e
                .credits
                .into_iter()
                .map(|c| CastMember {
                    kind: if c.kind == "voice" {
                        CreditKind::Voice
                    } else {
                        CreditKind::Cast
                    },
                    name: c.name,
                    character: c.character,
                    language: c.language,
                    image: c.image.map(Image::from),
                    character_image: c.character_image.map(Image::from),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MetadataSettings {
    /// Anime (AniList) and TV (TVmaze) need no key. Movies need a TMDB key.
    pub tmdb_key_set: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMetadataRequest {
    /// A TMDB API key or API Read Access Token. Empty or `null` clears it.
    pub tmdb_api_key: Option<String>,
}

#[utoipa::path(get, path = "/api/admin/metadata", responses((status = 200, body = MetadataSettings)))]
pub(super) async fn get_metadata(
    State(state): State<AppState>,
) -> Result<Json<MetadataSettings>, ApiError> {
    let tmdb_key_set = state
        .library
        .tmdb_key_set()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MetadataSettings { tmdb_key_set }))
}

#[utoipa::path(
    put,
    path = "/api/admin/metadata",
    request_body = UpdateMetadataRequest,
    responses((status = 200, body = MetadataSettings), (status = 400, body = ErrorBody)),
)]
pub(super) async fn update_metadata(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<UpdateMetadataRequest>,
) -> Result<Json<MetadataSettings>, ApiError> {
    if let Some(key) = &body.tmdb_api_key
        && (key.len() > 512 || key.chars().any(char::is_control))
    {
        return Err(ApiError::validation(
            "tmdb_api_key",
            "That doesn't look like a TMDB key",
        ));
    }
    state
        .library
        .set_tmdb_key(body.tmdb_api_key)
        .await
        .map_err(ApiError::internal)?;
    get_metadata(State(state)).await
}

#[utoipa::path(post, path = "/api/admin/metadata/refresh", responses((status = 202)))]
pub(super) async fn refresh_metadata(
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state
        .library
        .refresh_metadata()
        .await
        .map_err(ApiError::internal)?;
    Ok(StatusCode::ACCEPTED)
}
