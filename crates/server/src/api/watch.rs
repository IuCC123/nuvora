//! Watch progress: the client reports where playback is, the catalog decides
//! what that means (`nuvora_library::WatchEvent`). Every route is scoped to the
//! signed-in user.
//!
//! | method | path                     | purpose                  |
//! |--------|--------------------------|--------------------------|
//! | PUT    | /api/items/{id}/progress | Report where playback is |
//! | PUT    | /api/items/{id}/watched  | Mark watched             |
//! | DELETE | /api/items/{id}/watched  | Mark unwatched           |

use axum::extract::{Path, State};
use axum::http::StatusCode;
use nuvora_library::{FileId, ItemId, WatchError, WatchEvent};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, AppState, ErrorBody, JsonBody};
use crate::auth::SignedIn;

/// The signed-in user's state for one movie or episode.
#[derive(Serialize, ToSchema)]
pub struct WatchState {
    /// Where to resume. `None` unless the movie is in progress.
    pub position_ms: Option<u64>,
    /// The version `position_ms` belongs to.
    pub file_id: Option<i64>,
    /// Finished at least once.
    pub watched: bool,
    pub play_count: u32,
    /// Unix ms of the last finish.
    pub last_watched_at: Option<i64>,
}

/// Where playback is. Clients send it every 10 s while playing, on pause,
/// after a seek settles and when leaving the player.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProgressRequest {
    /// The version playing. Must be a version of this item.
    pub file_id: i64,
    pub position_ms: u64,
    /// At least 1. `position_ms` may exceed it by at most 5 s.
    pub duration_ms: u64,
}

impl From<nuvora_library::WatchState> for WatchState {
    fn from(state: nuvora_library::WatchState) -> Self {
        WatchState {
            position_ms: state.resume.map(|r| r.position_ms),
            file_id: state.resume.map(|r| r.file_id.raw()),
            watched: state.watched(),
            play_count: state.play_count,
            last_watched_at: state.last_watched_at,
        }
    }
}

/// Reports where playback is; the server decides what it means. Under 60 s
/// stores nothing and clears earlier progress. From 60 s to under 90% of the
/// duration stores the position. From 90% the movie counts as watched once and
/// the position is cleared; another report at 90% or more within 3 hours of
/// that does not count again.
#[utoipa::path(
    put,
    path = "/api/items/{id}/progress",
    params(("id" = i64, Path)),
    request_body = ProgressRequest,
    responses((status = 204), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)),
)]
pub(super) async fn put_progress(
    State(state): State<AppState>,
    me: SignedIn,
    Path(id): Path<i64>,
    JsonBody(body): JsonBody<ProgressRequest>,
) -> Result<StatusCode, ApiError> {
    let event = progress_event(&body)?;
    record(&state, &me, id, event).await
}

/// Marks the movie or episode watched, like finishing it. On a show or a
/// season, marks every episode in it watched; episodes already watched keep
/// their play count.
#[utoipa::path(
    put,
    path = "/api/items/{id}/watched",
    params(("id" = i64, Path)),
    responses((status = 204), (status = 404, body = ErrorBody)),
)]
pub(super) async fn put_watched(
    State(state): State<AppState>,
    me: SignedIn,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    record(&state, &me, id, WatchEvent::Watched).await
}

/// Marks the movie or episode unwatched: clears the position, the play
/// count and the last finish. On a show or a season, every episode in it.
#[utoipa::path(
    delete,
    path = "/api/items/{id}/watched",
    params(("id" = i64, Path)),
    responses((status = 204), (status = 404, body = ErrorBody)),
)]
pub(super) async fn delete_watched(
    State(state): State<AppState>,
    me: SignedIn,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    record(&state, &me, id, WatchEvent::Unwatched).await
}

/// How far past the reported duration a position may be: players round the
/// duration, and the last report lands a little after the end.
const PAST_END_SLACK_MS: u64 = 5_000;
/// Longer than any movie, and small enough for SQLite's signed integers.
const MAX_DURATION_MS: u64 = 7 * 24 * 3600 * 1000;

fn progress_event(body: &ProgressRequest) -> Result<WatchEvent, ApiError> {
    if !(1..=MAX_DURATION_MS).contains(&body.duration_ms) {
        return Err(ApiError::validation(
            "duration_ms",
            "Duration must be between 1 ms and a week",
        ));
    }
    if body.position_ms > body.duration_ms + PAST_END_SLACK_MS {
        return Err(ApiError::validation(
            "position_ms",
            "Position is past the end of the movie",
        ));
    }
    Ok(WatchEvent::Progress {
        file_id: FileId::from_raw(body.file_id),
        position_ms: body.position_ms.min(body.duration_ms),
        duration_ms: body.duration_ms,
    })
}

async fn record(
    state: &AppState,
    me: &SignedIn,
    item: i64,
    event: WatchEvent,
) -> Result<StatusCode, ApiError> {
    state
        .library
        .record_watch(
            me.user.id,
            ItemId::from_raw(item),
            event,
            state.auth.now_ms(),
        )
        .await
        .map_err(|e| match e {
            WatchError::ItemNotFound => ApiError::not_found(),
            WatchError::FileNotOnItem => {
                ApiError::validation("file_id", "That file isn't a version of this movie")
            }
            WatchError::Query(e) => ApiError::internal(e),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::header::{COOKIE, ETAG, IF_NONE_MATCH};
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};

    use super::super::test_app::{Reply, TestApp};

    const MOVIE_MS: u64 = 100 * 60_000;

    /// An admin and a member on a server with two movies, Big Buck and
    /// Sintel, with item and file ids 1 and 2 in that order.
    async fn two_users() -> (TestApp, String, String) {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        app.add_movies(&["Big Buck (2008)", "Sintel (2010)"]).await;
        let home = app.get("/api/home", &admin).await.json();
        for (id, title) in [(1, "Big Buck"), (2, "Sintel")] {
            let item = app.get(&format!("/api/items/{id}"), &admin).await.json();
            assert_eq!(
                (
                    item["title"].as_str(),
                    item["versions"][0]["file_id"].as_i64()
                ),
                (Some(title), Some(id)),
                "{home}"
            );
        }
        let created = app
            .call(
                "POST",
                "/api/admin/users",
                Some(&admin),
                Some(json!({"username": "bo", "display_name": "Bo", "password": "password bo", "role": "member"})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json());
        let member = app.login("bo", "password bo").await.cookie();
        (app, admin, member)
    }

    async fn report(app: &TestApp, cookie: &str, item: u32, file: u32, position_ms: u64) -> Reply {
        app.call(
            "PUT",
            &format!("/api/items/{item}/progress"),
            Some(cookie),
            Some(json!({"file_id": file, "position_ms": position_ms, "duration_ms": MOVIE_MS})),
        )
        .await
    }

    fn row_kinds(home: &Value) -> Vec<&str> {
        home["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap())
            .collect()
    }

    /// `(id, progress, watched)` of every card in the All Movies row.
    fn cards(home: &Value) -> Vec<(i64, Value, bool)> {
        let all = home["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["kind"] == "all_movies")
            .unwrap();
        all["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["id"].as_i64().unwrap(),
                    c["progress"].clone(),
                    c["watched"].as_bool().unwrap(),
                )
            })
            .collect()
    }

    async fn watch(app: &TestApp, cookie: &str, item: u32) -> Value {
        app.get(&format!("/api/items/{item}"), cookie).await.json()["watch"].clone()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn progress_belongs_to_the_user_who_reported_it() {
        let (app, admin, member) = two_users().await;
        let early = report(&app, &admin, 1, 1, 30_000).await;
        assert_eq!(early.status, StatusCode::NO_CONTENT);
        let home = app.get("/api/home", &admin).await.json();
        assert_eq!(
            row_kinds(&home),
            ["recently_added", "all_movies"],
            "30 s is ignored"
        );
        assert_eq!(watch(&app, &admin, 1).await["position_ms"], Value::Null);

        assert_eq!(
            report(&app, &admin, 1, 1, 30 * 60_000).await.status,
            StatusCode::NO_CONTENT
        );
        let home = app.get("/api/home", &admin).await.json();
        assert_eq!(
            row_kinds(&home),
            ["continue_watching", "recently_added", "all_movies"]
        );
        assert_eq!(home["rows"][0]["title"], "Continue Watching");
        assert_eq!(home["rows"][0]["items"][0]["id"], 1);
        assert_eq!(
            cards(&home),
            [(1, json!(0.3), false), (2, Value::Null, false)]
        );
        assert_eq!(
            watch(&app, &admin, 1).await,
            json!({"position_ms": 1_800_000, "file_id": 1, "watched": false, "play_count": 0, "last_watched_at": null})
        );

        let theirs = app.get("/api/home", &member).await.json();
        assert_eq!(row_kinds(&theirs), ["recently_added", "all_movies"]);
        assert_eq!(
            cards(&theirs),
            [(1, Value::Null, false), (2, Value::Null, false)]
        );
        assert_eq!(watch(&app, &member, 1).await["position_ms"], Value::Null);
        let row = app.get("/api/rows/continue_watching", &member).await;
        assert_eq!(
            (row.status, row.json()["items"].clone()),
            (StatusCode::OK, json!([]))
        );
        let row = app.get("/api/rows/continue_watching", &admin).await.json();
        assert_eq!(row["items"][0]["id"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ninety_percent_marks_watched_once_and_unwatched_clears_it() {
        let (app, admin, _) = two_users().await;
        report(&app, &admin, 2, 2, 50 * 60_000).await;
        assert_eq!(
            report(&app, &admin, 2, 2, 95 * 60_000).await.status,
            StatusCode::NO_CONTENT
        );
        let state = watch(&app, &admin, 2).await;
        assert_eq!(
            (
                state["position_ms"].clone(),
                state["watched"].clone(),
                state["play_count"].clone()
            ),
            (Value::Null, json!(true), json!(1))
        );
        assert!(state["last_watched_at"].as_i64().unwrap() > 0);
        let home = app.get("/api/home", &admin).await.json();
        assert_eq!(
            row_kinds(&home),
            ["recently_added", "all_movies"],
            "watched leaves Continue Watching"
        );
        assert_eq!(cards(&home)[1], (2, Value::Null, true));

        report(&app, &admin, 2, 2, 99 * 60_000).await;
        app.call("PUT", "/api/items/2/watched", Some(&admin), None)
            .await;
        assert_eq!(
            watch(&app, &admin, 2).await["play_count"],
            1,
            "a repeat within 3 h"
        );
        app.state
            .auth
            .advance(std::time::Duration::from_secs(3 * 3600));
        let again = app
            .call("PUT", "/api/items/2/watched", Some(&admin), None)
            .await;
        assert_eq!(again.status, StatusCode::NO_CONTENT);
        assert_eq!(watch(&app, &admin, 2).await["play_count"], 2);

        let cleared = app
            .call("DELETE", "/api/items/2/watched", Some(&admin), None)
            .await;
        assert_eq!(cleared.status, StatusCode::NO_CONTENT);
        assert_eq!(
            watch(&app, &admin, 2).await,
            json!({"position_ms": null, "file_id": null, "watched": false, "play_count": 0, "last_watched_at": null})
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reports_are_checked() {
        let (app, admin, _) = two_users().await;
        let wrong_file = report(&app, &admin, 1, 2, 10 * 60_000).await;
        assert_eq!(wrong_file.status, StatusCode::BAD_REQUEST);
        assert_eq!(wrong_file.json()["field"], "file_id");

        let past = |position_ms: u64| {
            app.call(
                "PUT",
                "/api/items/1/progress",
                Some(&admin),
                Some(json!({"file_id": 1, "position_ms": position_ms, "duration_ms": MOVIE_MS})),
            )
        };
        assert_eq!(past(MOVIE_MS + 5_000).await.status, StatusCode::NO_CONTENT);
        let too_far = past(MOVIE_MS + 5_001).await;
        assert_eq!(too_far.status, StatusCode::BAD_REQUEST);
        assert_eq!(too_far.json()["field"], "position_ms");
        let state = watch(&app, &admin, 1).await;
        assert_eq!(
            (state["play_count"].clone(), state["position_ms"].clone()),
            (json!(1), Value::Null)
        );

        for duration in [0, u64::MAX] {
            let reply = app
                .call(
                    "PUT",
                    "/api/items/1/progress",
                    Some(&admin),
                    Some(json!({"file_id": 1, "position_ms": 0, "duration_ms": duration})),
                )
                .await;
            assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{duration}");
            assert_eq!(reply.json()["field"], "duration_ms");
        }
        let negative = app
            .call(
                "PUT",
                "/api/items/1/progress",
                Some(&admin),
                Some(json!({"file_id": 1, "position_ms": -1, "duration_ms": MOVIE_MS})),
            )
            .await;
        assert_eq!(negative.status, StatusCode::BAD_REQUEST);

        assert_eq!(
            report(&app, &admin, 99, 1, 10 * 60_000).await.status,
            StatusCode::NOT_FOUND
        );
        let missing = app
            .call("PUT", "/api/items/99/watched", Some(&admin), None)
            .await;
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
        let anonymous = app.call("PUT", "/api/items/1/watched", None, None).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_304_never_serves_another_users_state() {
        let (app, admin, member) = two_users().await;
        let revalidate = |uri: &'static str, cookie: String, etag: String| {
            let app = &app;
            async move {
                let request = Request::builder()
                    .uri(uri)
                    .header(COOKIE, cookie)
                    .header(IF_NONE_MATCH, etag)
                    .body(Body::empty())
                    .unwrap();
                app.send(request).await
            }
        };
        let etag = |reply: &Reply| {
            reply
                .headers
                .get(ETAG)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        };

        let admin_home = app.get("/api/home", &admin).await;
        let member_home = app.get("/api/home", &member).await;
        let rev = etag(&admin_home);
        assert_eq!(
            etag(&member_home),
            rev,
            "neither has state yet: same body, same tag"
        );
        assert!(rev.ends_with(".0\""), "{rev}");

        report(&app, &admin, 1, 1, 10 * 60_000).await;
        let reply = revalidate("/api/home", member.clone(), rev.clone()).await;
        assert_eq!(
            reply.status,
            StatusCode::NOT_MODIFIED,
            "the member's home did not change"
        );
        let reply = revalidate("/api/home", admin.clone(), rev.clone()).await;
        assert_eq!(reply.status, StatusCode::OK, "the admin's did");
        let admin_rev = etag(&reply);
        assert_ne!(admin_rev, rev);

        let reply = revalidate("/api/home", member.clone(), admin_rev.clone()).await;
        assert_eq!(
            reply.status,
            StatusCode::OK,
            "the admin's tag never matches the member"
        );
        assert_eq!(row_kinds(&reply.json()), ["recently_added", "all_movies"]);

        report(&app, &member, 2, 2, 10 * 60_000).await;
        let member_rev = etag(&app.get("/api/home", &member).await);
        assert_ne!(member_rev, admin_rev, "revisions come from one sequence");
        let reply = revalidate("/api/home", admin.clone(), member_rev.clone()).await;
        assert_eq!(reply.status, StatusCode::OK);

        let item = app.get("/api/items/1", &admin).await;
        let item_rev = etag(&item);
        assert!(
            item_rev.starts_with(&admin_rev[..admin_rev.len() - 1]),
            "{item_rev}"
        );
        let reply = revalidate("/api/items/1", member.clone(), item_rev.clone()).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json()["watch"]["position_ms"], Value::Null);
        let reply = revalidate("/api/items/1", admin.clone(), item_rev).await;
        assert_eq!(reply.status, StatusCode::NOT_MODIFIED);
        let row = app.get("/api/rows/continue_watching", &admin).await;
        let reply = revalidate("/api/rows/continue_watching", member.clone(), etag(&row)).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json()["items"][0]["id"], 2);
    }
}
