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

#![allow(dead_code)]

use axum::Json;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, ErrorBody, JsonBody};

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
pub(super) async fn auth_state() -> Result<Json<AuthState>, ApiError> {
    Err(ApiError::not_implemented())
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
pub(super) async fn setup(JsonBody(_): JsonBody<SetupRequest>) -> Result<Json<User>, ApiError> {
    Err(ApiError::not_implemented())
}

/// Sets the session cookie. Wrong username, wrong password and disabled
/// account all answer 401 `bad_credentials`. After 5 failures for one username
/// or address within 15 minutes, answers 429 `rate_limited` for 15 minutes.
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
pub(super) async fn login(JsonBody(_): JsonBody<LoginRequest>) -> Result<Json<User>, ApiError> {
    Err(ApiError::not_implemented())
}

/// Ends this session and clears the cookie.
#[utoipa::path(post, path = "/api/auth/logout", responses((status = 204)))]
pub(super) async fn logout() -> Result<StatusCode, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(
    patch,
    path = "/api/auth/me",
    request_body = UpdateMeRequest,
    responses((status = 200, body = User), (status = 400, body = ErrorBody)),
)]
pub(super) async fn update_me(
    JsonBody(_): JsonBody<UpdateMeRequest>,
) -> Result<Json<User>, ApiError> {
    Err(ApiError::not_implemented())
}
