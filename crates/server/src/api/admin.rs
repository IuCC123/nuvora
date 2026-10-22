//! The admin panel: libraries, the folder picker, users, server settings.
//! Every route needs an admin session.
//!
//! | method | path                             | purpose                          |
//! |--------|----------------------------------|----------------------------------|
//! | GET    | /api/admin/libraries             | List libraries                   |
//! | POST   | /api/admin/libraries             | Add a library, starts a scan     |
//! | PATCH  | /api/admin/libraries/{id}        | Rename, change folders           |
//! | DELETE | /api/admin/libraries/{id}        | Remove a library and its items   |
//! | POST   | /api/admin/libraries/{id}/scan   | Scan now                         |
//! | GET    | /api/admin/fs                    | Folder picker                    |
//! | GET    | /api/admin/users                 | List users                       |
//! | POST   | /api/admin/users                 | Add a user                       |
//! | PATCH  | /api/admin/users/{id}            | Role, disable, reset password    |
//! | DELETE | /api/admin/users/{id}            | Delete a user                    |
//! | GET    | /api/admin/server                | Server info                      |
//! | PATCH  | /api/admin/server                | Server name                      |

#![allow(dead_code)]

use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::accounts::Role;
use super::{ApiError, ErrorBody, JsonBody, ScanStatusResponse};

#[derive(Serialize, ToSchema)]
pub struct AdminLibrary {
    pub id: i64,
    pub name: String,
    pub kind: LibraryKind,
    /// Absolute folder paths on the server.
    pub paths: Vec<String>,
    /// Items with at least one file present.
    pub item_count: u32,
    /// Entries of `paths` that cannot be read right now (unmounted share,
    /// permissions). Their items stay in the catalog.
    pub missing_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Movies,
}

/// `paths` are absolute folders that exist on the server; at least one, and
/// none already used by another library.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub kind: LibraryKind,
    pub paths: Vec<String>,
}

/// Every field is optional. `paths` replaces the whole list; removing a folder
/// removes its items from the catalog (never from disk).
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateLibraryRequest {
    pub name: Option<String>,
    pub paths: Option<Vec<String>>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FsQuery {
    /// An absolute folder. Absent: the filesystem roots (`/` on Unix, drive
    /// letters on Windows).
    pub path: Option<String>,
}

/// One folder's subfolders. Files and hidden folders are never listed.
#[derive(Serialize, ToSchema)]
pub struct FsListing {
    /// The listed folder, canonical. Empty for the list of Windows drives.
    pub path: String,
    /// The folder above `path`; `None` at a root.
    pub parent: Option<String>,
    /// Sorted by name, ignoring case.
    pub dirs: Vec<FsDir>,
}

#[derive(Serialize, ToSchema)]
pub struct FsDir {
    pub name: String,
    /// Absolute; pass it back as `?path=` to open the folder.
    pub path: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    /// Unix ms.
    pub created_at: i64,
    pub disabled: bool,
    /// Unix ms of the latest request from any of the user's sessions;
    /// `None` when the user has no session.
    pub last_seen_at: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role: Role,
}

/// Every field is optional. Disabling a user or resetting their password signs
/// them out everywhere. The last enabled admin cannot be demoted or disabled,
/// and nobody can disable themselves.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub role: Option<Role>,
    pub disabled: Option<bool>,
    pub password: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    pub server_name: String,
    pub version: String,
    pub data_dir: String,
    /// The listen address, e.g. `"0.0.0.0:8640"`.
    pub bind: String,
    pub ffmpeg: ToolInfo,
    pub ffprobe: ToolInfo,
    pub uptime_s: u64,
}

#[derive(Serialize, ToSchema)]
pub struct ToolInfo {
    pub path: String,
    /// As the tool reports it, e.g. `"7.1.1"`.
    pub version: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateServerRequest {
    pub server_name: String,
}

#[utoipa::path(get, path = "/api/admin/libraries", responses((status = 200, body = Vec<AdminLibrary>)))]
pub(super) async fn list_libraries() -> Result<Json<Vec<AdminLibrary>>, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(
    post,
    path = "/api/admin/libraries",
    request_body = CreateLibraryRequest,
    responses((status = 201, body = AdminLibrary), (status = 400, body = ErrorBody)),
)]
pub(super) async fn create_library(
    JsonBody(_): JsonBody<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<AdminLibrary>), ApiError> {
    Err(ApiError::not_implemented())
}

/// Starts a scan when `paths` changed.
#[utoipa::path(
    patch,
    path = "/api/admin/libraries/{id}",
    params(("id" = i64, Path)),
    request_body = UpdateLibraryRequest,
    responses(
        (status = 200, body = AdminLibrary),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn update_library(
    Path(_): Path<i64>,
    JsonBody(_): JsonBody<UpdateLibraryRequest>,
) -> Result<Json<AdminLibrary>, ApiError> {
    Err(ApiError::not_implemented())
}

/// Removes the library and its items from the catalog. Files on disk are untouched.
#[utoipa::path(
    delete,
    path = "/api/admin/libraries/{id}",
    params(("id" = i64, Path)),
    responses((status = 204), (status = 404, body = ErrorBody)),
)]
pub(super) async fn delete_library(Path(_): Path<i64>) -> Result<StatusCode, ApiError> {
    Err(ApiError::not_implemented())
}

/// Scan now. Scans run over every library and coalesce, so this is the same
/// scan as `POST /api/scan`; the id must exist.
#[utoipa::path(
    post,
    path = "/api/admin/libraries/{id}/scan",
    params(("id" = i64, Path)),
    responses((status = 202, body = ScanStatusResponse), (status = 404, body = ErrorBody)),
)]
pub(super) async fn scan_library(
    Path(_): Path<i64>,
) -> Result<(StatusCode, Json<ScanStatusResponse>), ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(
    get,
    path = "/api/admin/fs",
    params(FsQuery),
    responses((status = 200, body = FsListing), (status = 400, body = ErrorBody)),
)]
pub(super) async fn list_dirs(Query(_): Query<FsQuery>) -> Result<Json<FsListing>, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(get, path = "/api/admin/users", responses((status = 200, body = Vec<AdminUser>)))]
pub(super) async fn list_users() -> Result<Json<Vec<AdminUser>>, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(
    post,
    path = "/api/admin/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, body = AdminUser),
        (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ),
)]
pub(super) async fn create_user(
    JsonBody(_): JsonBody<CreateUserRequest>,
) -> Result<(StatusCode, Json<AdminUser>), ApiError> {
    Err(ApiError::not_implemented())
}

/// 409 `last_admin` when the change would leave no enabled admin.
#[utoipa::path(
    patch,
    path = "/api/admin/users/{id}",
    params(("id" = i64, Path)),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, body = AdminUser),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ),
)]
pub(super) async fn update_user(
    Path(_): Path<i64>,
    JsonBody(_): JsonBody<UpdateUserRequest>,
) -> Result<Json<AdminUser>, ApiError> {
    Err(ApiError::not_implemented())
}

/// 409 `last_admin` for the last enabled admin; 400 for yourself.
#[utoipa::path(
    delete,
    path = "/api/admin/users/{id}",
    params(("id" = i64, Path)),
    responses(
        (status = 204),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ),
)]
pub(super) async fn delete_user(Path(_): Path<i64>) -> Result<StatusCode, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(get, path = "/api/admin/server", responses((status = 200, body = ServerInfo)))]
pub(super) async fn get_server() -> Result<Json<ServerInfo>, ApiError> {
    Err(ApiError::not_implemented())
}

#[utoipa::path(
    patch,
    path = "/api/admin/server",
    request_body = UpdateServerRequest,
    responses((status = 200, body = ServerInfo), (status = 400, body = ErrorBody)),
)]
pub(super) async fn update_server(
    JsonBody(_): JsonBody<UpdateServerRequest>,
) -> Result<Json<ServerInfo>, ApiError> {
    Err(ApiError::not_implemented())
}
