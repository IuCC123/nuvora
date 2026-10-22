//! Health, first-run setup, sign in and out, and the signed-in user's account.
//!
//! | method | path              | access  |
//! |--------|-------------------|---------|
//! | GET    | /api/health       | public  |
//! | GET    | /api/auth/state   | public  |
//! | POST   | /api/setup        | public  |
//! | POST   | /api/auth/login   | public  |
//! | POST   | /api/auth/logout  | session |
//! | PATCH  | /api/auth/me      | session |

use axum::Json;
use axum::extract::State;
use axum::http::header::{SET_COOKIE, USER_AGENT};
use axum::http::{HeaderMap, StatusCode, request::Parts};
use axum::response::{IntoResponse, Response};
use nuvora_library::{AccountError, NewUser, UserChanges};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, AppState, ErrorBody, ErrorCode, JsonBody};
use crate::auth::{self, SESSION_TTL, SignedIn};

/// Shown until the admin names the server.
const DEFAULT_SERVER_NAME: &str = "Nuvora";

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: HealthStatus,
    /// The server's version, e.g. `"0.1.0"`.
    pub version: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
}

#[derive(Serialize, ToSchema)]
pub struct AuthState {
    /// True until the first admin exists. The web app then shows the welcome flow.
    pub setup_required: bool,
    /// The signed-in user, or `None` without a valid session.
    pub user: Option<User>,
    pub server_name: String,
}

#[derive(Serialize, ToSchema, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: Role,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

/// Creates the first admin. Usernames are 1 to 32 letters, digits, `.`, `_` or
/// `-`, unique ignoring case. Passwords are at least 8 characters.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetupRequest {
    pub username: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Every field is optional. Changing the password needs both passwords, and
/// signs out the user's other sessions.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateMeRequest {
    pub display_name: Option<String>,
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}

#[utoipa::path(get, path = "/api/health", responses((status = 200, body = Health)))]
pub(super) async fn health() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Ok,
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

/// Never 401: without a valid session `user` is `None`.
#[utoipa::path(get, path = "/api/auth/state", responses((status = 200, body = AuthState)))]
pub(super) async fn auth_state(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthState>, ApiError> {
    let lib = &state.library;
    let session = auth::current_session(&state, &headers).await?;
    Ok(Json(AuthState {
        setup_required: lib.setup_required().await.map_err(ApiError::internal)?,
        user: session.map(|s| User::from(s.user)),
        server_name: server_name(lib).await?,
    }))
}

/// Creates the first admin and signs them in (sets the session cookie). 409
/// `setup_done` once any user exists.
#[utoipa::path(
    post,
    path = "/api/setup",
    request_body = SetupRequest,
    responses(
        (status = 200, body = User),
        (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ),
)]
pub(super) async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonBody(body): JsonBody<SetupRequest>,
) -> Result<Response, ApiError> {
    let lib = &state.library;
    if !lib.setup_required().await.map_err(ApiError::internal)? {
        return Err(account_error(AccountError::SetupDone));
    }
    let new = NewUser {
        username: valid_username(&body.username)?,
        display_name: valid_display_name(&body.display_name)?,
        password_hash: auth::hash_password(valid_password("password", body.password)?).await?,
        role: nuvora_library::Role::Admin,
    };
    let user = lib
        .create_first_admin(new, state.auth.now_ms())
        .await
        .map_err(account_error)?;
    sign_in(&state, user, &headers).await
}

/// Sets the session cookie. Wrong username, wrong password and disabled
/// account all answer 401 `bad_credentials`. Each failure for a username makes
/// the next attempt wait (1 s, doubling, at most 10 s) and 20 failures from one
/// address within 15 minutes lock it for 15 minutes; until then the answer is
/// 429 `rate_limited` with `Retry-After`.
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, body = User),
        (status = 401, body = ErrorBody),
        (status = 429, body = ErrorBody),
    ),
)]
pub(super) async fn login(
    State(state): State<AppState>,
    parts: Parts,
    JsonBody(body): JsonBody<LoginRequest>,
) -> Result<Response, ApiError> {
    let username = body.username.trim();
    let keys = state.auth.sign_in_keys(username, &parts);
    if let Some(retry_after) = state.auth.blocked_for(&keys) {
        return Err(ApiError::rate_limited(retry_after));
    }
    let found = state
        .library
        .credentials(username.to_owned())
        .await
        .map_err(ApiError::internal)?;
    let (user, hash) = match found {
        Some(c) => (Some(c.user), Some(c.password_hash)),
        None => (None, None),
    };
    let password_ok = auth::verify_password(body.password, hash).await;
    match user {
        Some(user) if password_ok && !user.disabled => {
            state.auth.record_success(&keys);
            sign_in(&state, user, &parts.headers).await
        }
        _ => {
            state.auth.record_failure(&keys);
            Err(ApiError::bad_credentials())
        }
    }
}

/// Ends this session and clears the cookie.
#[utoipa::path(post, path = "/api/auth/logout", responses((status = 204)))]
pub(super) async fn logout(
    State(state): State<AppState>,
    signed_in: SignedIn,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    state
        .library
        .delete_session(signed_in.token)
        .await
        .map_err(ApiError::internal)?;
    let cookie = auth::session_cookie(None, &headers);
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, cookie)]).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/auth/me",
    request_body = UpdateMeRequest,
    responses((status = 200, body = User), (status = 400, body = ErrorBody)),
)]
pub(super) async fn update_me(
    State(state): State<AppState>,
    signed_in: SignedIn,
    JsonBody(body): JsonBody<UpdateMeRequest>,
) -> Result<Json<User>, ApiError> {
    let lib = &state.library;
    let me = signed_in.user.id;
    let mut changes = UserChanges {
        display_name: body
            .display_name
            .as_deref()
            .map(valid_display_name)
            .transpose()?,
        ..UserChanges::default()
    };
    if let Some(new_password) = body.new_password {
        let new_password = valid_password("new_password", new_password)?;
        let current = body.current_password.unwrap_or_default();
        let hash = lib.password_hash(me).await.map_err(ApiError::internal)?;
        if !auth::verify_password(current, hash).await {
            return Err(ApiError::validation(
                "current_password",
                "That isn't your current password",
            ));
        }
        changes.password_hash = Some(auth::hash_password(new_password).await?);
    }
    let user = lib
        .update_user(me, me, changes, Some(signed_in.token), state.auth.now_ms())
        .await
        .map_err(account_error)?;
    Ok(Json(User::from(user)))
}

/// Starts a session for `user`: the `User` body plus the session cookie.
async fn sign_in(
    state: &AppState,
    user: nuvora_library::User,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let (token, hash) = auth::new_token();
    let now = state.auth.now_ms();
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| ua.chars().take(256).collect());
    state
        .library
        .create_session(
            user.id,
            hash,
            now,
            now + SESSION_TTL.as_millis() as i64,
            user_agent,
        )
        .await
        .map_err(ApiError::internal)?;
    let cookie = auth::session_cookie(Some(&token), headers);
    Ok(([(SET_COOKIE, cookie)], Json(User::from(user))).into_response())
}

pub(super) async fn server_name(lib: &nuvora_library::Library) -> Result<String, ApiError> {
    Ok(lib
        .server_name()
        .await
        .map_err(ApiError::internal)?
        .unwrap_or_else(|| DEFAULT_SERVER_NAME.to_owned()))
}

pub(super) fn account_error(e: AccountError) -> ApiError {
    match e {
        AccountError::SetupDone => {
            ApiError::conflict(ErrorCode::SetupDone, "This server is already set up")
        }
        AccountError::UsernameTaken => {
            let mut error = ApiError::conflict(ErrorCode::UsernameTaken, "That username is taken");
            error.body.field = Some("username".to_owned());
            error
        }
        AccountError::LastAdmin => ApiError::conflict(
            ErrorCode::LastAdmin,
            "There must always be at least one enabled admin",
        ),
        AccountError::OwnAccount => ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Validation,
            "You can't disable or delete your own account",
        ),
        AccountError::NotFound => ApiError::not_found(),
        AccountError::Query(e) => ApiError::internal(e),
    }
}

/// 1 to 32 letters, digits, `.`, `_` or `-`, after trimming.
pub(super) fn valid_username(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    let ok = (1..=32).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err(ApiError::validation(
            "username",
            "Use 1 to 32 letters, numbers, dots, dashes or underscores",
        ));
    }
    Ok(name.to_owned())
}

pub(super) fn valid_display_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if !(1..=64).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(ApiError::validation(
            "display_name",
            "Use 1 to 64 characters",
        ));
    }
    Ok(name.to_owned())
}

/// At least 8 characters. The upper bound keeps hashing cheap to refuse.
pub(super) fn valid_password(field: &str, password: String) -> Result<String, ApiError> {
    if !(8..=256).contains(&password.chars().count()) {
        return Err(ApiError::validation(field, "Use at least 8 characters"));
    }
    Ok(password)
}

impl From<nuvora_library::User> for User {
    fn from(u: nuvora_library::User) -> Self {
        User {
            id: u.id.raw(),
            username: u.username,
            display_name: u.display_name,
            role: u.role.into(),
        }
    }
}

impl From<nuvora_library::Role> for Role {
    fn from(r: nuvora_library::Role) -> Self {
        match r {
            nuvora_library::Role::Admin => Role::Admin,
            nuvora_library::Role::Member => Role::Member,
        }
    }
}

impl From<Role> for nuvora_library::Role {
    fn from(r: Role) -> Self {
        match r {
            Role::Admin => nuvora_library::Role::Admin,
            Role::Member => nuvora_library::Role::Member,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::header::{
        AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN, RETRY_AFTER, SET_COOKIE,
    };
    use axum::http::{Request, StatusCode};
    use serde_json::json;

    use super::super::test_app::{ADMIN_PASSWORD, TestApp};

    #[tokio::test(flavor = "multi_thread")]
    async fn setup_works_once() {
        let app = TestApp::new().await;
        let state = app.call("GET", "/api/auth/state", None, None).await.json();
        assert_eq!(
            state,
            json!({"setup_required": true, "user": null, "server_name": "Nuvora"})
        );

        let short = app
            .call(
                "POST",
                "/api/setup",
                None,
                Some(json!({"username": "admin", "display_name": "Admin", "password": "short"})),
            )
            .await;
        assert_eq!(short.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            short.json(),
            json!({"code": "validation", "message": "Use at least 8 characters", "field": "password"})
        );

        let cookie = app.setup_admin().await;
        let state = app
            .call("GET", "/api/auth/state", Some(&cookie), None)
            .await
            .json();
        assert_eq!(
            state,
            json!({
                "setup_required": false,
                "user": {"id": 1, "username": "admin", "display_name": "Admin", "role": "admin"},
                "server_name": "Nuvora",
            })
        );

        let again = app
            .call(
                "POST",
                "/api/setup",
                None,
                Some(
                    json!({"username": "mallory", "display_name": "M", "password": "password123"}),
                ),
            )
            .await;
        assert_eq!(again.status, StatusCode::CONFLICT);
        assert_eq!(again.json()["code"], "setup_done");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_setups_make_exactly_one_admin() {
        let app = std::sync::Arc::new(TestApp::new().await);
        let attempts = (0..8).map(|i| {
            let app = app.clone();
            tokio::spawn(async move {
                app.call(
                    "POST",
                    "/api/setup",
                    None,
                    Some(json!({"username": format!("admin{i}"), "display_name": "A", "password": "password123"})),
                )
                .await
                .status
            })
        });
        let mut statuses = Vec::new();
        for attempt in attempts {
            statuses.push(attempt.await.unwrap());
        }
        statuses.sort();
        assert_eq!(
            statuses,
            [vec![StatusCode::OK], vec![StatusCode::CONFLICT; 7]].concat()
        );
        let users = app.state.library.users().await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].role, nuvora_library::Role::Admin);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn login_succeeds_and_fails_the_same_way_for_any_wrong_input() {
        let app = TestApp::new().await;
        app.setup_admin().await;
        for (username, password) in [("admin", "wrong password"), ("nobody", ADMIN_PASSWORD)] {
            let reply = app.login(username, password).await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
            assert_eq!(
                reply.json(),
                json!({"code": "bad_credentials", "message": "Wrong username or password", "field": null})
            );
            assert!(reply.headers.get(SET_COOKIE).is_none());
        }

        app.state.auth.advance(Duration::from_secs(1));
        let reply = app.login(" ADMIN ", ADMIN_PASSWORD).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json()["username"], "admin");
        let set_cookie = reply.headers.get(SET_COOKIE).unwrap().to_str().unwrap();
        assert!(set_cookie.starts_with("nuvora_session="), "{set_cookie}");
        assert!(
            set_cookie.ends_with("; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000"),
            "{set_cookie}"
        );
        assert_eq!(
            app.get("/api/home", &reply.cookie()).await.status,
            StatusCode::OK
        );

        let https = app
            .send(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(CONTENT_TYPE, "application/json")
                    .header("x-forwarded-proto", "https")
                    .body(Body::from(
                        json!({"username": "admin", "password": ADMIN_PASSWORD}).to_string(),
                    ))
                    .unwrap(),
            )
            .await;
        assert!(
            https
                .headers
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("; Secure")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_wrong_password_makes_the_username_wait() {
        let app = TestApp::new().await;
        app.setup_admin().await;
        assert_eq!(
            app.login("admin", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
        let waiting = app.login("Admin", ADMIN_PASSWORD).await;
        assert_eq!(waiting.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            waiting.json(),
            json!({"code": "rate_limited", "message": "Too many failed sign-ins. Try again in a second.", "field": null})
        );
        assert_eq!(waiting.headers.get(RETRY_AFTER).unwrap(), "1");

        app.state.auth.advance(Duration::from_secs(1));
        assert_eq!(
            app.login("admin", "wrong again").await.status,
            StatusCode::UNAUTHORIZED
        );
        let longer = app.login("admin", ADMIN_PASSWORD).await;
        assert_eq!(longer.headers.get(RETRY_AFTER).unwrap(), "2");
        app.state.auth.advance(Duration::from_secs(2));
        assert_eq!(
            app.login("admin", ADMIN_PASSWORD).await.status,
            StatusCode::OK
        );
        assert_eq!(
            app.login("admin", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.login("admin", ADMIN_PASSWORD)
                .await
                .headers
                .get(RETRY_AFTER)
                .unwrap(),
            "1",
            "a success started the wait over"
        );
    }

    /// Twenty failed sign-ins, each for a different username so only the
    /// address limit can trigger.
    async fn guess_twenty(app: &TestApp, peer: &str, forwarded_for: Option<&str>) {
        for i in 0..20 {
            let reply = app
                .login_from(
                    peer,
                    forwarded_for,
                    &format!("{peer} guess {i}"),
                    "wrong password",
                )
                .await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "attempt {i}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn twenty_failures_lock_a_public_address() {
        let app = TestApp::new().await;
        app.setup_admin().await;
        guess_twenty(&app, "203.0.113.9", None).await;
        let locked = app
            .login_from("203.0.113.9", None, "admin", ADMIN_PASSWORD)
            .await;
        assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(locked.headers.get(RETRY_AFTER).unwrap(), "900");
        assert_eq!(
            locked.json()["message"],
            "Too many failed sign-ins. Try again in 15 minutes."
        );
        let elsewhere = app
            .login_from("198.51.100.7", None, "admin", ADMIN_PASSWORD)
            .await;
        assert_eq!(elsewhere.status, StatusCode::OK);
        app.state.auth.advance(Duration::from_secs(15 * 60));
        let later = app
            .login_from("203.0.113.9", None, "admin", ADMIN_PASSWORD)
            .await;
        assert_eq!(later.status, StatusCode::OK);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn local_peers_have_no_address_limit_without_a_trusted_proxy() {
        let app = TestApp::new().await;
        app.setup_admin().await;
        for peer in ["127.0.0.1", "192.168.1.20", "10.0.0.3", "::1", "fd00::5"] {
            guess_twenty(&app, peer, Some("203.0.113.9")).await;
            let reply = app.login_from(peer, None, "admin", ADMIN_PASSWORD).await;
            assert_eq!(reply.status, StatusCode::OK, "{peer}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn behind_a_trusted_proxy_the_forwarded_client_is_limited() {
        let app = TestApp::behind(&["127.0.0.1", "10.0.0.0/8"]).await;
        app.setup_admin().await;
        guess_twenty(&app, "127.0.0.1", Some("6.6.6.6, 203.0.113.9, 10.1.1.1")).await;
        for spoofed in ["203.0.113.9", "1.1.1.1, 203.0.113.9"] {
            let reply = app
                .login_from("127.0.0.1", Some(spoofed), "admin", ADMIN_PASSWORD)
                .await;
            assert_eq!(reply.status, StatusCode::TOO_MANY_REQUESTS, "{spoofed}");
        }
        let neighbour = app
            .login_from("127.0.0.1", Some("198.51.100.7"), "admin", ADMIN_PASSWORD)
            .await;
        assert_eq!(neighbour.status, StatusCode::OK);

        guess_twenty(&app, "192.168.1.20", None).await;
        let lan = app
            .login_from("192.168.1.20", None, "admin", ADMIN_PASSWORD)
            .await;
        assert_eq!(
            lan.status,
            StatusCode::TOO_MANY_REQUESTS,
            "with a proxy configured, local addresses are limited too"
        );
        let untrusted = app
            .login_from(
                "198.51.100.8",
                Some("203.0.113.50"),
                "admin",
                ADMIN_PASSWORD,
            )
            .await;
        assert_eq!(untrusted.status, StatusCode::OK, "keyed by the peer itself");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sessions_slide_and_expire_after_thirty_idle_days() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let day = Duration::from_secs(24 * 3600);

        let fresh = app.get("/api/home", &cookie).await;
        assert_eq!(fresh.status, StatusCode::OK);
        assert!(fresh.headers.get(SET_COOKIE).is_none(), "not refreshed yet");

        app.state.auth.advance(2 * day);
        let slid = app.get("/api/home", &cookie).await;
        assert_eq!(slid.status, StatusCode::OK);
        assert_eq!(slid.cookie(), cookie, "same token, new Max-Age");

        app.state.auth.advance(29 * day);
        assert_eq!(
            app.get("/api/home", &cookie).await.status,
            StatusCode::OK,
            "31 days after sign in, 29 after the last refresh"
        );

        app.state.auth.advance(31 * day);
        let expired = app.get("/api/home", &cookie).await;
        assert_eq!(expired.status, StatusCode::UNAUTHORIZED);
        assert_eq!(expired.json()["code"], "unauthorized");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn logout_ends_the_session() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let reply = app
            .call("POST", "/api/auth/logout", Some(&cookie), None)
            .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        assert!(
            reply
                .headers
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
        assert_eq!(
            app.get("/api/home", &cookie).await.status,
            StatusCode::UNAUTHORIZED
        );
        let state = app
            .call("GET", "/api/auth/state", Some(&cookie), None)
            .await
            .json();
        assert_eq!(state["user"], json!(null));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_catalog_route_needs_a_session() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        for uri in [
            "/api/home",
            "/api/rows/all_movies",
            "/api/items/1",
            "/api/images/000102030405060708090a0b0c0d0e0f/small",
            "/api/files/1/stream",
            "/api/scan",
            "/api/admin/users",
        ] {
            let reply = app.call("GET", uri, None, None).await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{uri}");
            assert_eq!(reply.json()["code"], "unauthorized", "{uri}");
            let reply = app.get(uri, "nuvora_session=forged").await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{uri}");
        }

        let token = cookie.strip_prefix("nuvora_session=").unwrap();
        let bearer = app
            .send(
                Request::builder()
                    .uri("/api/home")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(
            bearer.status,
            StatusCode::OK,
            "bearer works like the cookie"
        );
        let basic = app
            .send(
                Request::builder()
                    .uri("/api/home")
                    .header(AUTHORIZATION, "Basic YWxpY2U6c2VjcmV0")
                    .header(COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(
            basic.status,
            StatusCode::OK,
            "a proxy's Basic auth falls through to the cookie"
        );
        let lowercase = app
            .send(
                Request::builder()
                    .uri("/api/home")
                    .header(AUTHORIZATION, format!("bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(lowercase.status, StatusCode::OK, "the scheme ignores case");
        let forged = app
            .send(
                Request::builder()
                    .uri("/api/home")
                    .header(AUTHORIZATION, "Bearer forged")
                    .header(COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(
            forged.status,
            StatusCode::UNAUTHORIZED,
            "a bearer token is used even when a cookie is present"
        );
        assert_eq!(
            app.call("GET", "/api/health", None, None).await.json(),
            json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")})
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn changes_must_be_json_from_this_origin() {
        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let form = app
            .send(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header(COOKIE, &cookie)
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(form.status, StatusCode::FORBIDDEN);
        assert_eq!(form.json()["code"], "forbidden");

        let cross_site = |origin: &'static str| {
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("host", "nuvora.local:8640")
                .header(ORIGIN, origin)
                .header(COOKIE, &cookie)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .unwrap()
        };
        let evil = app.send(cross_site("https://evil.example")).await;
        assert_eq!(evil.status, StatusCode::FORBIDDEN);
        assert_eq!(app.get("/api/home", &cookie).await.status, StatusCode::OK);

        let same = app.send(cross_site("http://nuvora.local:8640")).await;
        assert_eq!(same.status, StatusCode::NO_CONTENT);
    }

    /// A sign out relayed by a proxy on 127.0.0.1 that keeps `Host` as its own address.
    async fn proxied(
        app: &TestApp,
        cookie: &str,
        origin: &str,
        forwarded_host: Option<&str>,
    ) -> super::super::test_app::Reply {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/auth/logout")
            .header("host", "127.0.0.1:8640")
            .header(ORIGIN, origin)
            .header(COOKIE, cookie)
            .header(CONTENT_TYPE, "application/json");
        if let Some(host) = forwarded_host {
            request = request.header("x-forwarded-host", host);
        }
        app.send_from("127.0.0.1", request.body(Body::empty()).unwrap())
            .await
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn behind_a_trusted_proxy_origin_matches_the_forwarded_host() {
        let app = TestApp::behind(&["127.0.0.1"]).await;
        let cookie = app.setup_admin().await;
        let evil = proxied(
            &app,
            &cookie,
            "https://evil.example",
            Some("nuvora.example.com"),
        )
        .await;
        assert_eq!(evil.status, StatusCode::FORBIDDEN);
        let host_only = proxied(&app, &cookie, "https://nuvora.example.com", None).await;
        assert_eq!(
            host_only.status,
            StatusCode::FORBIDDEN,
            "falls back to Host"
        );
        let ok = proxied(
            &app,
            &cookie,
            "https://nuvora.example.com",
            Some("nuvora.example.com"),
        )
        .await;
        assert_eq!(ok.status, StatusCode::NO_CONTENT);

        let app = TestApp::new().await;
        let cookie = app.setup_admin().await;
        let untrusted = proxied(
            &app,
            &cookie,
            "https://nuvora.example.com",
            Some("nuvora.example.com"),
        )
        .await;
        assert_eq!(
            untrusted.status,
            StatusCode::FORBIDDEN,
            "an untrusted peer's X-Forwarded-Host is ignored"
        );
        let direct = proxied(&app, &cookie, "http://127.0.0.1:8640", Some("evil.example")).await;
        assert_eq!(direct.status, StatusCode::NO_CONTENT);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_password_change_keeps_this_session_and_ends_the_others() {
        let app = TestApp::new().await;
        let here = app.setup_admin().await;
        let elsewhere = app.login("admin", ADMIN_PASSWORD).await.cookie();
        let wrong = app
            .call(
                "PATCH",
                "/api/auth/me",
                Some(&here),
                Some(json!({"current_password": "nope nope", "new_password": "new password"})),
            )
            .await;
        assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
        assert_eq!(wrong.json()["field"], "current_password");

        let reply = app
            .call(
                "PATCH",
                "/api/auth/me",
                Some(&here),
                Some(json!({
                    "display_name": "  The Admin ",
                    "current_password": ADMIN_PASSWORD,
                    "new_password": "new password",
                })),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json()["display_name"], "The Admin");
        assert_eq!(app.get("/api/home", &here).await.status, StatusCode::OK);
        assert_eq!(
            app.get("/api/home", &elsewhere).await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.login("admin", "new password").await.status,
            StatusCode::OK
        );
    }
}
