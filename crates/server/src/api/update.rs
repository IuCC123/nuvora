//! Updating the server from Settings › General ([`crate::update`]).
//!
//! | method | path              | purpose                                             |
//! |--------|-------------------|-----------------------------------------------------|
//! | GET    | /api/admin/update | This version, the newest, and install progress      |
//! | POST   | /api/admin/update | Download and install the newest version, then restart |

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, AppState, ErrorBody, ErrorCode};
use crate::update::{Phase, VERSION};

#[derive(Serialize, ToSchema)]
pub struct UpdateStatus {
    /// This server's version.
    pub current: String,
    /// The newest release, when the last check worked.
    pub latest: Option<String>,
    /// The newest release's page, with what changed.
    pub release_url: Option<String>,
    /// `latest` is newer than `current`.
    pub available: bool,
    /// The server can install `latest` itself: the release has a download for
    /// this platform. Otherwise the admin updates it by hand.
    pub can_install: bool,
    /// False when the server was started with `NUVORA_UPDATES=false`; nothing
    /// else here is known then.
    pub checks: bool,
    pub phase: UpdatePhase,
    /// Why the check or the install failed, as a sentence.
    pub error: Option<String>,
}

#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Downloading,
    /// Installed; the server is restarting as the new version.
    Restarting,
    Failed,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UpdateQuery {
    /// Ask GitHub now rather than answering from the last check (at most once a minute).
    #[serde(default)]
    pub refresh: bool,
}

#[utoipa::path(
    get,
    path = "/api/admin/update",
    params(UpdateQuery),
    responses((status = 200, body = UpdateStatus)),
)]
pub(super) async fn get_update(
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
) -> Json<UpdateStatus> {
    Json(status(&state, query.refresh).await)
}

#[utoipa::path(
    post,
    path = "/api/admin/update",
    responses((status = 202, body = UpdateStatus), (status = 409, body = ErrorBody)),
)]
pub(super) async fn install_update(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<UpdateStatus>), ApiError> {
    let release = match state.updater.latest(false).await {
        Some(Ok(release)) => release,
        Some(Err(message)) => {
            return Err(ApiError::conflict(ErrorCode::UpdateUnavailable, message));
        }
        None => {
            return Err(ApiError::conflict(
                ErrorCode::UpdateUnavailable,
                "Update checks are turned off on this server.",
            ));
        }
    };
    if release.version <= crate::update::Version::current() {
        return Err(ApiError::conflict(
            ErrorCode::UpdateUnavailable,
            "This server is up to date.",
        ));
    }
    if !release.installable() {
        return Err(ApiError::conflict(
            ErrorCode::UpdateUnavailable,
            "This release has no server download for this computer.",
        ));
    }
    state.updater.install(release);
    Ok((StatusCode::ACCEPTED, Json(status(&state, false).await)))
}

async fn status(state: &AppState, refresh: bool) -> UpdateStatus {
    let updater = &state.updater;
    let latest = updater.latest(refresh).await;
    let release = latest.as_ref().and_then(|r| r.as_ref().ok());
    let (phase, failed) = match updater.phase() {
        Phase::Idle => (UpdatePhase::Idle, None),
        Phase::Downloading => (UpdatePhase::Downloading, None),
        Phase::Restarting => (UpdatePhase::Restarting, None),
        Phase::Failed(message) => (UpdatePhase::Failed, Some(message)),
    };
    UpdateStatus {
        current: VERSION.to_owned(),
        latest: release.map(|r| r.version.to_string()),
        release_url: release.map(|r| r.page.clone()),
        available: release.is_some_and(|r| r.version > crate::update::Version::current()),
        can_install: release.is_some_and(crate::update::Release::installable),
        checks: updater.enabled(),
        phase,
        error: failed.or_else(|| latest.and_then(Result::err)),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::super::test_app::TestApp;

    #[tokio::test]
    async fn admins_see_the_version_and_that_checks_are_off() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let reply = app.get("/api/admin/update", &admin).await;
        assert_eq!(reply.status, StatusCode::OK);
        let body = reply.json();
        assert_eq!(body["current"], env!("CARGO_PKG_VERSION"));
        assert_eq!(body["checks"], false);
        assert_eq!(body["available"], false);
        assert_eq!(body["phase"], "idle");

        let refused = app
            .call("POST", "/api/admin/update", Some(&admin), None)
            .await;
        assert_eq!(refused.status, StatusCode::CONFLICT);
        assert_eq!(
            refused.json()["message"],
            "Update checks are turned off on this server."
        );
    }
}
