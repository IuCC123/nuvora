//! The admin panel: libraries, the folder picker, users, server settings.
//! Every route needs an admin session.
//!
//! | method | path                             | purpose                          |
//! |--------|----------------------------------|----------------------------------|
//! | GET    | /api/admin/libraries             | List libraries                   |
//! | POST   | /api/admin/libraries             | Add a library, starts a scan     |
//! | PATCH  | /api/admin/libraries/{id}        | Rename, change folders or type   |
//! | DELETE | /api/admin/libraries/{id}        | Remove a library and its items   |
//! | POST   | /api/admin/libraries/{id}/scan   | Scan now                         |
//! | GET    | /api/admin/fs                    | Folder picker                    |
//! | GET    | /api/admin/fs/inspect            | Suggest a library type           |
//! | GET    | /api/admin/users                 | List users                       |
//! | POST   | /api/admin/users                 | Add a user                       |
//! | PATCH  | /api/admin/users/{id}            | Role, disable, reset password    |
//! | DELETE | /api/admin/users/{id}            | Delete a user                    |
//! | GET    | /api/admin/server                | Server info                      |
//! | PATCH  | /api/admin/server                | Server name                      |

use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use nuvora_library::{LibraryError, LibraryId, LibraryInfo, NewUser, UserChanges, UserId};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::accounts::{
    Role, account_error, server_name, valid_display_name, valid_password, valid_username,
};
use super::{ApiError, AppState, ErrorBody, JsonBody, ScanStatusResponse, scan_status_wire};
use crate::auth::{self, SignedIn};

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

/// Chosen when the library is added. Changing it later rebuilds the
/// library's items.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Movies,
    /// TV Shows: files are episodes, grouped into shows and seasons.
    Shows,
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
/// removes its items from the catalog (never from disk). A different `kind`
/// rebuilds every item of the library and resets watch progress in it (ask
/// first); a scan then fills in artwork.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateLibraryRequest {
    pub name: Option<String>,
    pub paths: Option<Vec<String>>,
    pub kind: Option<LibraryKind>,
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

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FsInspectQuery {
    /// An absolute folder.
    pub path: String,
}

/// A quick look at a folder before adding it as a library: video files under
/// it, and how many of them are named like episodes. Counts at most 2,000
/// video files.
#[derive(Serialize, ToSchema)]
pub struct FsInspect {
    pub video_files: u32,
    pub episode_like: u32,
    /// `shows` when more than half of the video files look like episodes.
    pub suggested_kind: LibraryKind,
    /// The count stopped at 2,000 files.
    pub truncated: bool,
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
pub(super) async fn list_libraries(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminLibrary>>, ApiError> {
    let libraries = state
        .library
        .libraries()
        .await
        .map_err(ApiError::internal)?;
    let mut out = Vec::with_capacity(libraries.len());
    for library in libraries {
        out.push(admin_library(library).await);
    }
    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/api/admin/libraries",
    request_body = CreateLibraryRequest,
    responses((status = 201, body = AdminLibrary), (status = 400, body = ErrorBody)),
)]
pub(super) async fn create_library(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<AdminLibrary>), ApiError> {
    let name = valid_library_name(&body.name)?;
    let roots = valid_folders(body.paths).await?;
    let kind = body.kind.into();
    let library = state
        .library
        .create_library(name, kind, roots)
        .await
        .map_err(library_error)?;
    state.library.request_scan();
    Ok((StatusCode::CREATED, Json(admin_library(library).await)))
}

/// Starts a scan when `paths` or `kind` changed.
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
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(body): JsonBody<UpdateLibraryRequest>,
) -> Result<Json<AdminLibrary>, ApiError> {
    let name = body.name.as_deref().map(valid_library_name).transpose()?;
    let roots = match body.paths {
        Some(paths) => Some(valid_folders(paths).await?),
        None => None,
    };
    let (library, folders_changed) = state
        .library
        .update_library(
            LibraryId::from_raw(id),
            name,
            roots,
            body.kind.map(LibraryKind::into),
            state.auth.now_ms(),
        )
        .await
        .map_err(library_error)?;
    if folders_changed {
        state.library.request_scan();
    }
    Ok(Json(admin_library(library).await))
}

/// Removes the library and its items from the catalog. Files on disk are untouched.
#[utoipa::path(
    delete,
    path = "/api/admin/libraries/{id}",
    params(("id" = i64, Path)),
    responses((status = 204), (status = 404, body = ErrorBody)),
)]
pub(super) async fn delete_library(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state
        .library
        .delete_library(LibraryId::from_raw(id))
        .await
        .map_err(library_error)?;
    // The scan's cleanup deletes the removed items' artwork files.
    state.library.request_scan();
    Ok(StatusCode::NO_CONTENT)
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
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<ScanStatusResponse>), ApiError> {
    state
        .library
        .library(LibraryId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::not_found)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(scan_status_wire(state.library.request_scan())),
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/fs",
    params(FsQuery),
    responses((status = 200, body = FsListing), (status = 400, body = ErrorBody)),
)]
pub(super) async fn list_dirs(Query(query): Query<FsQuery>) -> Result<Json<FsListing>, ApiError> {
    tokio::task::spawn_blocking(move || list_folder(query.path.as_deref()))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
}

/// Suggests a library type for a folder the admin is about to add.
#[utoipa::path(
    get,
    path = "/api/admin/fs/inspect",
    params(FsInspectQuery),
    responses((status = 200, body = FsInspect), (status = 400, body = ErrorBody)),
)]
pub(super) async fn inspect_dir(
    Query(query): Query<FsInspectQuery>,
) -> Result<Json<FsInspect>, ApiError> {
    tokio::task::spawn_blocking(move || inspect_folder(&query.path))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
}

/// Video files counted by [`inspect_folder`] before it stops.
const INSPECT_FILES: u32 = 2_000;
/// Directory entries read before it stops, whatever they are, so a huge
/// tree (or a symlink loop) cannot keep the request open.
const INSPECT_ENTRIES: u32 = 50_000;

/// Counts video files under `path` as a scan would see them (hidden and
/// extras folders skipped) and how many are named like episodes.
fn inspect_folder(path: &str) -> Result<FsInspect, ApiError> {
    use nuvora_media::{EpisodeName, PathClass, classify_path, is_skipped_dir};
    let requested = PathBuf::from(path);
    if !requested.is_absolute() {
        return Err(ApiError::validation("path", "Use an absolute path"));
    }
    let root = requested
        .canonicalize()
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| ApiError::validation("path", "That folder doesn't exist"))?;
    let (mut video_files, mut episode_like, mut entries) = (0, 0, 0);
    let mut truncated = false;
    let mut folders = vec![root.clone()];
    'walk: while let Some(folder) = folders.pop() {
        let Ok(listing) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in listing.flatten() {
            entries += 1;
            if video_files >= INSPECT_FILES || entries >= INSPECT_ENTRIES {
                truncated = true;
                break 'walk;
            }
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(&root) else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                if !is_skipped_dir(rel) {
                    folders.push(path);
                }
            } else if classify_path(rel) == PathClass::Video {
                video_files += 1;
                if EpisodeName::parse(rel).is_some() {
                    episode_like += 1;
                }
            }
        }
    }
    Ok(FsInspect {
        video_files,
        episode_like,
        suggested_kind: if episode_like * 2 > video_files {
            LibraryKind::Shows
        } else {
            LibraryKind::Movies
        },
        truncated,
    })
}

#[utoipa::path(get, path = "/api/admin/users", responses((status = 200, body = Vec<AdminUser>)))]
pub(super) async fn list_users(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminUser>>, ApiError> {
    let users = state.library.users().await.map_err(ApiError::internal)?;
    Ok(Json(users.into_iter().map(AdminUser::from).collect()))
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
    State(state): State<AppState>,
    JsonBody(body): JsonBody<CreateUserRequest>,
) -> Result<(StatusCode, Json<AdminUser>), ApiError> {
    let new = NewUser {
        username: valid_username(&body.username)?,
        display_name: valid_display_name(&body.display_name)?,
        password_hash: auth::hash_password(valid_password("password", body.password)?).await?,
        role: body.role.into(),
    };
    let user = state
        .library
        .create_user(new, state.auth.now_ms())
        .await
        .map_err(account_error)?;
    Ok((StatusCode::CREATED, Json(AdminUser::from(user))))
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
    State(state): State<AppState>,
    signed_in: SignedIn,
    Path(id): Path<i64>,
    JsonBody(body): JsonBody<UpdateUserRequest>,
) -> Result<Json<AdminUser>, ApiError> {
    let id = UserId::from_raw(id);
    let password_hash = match body.password {
        Some(password) => Some(auth::hash_password(valid_password("password", password)?).await?),
        None => None,
    };
    let changes = UserChanges {
        display_name: body
            .display_name
            .as_deref()
            .map(valid_display_name)
            .transpose()?,
        role: body.role.map(Into::into),
        disabled: body.disabled,
        password_hash,
        ..UserChanges::default()
    };
    let me = signed_in.user.id;
    let keep = (id == me).then_some(signed_in.token);
    let user = state
        .library
        .update_user(me, id, changes, keep, state.auth.now_ms())
        .await
        .map_err(account_error)?;
    Ok(Json(AdminUser::from(user)))
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
pub(super) async fn delete_user(
    State(state): State<AppState>,
    signed_in: SignedIn,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state
        .library
        .delete_user(signed_in.user.id, UserId::from_raw(id))
        .await
        .map_err(account_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/admin/server", responses((status = 200, body = ServerInfo)))]
pub(super) async fn get_server(
    State(state): State<AppState>,
) -> Result<Json<ServerInfo>, ApiError> {
    server_info(&state).await.map(Json)
}

#[utoipa::path(
    patch,
    path = "/api/admin/server",
    request_body = UpdateServerRequest,
    responses((status = 200, body = ServerInfo), (status = 400, body = ErrorBody)),
)]
pub(super) async fn update_server(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<UpdateServerRequest>,
) -> Result<Json<ServerInfo>, ApiError> {
    let name = body.server_name.trim();
    if !(1..=64).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(ApiError::validation(
            "server_name",
            "Use 1 to 64 characters",
        ));
    }
    state
        .library
        .set_server_name(name.to_owned())
        .await
        .map_err(ApiError::internal)?;
    server_info(&state).await.map(Json)
}

async fn server_info(state: &AppState) -> Result<ServerInfo, ApiError> {
    let server = &state.server;
    let tools = &server.tools;
    Ok(ServerInfo {
        server_name: server_name(&state.library).await?,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        data_dir: server.data_dir.display().to_string(),
        bind: server.bind.to_string(),
        ffmpeg: ToolInfo {
            path: on_path(tools.ffmpeg()).display().to_string(),
            version: tools.ffmpeg_version().to_owned(),
        },
        ffprobe: ToolInfo {
            path: on_path(tools.ffprobe()).display().to_string(),
            version: tools.ffprobe_version().to_owned(),
        },
        uptime_s: server.started.elapsed().as_secs(),
    })
}

/// Where a bare tool name such as `ffmpeg` resolves on `PATH`; other paths as given.
fn on_path(tool: &std::path::Path) -> PathBuf {
    if tool.components().count() != 1 {
        return tool.to_path_buf();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(tool))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| tool.to_path_buf())
}

async fn admin_library(library: LibraryInfo) -> AdminLibrary {
    let roots = library.roots.clone();
    let missing = tokio::task::spawn_blocking(move || {
        roots
            .into_iter()
            .filter(|root| std::fs::read_dir(root).is_err())
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    AdminLibrary {
        id: library.id.raw(),
        name: library.name,
        kind: library.kind.into(),
        paths: library
            .roots
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        item_count: library.item_count,
        missing_paths: missing.iter().map(|p| p.display().to_string()).collect(),
    }
}

impl From<LibraryKind> for nuvora_library::LibraryKind {
    fn from(kind: LibraryKind) -> Self {
        match kind {
            LibraryKind::Movies => nuvora_library::LibraryKind::Movies,
            LibraryKind::Shows => nuvora_library::LibraryKind::Shows,
        }
    }
}

impl From<nuvora_library::LibraryKind> for LibraryKind {
    fn from(kind: nuvora_library::LibraryKind) -> Self {
        match kind {
            nuvora_library::LibraryKind::Movies => LibraryKind::Movies,
            nuvora_library::LibraryKind::Shows => LibraryKind::Shows,
        }
    }
}

fn library_error(e: LibraryError) -> ApiError {
    match e {
        LibraryError::PathInUse(path) => ApiError::validation(
            "paths",
            format!("{} is already in another library", path.display()),
        ),
        LibraryError::NotFound => ApiError::not_found(),
        LibraryError::Query(e) => ApiError::internal(e),
    }
}

fn valid_library_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if !(1..=64).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(ApiError::validation("name", "Use 1 to 64 characters"));
    }
    Ok(name.to_owned())
}

/// At least one absolute path to an existing folder; canonical and deduplicated.
async fn valid_folders(paths: Vec<String>) -> Result<Vec<PathBuf>, ApiError> {
    if paths.is_empty() {
        return Err(ApiError::validation("paths", "Choose at least one folder"));
    }
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::with_capacity(paths.len());
        for raw in paths {
            let path = PathBuf::from(&raw);
            let folder = path
                .is_absolute()
                .then(|| path.canonicalize().ok())
                .flatten()
                .filter(|p| p.is_dir())
                .ok_or_else(|| {
                    ApiError::validation("paths", format!("{raw} is not a folder on the server"))
                })?;
            if !out.contains(&folder) {
                out.push(folder);
            }
        }
        Ok(out)
    })
    .await
    .map_err(ApiError::internal)?
}

/// The folder picker's listing of `path`, or of the filesystem roots.
fn list_folder(path: Option<&str>) -> Result<FsListing, ApiError> {
    let Some(raw) = path.filter(|p| !p.is_empty()) else {
        return filesystem_roots();
    };
    let requested = PathBuf::from(raw);
    if !requested.is_absolute() {
        return Err(ApiError::validation("path", "Use an absolute path"));
    }
    let folder = requested
        .canonicalize()
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| ApiError::validation("path", "That folder doesn't exist"))?;
    let entries = std::fs::read_dir(&folder)
        .map_err(|_| ApiError::validation("path", "Nuvora can't read that folder"))?;
    let mut dirs: Vec<FsDir> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let path = entry.path();
            // `metadata` follows symlinks, so a link to a folder is listed.
            let meta = std::fs::metadata(&path).ok()?;
            (meta.is_dir() && !is_hidden(&name, &meta)).then(|| FsDir {
                name,
                path: path.display().to_string(),
            })
        })
        .collect();
    dirs.sort_by_cached_key(|d| d.name.to_lowercase());
    Ok(FsListing {
        path: folder.display().to_string(),
        parent: folder.parent().map(|p| p.display().to_string()),
        dirs,
    })
}

#[cfg(windows)]
fn is_hidden(name: &str, meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    name.starts_with('.') || meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _: &std::fs::Metadata) -> bool {
    name.starts_with('.')
}

/// Unix has one root, so list it directly.
#[cfg(not(windows))]
fn filesystem_roots() -> Result<FsListing, ApiError> {
    list_folder(Some("/"))
}

#[cfg(windows)]
fn filesystem_roots() -> Result<FsListing, ApiError> {
    let dirs = (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|drive| std::path::Path::new(drive).is_dir())
        .map(|drive| FsDir {
            name: drive.clone(),
            path: drive,
        })
        .collect();
    Ok(FsListing {
        path: String::new(),
        parent: None,
        dirs,
    })
}

impl From<nuvora_library::User> for AdminUser {
    fn from(u: nuvora_library::User) -> Self {
        AdminUser {
            id: u.id.raw(),
            username: u.username,
            display_name: u.display_name,
            role: u.role.into(),
            created_at: u.created_at,
            disabled: u.disabled,
            last_seen_at: u.last_seen_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use super::super::test_app::TestApp;

    async fn add_member(app: &TestApp, admin: &str, username: &str) -> Value {
        let reply = app
            .call(
                "POST",
                "/api/admin/users",
                Some(admin),
                Some(json!({"username": username, "display_name": username, "password": "member password", "role": "member"})),
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.json());
        reply.json()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn members_browse_but_get_403_on_admin_routes() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let created = add_member(&app, &admin, "kid").await;
        assert_eq!(created["role"], "member");
        assert_eq!(created["disabled"], false);
        assert_eq!(created["last_seen_at"], Value::Null);

        let member = app.login("kid", "member password").await.cookie();
        assert_eq!(app.get("/api/home", &member).await.status, StatusCode::OK);
        for (method, uri) in [
            ("GET", "/api/admin/users"),
            ("GET", "/api/admin/libraries"),
            ("GET", "/api/admin/fs"),
            ("GET", "/api/admin/server"),
            ("POST", "/api/admin/libraries/1/scan"),
            ("DELETE", "/api/admin/users/1"),
        ] {
            let reply = app.call(method, uri, Some(&member), None).await;
            assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method} {uri}");
            assert_eq!(reply.json()["code"], "forbidden");
        }

        let taken = app
            .call(
                "POST",
                "/api/admin/users",
                Some(&admin),
                Some(json!({"username": "KID", "display_name": "x", "password": "member password", "role": "member"})),
            )
            .await;
        assert_eq!(taken.status, StatusCode::CONFLICT);
        assert_eq!(taken.json()["code"], "username_taken");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_last_admin_cannot_be_demoted_disabled_or_deleted() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let patch = |cookie: &str, id: i64, body: Value| {
            let (app, cookie) = (&app, cookie.to_owned());
            async move {
                app.call(
                    "PATCH",
                    &format!("/api/admin/users/{id}"),
                    Some(&cookie),
                    Some(body),
                )
                .await
            }
        };
        let demote = patch(&admin, 1, json!({"role": "member"})).await;
        assert_eq!(demote.status, StatusCode::CONFLICT);
        assert_eq!(demote.json()["code"], "last_admin");
        let disable = patch(&admin, 1, json!({"disabled": true})).await;
        assert_eq!(
            disable.status,
            StatusCode::BAD_REQUEST,
            "nobody disables themselves"
        );
        let delete = app
            .call("DELETE", "/api/admin/users/1", Some(&admin), None)
            .await;
        assert_eq!(
            delete.status,
            StatusCode::BAD_REQUEST,
            "nobody deletes themselves"
        );

        let second = add_member(&app, &admin, "second").await;
        let id = second["id"].as_i64().unwrap();
        assert_eq!(
            patch(&admin, id, json!({"role": "admin"})).await.status,
            StatusCode::OK
        );
        let second = app.login("second", "member password").await.cookie();
        let demoted = patch(&second, 1, json!({"role": "member"})).await;
        assert_eq!(demoted.status, StatusCode::OK);
        assert_eq!(demoted.json()["role"], "member");
        assert_eq!(
            app.get("/api/admin/users", &admin).await.status,
            StatusCode::FORBIDDEN,
            "a demoted admin loses the panel at once"
        );
        let last = app
            .call(
                "DELETE",
                &format!("/api/admin/users/{id}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(last.status, StatusCode::FORBIDDEN);
        assert_eq!(
            patch(&second, id, json!({"role": "member"})).await.json()["code"],
            "last_admin"
        );
        let gone = app
            .call("DELETE", "/api/admin/users/1", Some(&second), None)
            .await;
        assert_eq!(gone.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/home", &admin).await.status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn disabling_a_user_ends_their_sessions() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let id = add_member(&app, &admin, "friend").await["id"]
            .as_i64()
            .unwrap();
        let friend = app.login("friend", "member password").await.cookie();
        assert_eq!(app.get("/api/home", &friend).await.status, StatusCode::OK);

        let uri = format!("/api/admin/users/{id}");
        let disabled = app
            .call("PATCH", &uri, Some(&admin), Some(json!({"disabled": true})))
            .await;
        assert_eq!(disabled.status, StatusCode::OK);
        assert_eq!(disabled.json()["disabled"], true);
        assert_eq!(
            disabled.json()["last_seen_at"],
            Value::Null,
            "sessions deleted"
        );
        assert_eq!(
            app.get("/api/home", &friend).await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.login("friend", "member password").await.json()["code"],
            "bad_credentials"
        );

        app.call(
            "PATCH",
            &uri,
            Some(&admin),
            Some(json!({"disabled": false, "password": "reset password"})),
        )
        .await;
        app.state.auth.advance(Duration::from_secs(1));
        assert_eq!(
            app.login("friend", "reset password").await.status,
            StatusCode::OK
        );
        let users = app.get("/api/admin/users", &admin).await.json();
        assert_eq!(users.as_array().unwrap().len(), 2);
        assert!(users[1]["last_seen_at"].as_i64().unwrap() > 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn adding_a_library_scans_it() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let films = app.path("films");
        let movie = films.join("Big Buck (2008)/Big Buck (2008).mp4");
        std::fs::create_dir_all(movie.parent().unwrap()).unwrap();
        let ffmpeg = std::process::Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi", "-i"])
            .args([
                "testsrc2=size=320x180:rate=24:duration=1",
                "-c:v",
                "libx264",
            ])
            .args(["-preset", "ultrafast", "-pix_fmt", "yuv420p"])
            .arg(&movie)
            .status()
            .unwrap();
        assert!(ffmpeg.success());

        let bad = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({"name": "Films", "kind": "movies", "paths": ["/no/such/folder"]})),
            )
            .await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad.json()["field"], "paths");

        let created = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({"name": " Films ", "kind": "movies", "paths": [films.display().to_string()]})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED);
        let created = created.json();
        let canonical = films.canonicalize().unwrap().display().to_string();
        assert_eq!(created["name"], "Films");
        assert_eq!(created["paths"], json!([canonical]));
        assert_eq!(created["missing_paths"], json!([]));

        let mut libraries = Value::Null;
        for _ in 0..100 {
            libraries = app.get("/api/admin/libraries", &admin).await.json();
            if libraries[1]["item_count"] == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(libraries[1]["item_count"], 1, "{libraries}");
        let home = app.get("/api/home", &admin).await.json();
        assert_eq!(
            (
                &home["rows"][2]["library_id"],
                &home["rows"][2]["items"][0]["title"]
            ),
            (&created["id"], &json!("Big Buck")),
            "the new library has its own row"
        );

        let again = app
            .call(
                "POST",
                "/api/admin/libraries",
                Some(&admin),
                Some(json!({"name": "Again", "kind": "movies", "paths": [canonical]})),
            )
            .await;
        assert_eq!(
            again.status,
            StatusCode::BAD_REQUEST,
            "a folder is in one library"
        );

        let id = created["id"].as_i64().unwrap();
        let deleted = app
            .call(
                "DELETE",
                &format!("/api/admin/libraries/{id}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(deleted.status, StatusCode::NO_CONTENT);
        let home = app.get("/api/home", &admin).await.json();
        assert_eq!(home["rows"][0]["total"], 0, "nothing recently added");
        assert_eq!(home["rows"].as_array().unwrap().len(), 2, "its row is gone");
        assert!(movie.is_file(), "files on disk are untouched");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_folder_picker_lists_visible_folders_only() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let root = app.path("pick");
        for dir in ["b", "A", ".hidden"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("file.mkv"), b"").unwrap();
        let canonical = root.canonicalize().unwrap();
        let reply = app
            .get(&format!("/api/admin/fs?path={}", root.display()), &admin)
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.json(),
            json!({
                "path": canonical.display().to_string(),
                "parent": canonical.parent().unwrap().display().to_string(),
                "dirs": [
                    {"name": "A", "path": canonical.join("A").display().to_string()},
                    {"name": "b", "path": canonical.join("b").display().to_string()},
                ],
            })
        );

        #[cfg(unix)]
        {
            let top = app.get("/api/admin/fs", &admin).await.json();
            assert_eq!((&top["path"], &top["parent"]), (&json!("/"), &Value::Null));
        }
        let relative = app.get("/api/admin/fs?path=movies", &admin).await;
        assert_eq!(relative.status, StatusCode::BAD_REQUEST);
        assert_eq!(relative.json()["field"], "path");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn server_info_and_name() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let info = app.get("/api/admin/server", &admin).await.json();
        assert_eq!(info["server_name"], "Nuvora");
        assert_eq!(info["bind"], "127.0.0.1:8640");
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert!(
            info["ffmpeg"]["path"].as_str().unwrap().ends_with("ffmpeg"),
            "{info}"
        );
        assert!(
            info["ffprobe"]["version"]
                .as_str()
                .unwrap()
                .chars()
                .next()
                .unwrap()
                .is_ascii_digit(),
            "{info}"
        );

        let renamed = app
            .call(
                "PATCH",
                "/api/admin/server",
                Some(&admin),
                Some(json!({"server_name": "Den"})),
            )
            .await;
        assert_eq!(renamed.json()["server_name"], "Den");
        let state = app.call("GET", "/api/auth/state", None, None).await.json();
        assert_eq!(state["server_name"], "Den");
    }
}
