//! Streaming copies (see [`crate::copies`]): where a heavy video's 1080p copy
//! stands, and the finished copy as HLS, served straight from disk.
//!
//! | method | path                               | access |
//! |--------|------------------------------------|--------|
//! | GET    | /api/files/{id}/copy               | member |
//! | GET    | /api/files/{id}/copy/main.m3u8     | member |
//! | GET    | /api/files/{id}/copy/init.mp4      | member |
//! | GET    | /api/files/{id}/copy/{segment}     | member |

use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use nuvora_library::FileId;
use nuvora_media::INIT_SEGMENT;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, AppState, ErrorBody};
use crate::copies::{Copies, CopyState};

#[derive(Serialize, ToSchema)]
pub struct Copy {
    #[serde(flatten)]
    pub state: CopyState,
    /// The copy's playlist once it is ready: HLS, H.264 and AAC.
    pub url: Option<String>,
}

/// Where the file's streaming copy stands. Asking moves it to the front of
/// the queue.
#[utoipa::path(
    get,
    path = "/api/files/{id}/copy",
    params(("id" = i64, Path)),
    responses((status = 200, body = Copy), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_copy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Copy>, ApiError> {
    let copy = state.copies.state(FileId::from_raw(id));
    let url =
        matches!(copy, CopyState::Ready { .. }).then(|| format!("/api/files/{id}/copy/main.m3u8"));
    Ok(Json(Copy { state: copy, url }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CopyQuery {
    /// Where playback starts, in seconds (`EXT-X-START`).
    pub start: Option<f64>,
}

/// The finished copy's playlist, VOD, segment URIs relative.
#[utoipa::path(
    get,
    path = "/api/files/{id}/copy/main.m3u8",
    params(("id" = i64, Path), CopyQuery),
    responses(
        (status = 200, content_type = "application/vnd.apple.mpegurl", body = String),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_copy_playlist(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<CopyQuery>,
) -> Result<Response, ApiError> {
    let dir = ready_dir(&state, id).await?;
    let body = Copies::playlist(&dir, query.start).ok_or(ApiError::not_found())?;
    let headers = [
        (CONTENT_TYPE, "application/vnd.apple.mpegurl"),
        (CACHE_CONTROL, "private, no-cache"),
    ];
    Ok((headers, body).into_response())
}

/// The copy's fMP4 init segment.
#[utoipa::path(
    get,
    path = "/api/files/{id}/copy/init.mp4",
    params(("id" = i64, Path)),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_copy_init(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let dir = ready_dir(&state, id).await?;
    serve(dir.join(INIT_SEGMENT)).await
}

/// Media segment `{n}.m4s` of the copy.
#[utoipa::path(
    get,
    path = "/api/files/{id}/copy/{segment}",
    params(("id" = i64, Path), ("segment" = String, Path, description = "`{n}.m4s`")),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_copy_segment(
    State(state): State<AppState>,
    Path((id, segment)): Path<(i64, String)>,
) -> Result<Response, ApiError> {
    let index: usize = segment
        .strip_suffix(".m4s")
        .and_then(|n| n.parse().ok())
        .ok_or(ApiError::not_found())?;
    let dir = ready_dir(&state, id).await?;
    serve(dir.join(format!("{index}.m4s"))).await
}

async fn ready_dir(state: &AppState, id: i64) -> Result<PathBuf, ApiError> {
    let source = state
        .library
        .stream_source(FileId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::not_found())?;
    state.copies.ready_dir(&source).ok_or(ApiError::not_found())
}

async fn serve(path: PathBuf) -> Result<Response, ApiError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::not_found())?;
    let headers = [
        (CONTENT_TYPE, "video/mp4"),
        (CACHE_CONTROL, "private, max-age=86400"),
    ];
    Ok((headers, bytes).into_response())
}
