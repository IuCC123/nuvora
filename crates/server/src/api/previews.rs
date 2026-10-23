//! Seek previews: small frames a player shows above the scrubber.
//!
//! | method | path                           | access |
//! |--------|--------------------------------|--------|
//! | GET    | /api/files/{id}/previews       | member |
//! | GET    | /api/previews/{key}/{sheet}    | member |

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use nuvora_library::{FileId, Library, PreviewKey};
use serde::Serialize;
use utoipa::ToSchema;

use super::ApiError;

/// Where a file's previews are. Sheet `n` holds previews from
/// `n * columns * rows`, left to right, then top to bottom, each
/// `width` x `height` pixels; preview `i` shows the video at `i * interval_ms`.
#[derive(Serialize, ToSchema)]
pub struct PreviewInfo {
    pub interval_ms: u32,
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    /// Previews in all.
    pub count: u32,
    /// JPEG sheet URLs, in order. Immutable: cache forever.
    pub sheets: Vec<String>,
}

/// 404 while the previews are still being made (asking moves the file to the
/// front of the queue; they are usually ready within a minute), or when the
/// file has no video.
#[utoipa::path(
    get,
    path = "/api/files/{id}/previews",
    params(("id" = i64, Path)),
    responses((status = 200, body = PreviewInfo), (status = 404)),
)]
pub(super) async fn get_previews(
    State(lib): State<Library>,
    Path(id): Path<i64>,
) -> Result<Json<PreviewInfo>, ApiError> {
    let previews = lib
        .previews(FileId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::not_found())?;
    let sheets = previews.sheets;
    Ok(Json(PreviewInfo {
        interval_ms: sheets.interval_ms,
        width: sheets.width,
        height: sheets.height,
        columns: sheets.columns,
        rows: sheets.rows,
        count: sheets.count,
        sheets: (0..sheets.sheets)
            .map(|n| format!("/api/previews/{}/{n}", previews.key))
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/previews/{key}/{sheet}",
    params(("key" = String, Path), ("sheet" = u32, Path)),
    responses((status = 200, content_type = "image/jpeg", body = Vec<u8>), (status = 404)),
)]
pub(super) async fn get_sheet(
    State(lib): State<Library>,
    Path((key, sheet)): Path<(String, u32)>,
) -> Result<Response, ApiError> {
    let key: PreviewKey = key.parse().map_err(|_| ApiError::not_found())?;
    let path = lib
        .preview_sheet(&key, sheet)
        .ok_or(ApiError::not_found())?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::not_found())?;
    let headers = [
        (CONTENT_TYPE, "image/jpeg"),
        (CACHE_CONTROL, "private, max-age=31536000, immutable"),
    ];
    Ok((headers, bytes).into_response())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use serde_json::json;

    use super::super::test_app::TestApp;

    #[tokio::test(flavor = "multi_thread")]
    async fn previews_are_made_after_a_scan_and_served_as_jpeg_sheets() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        app.add_movies(&["Arrival (2016)"]).await;
        let movies = app.get("/api/rows/all_movies", &admin).await.json();
        let item = movies["items"][0]["id"].as_i64().unwrap();
        let detail = app.get(&format!("/api/items/{item}"), &admin).await.json();
        let file = detail["versions"][0]["file_id"].as_i64().unwrap();

        let url = format!("/api/files/{file}/previews");
        let mut info = None;
        for _ in 0..100 {
            let reply = app.get(&url, &admin).await;
            if reply.status == StatusCode::OK {
                info = Some(reply.json());
                break;
            }
            assert_eq!(reply.status, StatusCode::NOT_FOUND);
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let info = info.expect("previews made within 20 s");
        let sheet = info["sheets"][0].as_str().unwrap().to_owned();
        assert_eq!(
            info,
            json!({
                "interval_ms": 2000,
                "width": 320,
                "height": 180,
                "columns": 10,
                "rows": 10,
                "count": 1,
                "sheets": [sheet],
            })
        );

        let jpeg = app.get(&sheet, &admin).await;
        assert_eq!(jpeg.status, StatusCode::OK);
        assert_eq!(jpeg.headers["content-type"], "image/jpeg");
        assert_eq!(
            jpeg.headers["cache-control"],
            "private, max-age=31536000, immutable"
        );
        assert_eq!(&jpeg.body[..3], [0xff, 0xd8, 0xff]);

        let key = sheet
            .trim_start_matches("/api/previews/")
            .trim_end_matches("/0");
        for missing in [
            format!("/api/previews/{key}/1"),
            "/api/previews/..%2F..%2Fnuvora.db/0".into(),
            "/api/previews/abc/0".into(),
        ] {
            assert_eq!(
                app.get(&missing, &admin).await.status,
                StatusCode::NOT_FOUND,
                "{missing}"
            );
        }
        assert_eq!(
            app.get(&sheet, "").await.status,
            StatusCode::UNAUTHORIZED,
            "needs a session"
        );
    }
}
