//! Each user's home screen layout: background, poster size, and which rows
//! in what order with what sort and filter. Saved on the account, so every
//! device and the TV apps show the same home.
//!
//! | method | path          | purpose                                         |
//! |--------|---------------|-------------------------------------------------|
//! | GET    | /api/me/home  | The layout: saved, else the default             |
//! | PUT    | /api/me/home  | Save a layout                                   |
//! | DELETE | /api/me/home  | Back to the default                             |
//!
//! A saved layout keeps every row it has ever had, hidden ones included, so a
//! row the user hid stays hidden. A library added after the layout was saved
//! gets a row at the end; a deleted library's row disappears.

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use nuvora_library::{HomeRowSpec, LibraryId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::browse::{LibrarySort, Resolution, SortOrder, WatchFilter, library_query};
use super::{ApiError, AppState, ErrorBody, JsonBody};
use crate::auth::SignedIn;

/// The library icons a client can choose from. Clients draw them; the
/// server only checks the name.
pub(super) const LIBRARY_ICONS: &[&str] = &[
    "film",
    "tv",
    "anime",
    "sparkles",
    "kids",
    "documentary",
    "music",
    "concert",
    "sports",
    "star",
    "heart",
    "globe",
    "popcorn",
    "ghost",
    "rocket",
    "book",
    "camera",
    "family",
    "trophy",
    "laugh",
    "sword",
    "leaf",
];

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HomeBackground {
    /// The plain canvas.
    #[default]
    None,
    /// A soft wash from the backdrop of what the user is watching.
    Ambient,
    Dusk,
    Ocean,
    Forest,
    Ember,
    Aurora,
    Graphite,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PosterSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HomeRowType {
    ContinueWatching,
    RecentlyAdded,
    Library,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct HomeLayout {
    #[serde(default)]
    pub background: HomeBackground,
    #[serde(default)]
    pub poster_size: PosterSize,
    /// In display order, hidden ones included.
    pub rows: Vec<HomeRowConfig>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct HomeRowConfig {
    /// Chosen by the client, unique in the layout: 1 to 40 letters, digits, `-` or `_`.
    pub id: String,
    #[serde(rename = "type")]
    pub row_type: HomeRowType,
    #[serde(default)]
    pub hidden: bool,
    /// The user's own title; `None` uses the default (the library's name).
    pub title: Option<String>,
    /// Required on `library` rows.
    pub library_id: Option<i64>,
    pub sort: Option<LibrarySort>,
    pub order: Option<SortOrder>,
    pub filter: Option<WatchFilter>,
    pub resolution: Option<Resolution>,
    pub hdr: Option<bool>,
}

impl HomeRowConfig {
    fn plain(id: &str, row_type: HomeRowType, library_id: Option<i64>) -> HomeRowConfig {
        HomeRowConfig {
            id: id.to_owned(),
            row_type,
            hidden: false,
            title: None,
            library_id,
            sort: None,
            order: None,
            filter: None,
            resolution: None,
            hdr: None,
        }
    }
}

fn library_row_id(library: i64) -> String {
    format!("library-{library}")
}

/// The saved layout, cleaned against today's libraries; the default when
/// nothing is saved (or what is saved no longer parses).
pub(super) async fn layout_for(
    state: &AppState,
    signed_in: &SignedIn,
) -> Result<HomeLayout, ApiError> {
    let libraries = state
        .library
        .libraries()
        .await
        .map_err(ApiError::internal)?;
    let ids: Vec<i64> = libraries.iter().map(|l| l.id.raw()).collect();
    let saved = state
        .library
        .home_layout(signed_in.user.id)
        .await
        .map_err(ApiError::internal)?
        .and_then(|json| serde_json::from_str::<HomeLayout>(&json).ok());
    let mut layout = saved.unwrap_or_else(|| HomeLayout {
        background: HomeBackground::default(),
        poster_size: PosterSize::default(),
        rows: vec![
            HomeRowConfig::plain("continue-watching", HomeRowType::ContinueWatching, None),
            HomeRowConfig::plain("recently-added", HomeRowType::RecentlyAdded, None),
        ],
    });
    layout.rows.retain(|r| {
        r.row_type != HomeRowType::Library || r.library_id.is_some_and(|id| ids.contains(&id))
    });
    let present: HashSet<i64> = layout.rows.iter().filter_map(|r| r.library_id).collect();
    for id in ids.into_iter().filter(|id| !present.contains(id)) {
        layout.rows.push(HomeRowConfig::plain(
            &library_row_id(id),
            HomeRowType::Library,
            Some(id),
        ));
    }
    Ok(layout)
}

/// What the library builds: visible rows only.
pub(super) fn specs(layout: &HomeLayout) -> Vec<HomeRowSpec> {
    layout
        .rows
        .iter()
        .filter(|r| !r.hidden)
        .filter_map(|r| {
            let title = r.title.clone();
            Some(match r.row_type {
                HomeRowType::ContinueWatching => HomeRowSpec::ContinueWatching { title },
                HomeRowType::RecentlyAdded => HomeRowSpec::RecentlyAdded { title },
                HomeRowType::Library => HomeRowSpec::Library {
                    library: LibraryId::from_raw(r.library_id?),
                    query: library_query(r.sort, r.order, r.filter, r.resolution, r.hdr),
                    title,
                },
            })
        })
        .collect()
}

fn validate(layout: &HomeLayout) -> Result<(), ApiError> {
    if layout.rows.len() > 60 {
        return Err(ApiError::validation(
            "rows",
            "A home screen holds at most 60 rows",
        ));
    }
    let mut seen = HashSet::new();
    for row in &layout.rows {
        let id_ok = (1..=40).contains(&row.id.len())
            && row
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !id_ok || !seen.insert(row.id.as_str()) {
            return Err(ApiError::validation("rows", "Every row needs its own id"));
        }
        if let Some(title) = &row.title {
            let n = title.trim().chars().count();
            if !(1..=64).contains(&n) || title.chars().any(char::is_control) {
                return Err(ApiError::validation(
                    "title",
                    "Use 1 to 64 characters for a row title",
                ));
            }
        }
        if row.row_type == HomeRowType::Library && row.library_id.is_none() {
            return Err(ApiError::validation(
                "library_id",
                "Choose a library for this row",
            ));
        }
    }
    Ok(())
}

#[utoipa::path(get, path = "/api/me/home", responses((status = 200, body = HomeLayout)))]
pub(super) async fn get_layout(
    State(state): State<AppState>,
    signed_in: SignedIn,
) -> Result<Json<HomeLayout>, ApiError> {
    layout_for(&state, &signed_in).await.map(Json)
}

#[utoipa::path(
    put,
    path = "/api/me/home",
    request_body = HomeLayout,
    responses((status = 200, body = HomeLayout), (status = 400, body = ErrorBody)),
)]
pub(super) async fn put_layout(
    State(state): State<AppState>,
    signed_in: SignedIn,
    JsonBody(mut body): JsonBody<HomeLayout>,
) -> Result<Json<HomeLayout>, ApiError> {
    validate(&body)?;
    for row in &mut body.rows {
        row.title = row.title.as_ref().map(|t| t.trim().to_owned());
    }
    let json = serde_json::to_string(&body).map_err(ApiError::internal)?;
    state
        .library
        .set_home_layout(signed_in.user.id, Some(json))
        .await
        .map_err(ApiError::internal)?;
    layout_for(&state, &signed_in).await.map(Json)
}

#[utoipa::path(delete, path = "/api/me/home", responses((status = 204)))]
pub(super) async fn reset_layout(
    State(state): State<AppState>,
    signed_in: SignedIn,
) -> Result<StatusCode, ApiError> {
    state
        .library
        .set_home_layout(signed_in.user.id, None)
        .await
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
