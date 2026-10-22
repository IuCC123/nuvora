//! Watch Together ([`crate::party`]): who's in, invitations, and the live
//! connection that carries playback.
//!
//! | method | path                           | access |
//! |--------|--------------------------------|--------|
//! | GET    | /api/live                      | member | WebSocket: [`LiveEvent`] out, [`LiveCommand`] in
//! | GET    | /api/party                     | member |
//! | POST   | /api/party/invites             | member |
//! | DELETE | /api/party/invites/{user_id}   | member |
//! | POST   | /api/party/{id}/join           | member |
//! | POST   | /api/party/{id}/decline        | member |
//! | POST   | /api/party/leave               | member |
//! | GET    | /api/people                    | member | Who you can invite

use std::time::Duration;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use nuvora_library::{Item, ItemId, User};
use nuvora_media::DeviceProfile;
use serde::Deserialize;
use utoipa::ToSchema;

use super::{ApiError, AppState, ErrorBody, JsonBody};
use crate::auth::SignedIn;
use crate::party::{
    LiveCommand, LiveEvent, Media, PartyAction, PartyError, PartyMediaKind, PartyPerson,
    PartyStatus, PartyView, now_ms,
};

/// A live connection's keep-alive ping, so proxies don't drop an idle socket.
const KEEPALIVE: Duration = Duration::from_secs(25);

#[derive(Deserialize, ToSchema)]
pub struct InviteRequest {
    pub user_id: i64,
}

fn person(user: &User) -> PartyPerson {
    PartyPerson {
        id: user.id.raw(),
        display_name: user.display_name.clone(),
    }
}

fn party_error(e: PartyError) -> ApiError {
    match e {
        PartyError::SelfInvite => ApiError::validation("user_id", e.to_string()),
        PartyError::NoInvite => ApiError::not_found(),
        PartyError::NotInParty => ApiError::bad_request(e.to_string()),
    }
}

/// Everyone else with an account who can sign in, by name.
#[utoipa::path(get, path = "/api/people", responses((status = 200, body = Vec<PartyPerson>)))]
pub(super) async fn people(
    State(state): State<AppState>,
    me: SignedIn,
) -> Result<Json<Vec<PartyPerson>>, ApiError> {
    let mut people: Vec<PartyPerson> = state
        .library
        .users()
        .await
        .map_err(ApiError::internal)?
        .iter()
        .filter(|u| !u.disabled && u.id != me.user.id)
        .map(person)
        .collect();
    people.sort_by_key(|p| p.display_name.to_lowercase());
    Ok(Json(people))
}

#[utoipa::path(get, path = "/api/party", responses((status = 200, body = PartyStatus)))]
pub(super) async fn status(State(state): State<AppState>, me: SignedIn) -> Json<PartyStatus> {
    Json(state.parties.status(me.user.id.raw()))
}

/// Invites someone to watch with you, starting a party if you aren't in one.
#[utoipa::path(
    post,
    path = "/api/party/invites",
    request_body = InviteRequest,
    responses((status = 200, body = PartyView), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)),
)]
pub(super) async fn invite(
    State(state): State<AppState>,
    me: SignedIn,
    JsonBody(body): JsonBody<InviteRequest>,
) -> Result<Json<PartyView>, ApiError> {
    let to = state
        .library
        .users()
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|u| u.id.raw() == body.user_id && !u.disabled)
        .ok_or(ApiError::not_found())?;
    state
        .parties
        .invite(person(&me.user), person(&to))
        .map(Json)
        .map_err(party_error)
}

#[utoipa::path(
    delete,
    path = "/api/party/invites/{user_id}",
    params(("user_id" = i64, Path)),
    responses((status = 200, body = PartyView), (status = 400, body = ErrorBody)),
)]
pub(super) async fn cancel_invite(
    State(state): State<AppState>,
    me: SignedIn,
    Path(user_id): Path<i64>,
) -> Result<Json<PartyView>, ApiError> {
    state
        .parties
        .cancel_invite(me.user.id.raw(), user_id)
        .map(Json)
        .map_err(party_error)
}

/// Accepts an invitation, leaving any party you're in.
#[utoipa::path(
    post,
    path = "/api/party/{id}/join",
    params(("id" = String, Path)),
    responses((status = 200, body = PartyView), (status = 404, body = ErrorBody)),
)]
pub(super) async fn join(
    State(state): State<AppState>,
    me: SignedIn,
    Path(id): Path<String>,
) -> Result<Json<PartyView>, ApiError> {
    state
        .parties
        .join(person(&me.user), &id)
        .map(Json)
        .map_err(party_error)
}

#[utoipa::path(post, path = "/api/party/{id}/decline", params(("id" = String, Path)), responses((status = 204)))]
pub(super) async fn decline(
    State(state): State<AppState>,
    me: SignedIn,
    Path(id): Path<String>,
) -> StatusCode {
    state.parties.decline(me.user.id.raw(), &id);
    StatusCode::NO_CONTENT
}

#[utoipa::path(post, path = "/api/party/leave", responses((status = 204)))]
pub(super) async fn leave(State(state): State<AppState>, me: SignedIn) -> StatusCode {
    state.parties.leave(me.user.id.raw());
    StatusCode::NO_CONTENT
}

/// The live connection: JSON text frames, [`LiveEvent`] from the server
/// (the first is `hello`), [`LiveCommand`] from the client.
#[utoipa::path(get, path = "/api/live", responses((status = 101, description = "Switching to WebSocket")))]
pub(super) async fn live(
    State(state): State<AppState>,
    me: SignedIn,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| connection(socket, state, me.user))
}

async fn connection(mut socket: WebSocket, state: AppState, user: User) {
    let me = user.id.raw();
    let (conn, mut outgoing) = state.parties.connect(me);
    let hello = LiveEvent::Hello {
        server_time_ms: now_ms(),
        status: state.parties.status(me),
    };
    if send(&mut socket, &hello).await {
        let mut keepalive = tokio::time::interval(KEEPALIVE);
        keepalive.tick().await;
        loop {
            tokio::select! {
                frame = outgoing.recv() => match frame {
                    Some(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                incoming = socket.recv() => match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(command) = serde_json::from_str::<LiveCommand>(&text)
                            && !handle(&mut socket, &state, &user, command).await
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                },
                _ = keepalive.tick() => {
                    if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    state.parties.disconnect(me, conn);
}

/// Carries out one command; false when the socket is gone.
async fn handle(
    socket: &mut WebSocket,
    state: &AppState,
    user: &User,
    command: LiveCommand,
) -> bool {
    let me = person(user);
    let result = match command {
        LiveCommand::Ping { client_time_ms } => {
            let pong = LiveEvent::Pong {
                client_time_ms,
                server_time_ms: now_ms(),
            };
            return send(socket, &pong).await;
        }
        LiveCommand::Start {
            item_id,
            file_id,
            position_ms,
        } => match media(state, user, item_id, file_id).await {
            Some(media) => state.parties.start(me, media, position_ms),
            None => Ok(()),
        },
        LiveCommand::Play { position_ms } => {
            state.parties.control(me, PartyAction::Play, position_ms)
        }
        LiveCommand::Pause { position_ms } => {
            state.parties.control(me, PartyAction::Pause, position_ms)
        }
        LiveCommand::Seek { position_ms } => {
            state.parties.control(me, PartyAction::Seek, position_ms)
        }
    };
    // Not in a party: nothing to sync, and nothing to tell.
    let _ = result;
    true
}

/// The movie or episode `item_id` with its file `file_id`, titled for the party.
async fn media(state: &AppState, user: &User, item_id: i64, file_id: i64) -> Option<Media> {
    let item = state
        .library
        .item(
            ItemId::from_raw(item_id),
            DeviceProfile::baseline(),
            user.id,
        )
        .await
        .ok()??;
    let (kind, title, subtitle, versions) = match item {
        Item::Movie(movie) => (PartyMediaKind::Movie, movie.title, None, movie.versions),
        Item::Episode(episode) => {
            let number = episode.number;
            let label = if number.season == 0 {
                format!("Special {}", number.episode)
            } else {
                format!("S{} · E{}", number.season, number.episode)
            };
            (
                PartyMediaKind::Episode,
                episode.show_title,
                Some(label),
                episode.versions,
            )
        }
        _ => return None,
    };
    versions
        .iter()
        .any(|v| v.file_id.raw() == file_id)
        .then_some(Media {
            item_id,
            file_id,
            kind,
            title,
            subtitle,
        })
}

async fn send(socket: &mut WebSocket, event: &LiveEvent) -> bool {
    match serde_json::to_string(event) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::super::test_app::TestApp;

    #[tokio::test(flavor = "multi_thread")]
    async fn inviting_joining_and_leaving_over_http() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let created = app
            .call(
                "POST",
                "/api/admin/users",
                Some(&admin),
                Some(json!({"username": "bo", "display_name": "Bo", "password": "password bo", "role": "member"})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED);
        let bo_id = created.json()["id"].as_i64().unwrap();
        let bo = app.login("bo", "password bo").await.cookie();

        let people = app.get("/api/people", &admin).await.json();
        assert_eq!(people, json!([{"id": bo_id, "display_name": "Bo"}]));

        let party = app
            .call(
                "POST",
                "/api/party/invites",
                Some(&admin),
                Some(json!({"user_id": bo_id})),
            )
            .await;
        assert_eq!(party.status, StatusCode::OK);
        let party = party.json();
        assert_eq!(
            party["invited"],
            json!([{"id": bo_id, "display_name": "Bo"}])
        );
        let id = party["id"].as_str().unwrap().to_owned();

        let status = app.get("/api/party", &bo).await.json();
        assert_eq!(status["party"], json!(null));
        assert_eq!(status["invites"][0]["from"]["display_name"], "Admin");
        assert_eq!(status["invites"][0]["party_id"], id.as_str());

        let joined = app
            .call("POST", &format!("/api/party/{id}/join"), Some(&bo), None)
            .await;
        assert_eq!(joined.status, StatusCode::OK);
        assert_eq!(joined.json()["members"].as_array().unwrap().len(), 2);
        let again = app
            .call("POST", "/api/party/nope/join", Some(&bo), None)
            .await;
        assert_eq!(again.status, StatusCode::NOT_FOUND);

        let left = app.call("POST", "/api/party/leave", Some(&bo), None).await;
        assert_eq!(left.status, StatusCode::NO_CONTENT);
        let status = app.get("/api/party", &admin).await.json();
        assert_eq!(status["party"]["members"].as_array().unwrap().len(), 1);

        // Inviting yourself is refused; a socket from another site is too.
        let me = party["host_id"].as_i64().unwrap();
        let own = app
            .call(
                "POST",
                "/api/party/invites",
                Some(&admin),
                Some(json!({"user_id": me})),
            )
            .await;
        assert_eq!(own.status, StatusCode::BAD_REQUEST);
        let hijack = app
            .send(
                axum::http::Request::get("/api/live")
                    .header("cookie", &admin)
                    .header("origin", "https://evil.example")
                    .header("host", "localhost")
                    .header("upgrade", "websocket")
                    .header("connection", "upgrade")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(hijack.status, StatusCode::FORBIDDEN);
    }
}
