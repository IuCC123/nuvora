//! The admin panel's Transcoding settings and its Activity page: what the
//! server is converting right now, how far each conversion has got, and the
//! library's background work.
//!
//! | method | path                  | purpose                                       |
//! |--------|-----------------------|-----------------------------------------------|
//! | GET    | /api/admin/tuning     | Transcoding settings, defaults, the encoder   |
//! | PUT    | /api/admin/tuning     | Save them; new streams use them               |
//! | GET    | /api/admin/activity   | Streams, scan and seek previews, right now    |

use axum::Json;
use axum::extract::State;
use nuvora_library::{FileId, UserId};
use nuvora_media::{AudioOutput, VideoPlan};
use serde::Serialize;
use utoipa::ToSchema;

use super::{
    ApiError, AppState, AudioTrack, ErrorBody, JsonBody, MediaDetails, ScanStatusResponse,
    VideoTrack, scan_status_wire, unix_ms,
};
use crate::config::HardwareEncoding;
use crate::transcode::{RunPhase, StreamActivity};
use crate::tuning::Tuning;

#[derive(Serialize, ToSchema)]
pub struct TuningResponse {
    pub settings: Tuning,
    /// What every setting starts as.
    pub defaults: Tuning,
    pub encoder: EncoderStatus,
}

#[derive(Serialize, ToSchema)]
pub struct EncoderStatus {
    /// What converts video for streams that start now: `"x264 (processor)"`,
    /// `"VideoToolbox (Apple graphics)"`.
    pub active: String,
    pub hardware: bool,
    /// Set by `nuvora.toml` or `--hardware-encoding`, which win over
    /// `settings.hardware_encoding`.
    pub pinned: Option<HardwareEncoding>,
    /// Why the chosen graphics encoder isn't the one in use.
    pub problem: Option<String>,
}

#[utoipa::path(get, path = "/api/admin/tuning", responses((status = 200, body = TuningResponse)))]
pub(super) async fn get_tuning(State(state): State<AppState>) -> Json<TuningResponse> {
    Json(tuning_response(&state))
}

/// Saves every setting at once. Streams already playing keep their encoder,
/// quality and segment length; pacing, limits and timeouts apply to them
/// too. Changing what the encoder depends on converts a test clip first, so
/// this can take a few seconds.
#[utoipa::path(
    put,
    path = "/api/admin/tuning",
    request_body = Tuning,
    responses((status = 200, body = TuningResponse), (status = 400, body = ErrorBody)),
)]
pub(super) async fn put_tuning(
    State(state): State<AppState>,
    JsonBody(mut body): JsonBody<Tuning>,
) -> Result<Json<TuningResponse>, ApiError> {
    body.vaapi_device = body.vaapi_device.trim().to_owned();
    body.validate()
        .map_err(|(field, message)| ApiError::validation(field, message))?;
    body.save(&state.library)
        .await
        .map_err(ApiError::internal)?;
    state.library.set_previews_enabled(body.seek_previews);
    state.transcoder.retune(body).await;
    Ok(Json(tuning_response(&state)))
}

fn tuning_response(state: &AppState) -> TuningResponse {
    let transcoder = &state.transcoder;
    let encoder = transcoder.encoder();
    TuningResponse {
        settings: (*transcoder.tuning()).clone(),
        defaults: Tuning::default(),
        encoder: EncoderStatus {
            active: encoder.label().to_owned(),
            hardware: encoder.is_hardware(),
            pinned: transcoder.pinned(),
            problem: transcoder.encoder_problem(),
        },
    }
}

#[derive(Serialize, ToSchema)]
pub struct Activity {
    /// Oldest first.
    pub streams: Vec<ActiveStream>,
    pub scan: ScanStatusResponse,
    pub previews: PreviewQueue,
}

/// One HLS stream. Times are in seconds of the video (0 is its first frame).
#[derive(Serialize, ToSchema)]
pub struct ActiveStream {
    pub id: String,
    /// Display name of who is watching.
    pub user: String,
    /// The movie, or `Show · S1E3`.
    pub title: String,
    pub file_id: i64,
    pub state: StreamState,
    /// For `failed`: FFmpeg's last words.
    pub error: Option<String>,
    pub container: String,
    pub source_video: Option<VideoTrack>,
    /// The audio track playing; `None` for a video without sound.
    pub source_audio: Option<AudioTrack>,
    /// `None`: the picture is copied as it is.
    pub video_output: Option<VideoOutput>,
    /// `None`: no audio. `copy`: as it is.
    pub audio_output: Option<AudioOutputWire>,
    pub duration_s: f64,
    /// Where the player is: the segment it asked for last.
    pub position_s: f64,
    /// What is made and still on disk.
    pub made: Vec<Span>,
    /// The current FFmpeg run: where it started and how far it has got.
    pub run: Option<Span>,
    pub run_for_s: Option<f64>,
    /// Seconds of video made per second; above 1 keeps ahead of the player.
    pub speed: Option<f64>,
    pub fps: Option<f64>,
    pub frames: Option<u64>,
    pub bitrate_kbps: Option<f64>,
    pub segments: u32,
    pub disk_bytes: u64,
    /// Since the player last asked for anything.
    pub idle_s: u64,
    /// Unix ms.
    pub opened_at: i64,
}

/// Seconds of the video.
#[derive(Serialize, ToSchema)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

impl From<(f64, f64)> for Span {
    fn from((start, end): (f64, f64)) -> Self {
        Span { start, end }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    /// No FFmpeg running: none asked for yet, or between runs.
    Waiting,
    Converting,
    /// Far enough ahead of the player; resumes as it catches up.
    Paused,
    /// Made to the end of the video.
    Finished,
    Failed,
}

#[derive(Serialize, ToSchema)]
pub struct VideoOutput {
    /// Always `"H.264"`.
    pub codec: String,
    /// At most this many lines (smaller sources keep their own).
    pub max_height: u32,
    pub encoder: String,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioOutputWire {
    Copy,
    Aac { channels: u8, kbps: u32 },
}

#[derive(Serialize, ToSchema)]
pub struct PreviewQueue {
    /// Off in Transcoding settings: nothing new is made.
    pub enabled: bool,
    /// The file being worked on.
    pub current: Option<String>,
    pub queued: u32,
    /// Since the server started.
    pub made: u32,
    pub failed: u32,
}

#[utoipa::path(get, path = "/api/admin/activity", responses((status = 200, body = Activity)))]
pub(super) async fn get_activity(
    State(state): State<AppState>,
) -> Result<Json<Activity>, ApiError> {
    let mut streams = Vec::new();
    for stream in state.transcoder.activity().await {
        streams.push(active_stream(&state, stream).await?);
    }
    let previews = state.library.preview_activity();
    Ok(Json(Activity {
        streams,
        scan: scan_status_wire(state.library.scan_status()),
        previews: PreviewQueue {
            enabled: previews.enabled,
            current: previews
                .current
                .as_deref()
                .and_then(std::path::Path::file_name)
                .map(|n| n.to_string_lossy().into_owned()),
            queued: previews.queued as u32,
            made: previews.made,
            failed: previews.failed,
        },
    }))
}

async fn active_stream(state: &AppState, s: StreamActivity) -> Result<ActiveStream, ApiError> {
    let library = &state.library;
    let user = library
        .user(UserId::from_raw(s.user))
        .await
        .map_err(ApiError::internal)?
        .map_or_else(|| "Someone".to_owned(), |u| u.display_name);
    // Torrent streams have negative keys.
    let title = if s.file < 0 {
        state
            .streams
            .by_key(s.file)
            .map_or_else(|| "A stream that has ended".to_owned(), |t| t.title())
    } else {
        library
            .file_title(FileId::from_raw(s.file))
            .await
            .map_err(ApiError::internal)?
            .unwrap_or_else(|| "A video no longer in the library".to_owned())
    };
    let audio_index = s.plan.audio.map(|a| a.stream);
    let details = MediaDetails::from(s.media);
    let video_output = match s.plan.video {
        VideoPlan::Copy => None,
        VideoPlan::H264 { max_height } => Some(VideoOutput {
            codec: "H.264".to_owned(),
            max_height: s.options.video_height(max_height),
            encoder: s.encoder.unwrap_or_default().to_owned(),
        }),
    };
    let audio_output = s.plan.audio.map(|a| match a.output {
        AudioOutput::Copy => AudioOutputWire::Copy,
        AudioOutput::Aac { channels } => {
            let (channels, kbps) = s.options.aac(channels);
            AudioOutputWire::Aac { channels, kbps }
        }
    });
    let (state_wire, error) = match s.state {
        RunPhase::Waiting => (StreamState::Waiting, None),
        RunPhase::Converting => (StreamState::Converting, None),
        RunPhase::Paused => (StreamState::Paused, None),
        RunPhase::Finished => (StreamState::Finished, None),
        RunPhase::Failed(why) => (StreamState::Failed, Some(why)),
    };
    let progress = s.progress.as_ref();
    Ok(ActiveStream {
        id: s.id,
        user,
        title,
        file_id: s.file,
        state: state_wire,
        error,
        container: details.container,
        source_video: details.video.into_iter().next(),
        source_audio: details
            .audio
            .into_iter()
            .find(|a| Some(a.index) == audio_index),
        video_output,
        audio_output,
        duration_s: s.duration,
        position_s: s.position,
        made: s.made.into_iter().map(Span::from).collect(),
        run: s.run_span.map(Span::from),
        run_for_s: s.run_for.map(|d| d.as_secs_f64()),
        speed: progress.and_then(|p| p.speed),
        fps: progress.map(|p| p.fps),
        frames: progress.map(|p| p.frame),
        bitrate_kbps: progress.and_then(|p| p.bitrate_kbps),
        segments: s.segments as u32,
        disk_bytes: s.disk_bytes,
        idle_s: s.idle.as_secs(),
        opened_at: unix_ms(s.opened),
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use super::super::test_app::TestApp;

    #[tokio::test]
    async fn settings_save_validate_and_survive_a_reload() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let got = app
            .call("GET", "/api/admin/tuning", Some(&admin), None)
            .await;
        assert_eq!(got.status, StatusCode::OK);
        let body: Value = got.json();
        assert_eq!(body["settings"], body["defaults"]);
        assert_eq!(
            body["encoder"]["pinned"], "off",
            "the test server pins x264"
        );
        assert_eq!(body["encoder"]["active"], "x264 (processor)");

        let mut settings = body["settings"].clone();
        settings["x264_preset"] = json!("slow");
        settings["x264_crf"] = json!(18);
        settings["night_mode"] = json!(true);
        settings["max_mbps"] = json!(20);
        let saved = app
            .call(
                "PUT",
                "/api/admin/tuning",
                Some(&admin),
                Some(settings.clone()),
            )
            .await;
        assert_eq!(saved.status, StatusCode::OK);
        assert_eq!(saved.json()["settings"], settings);
        let reloaded = crate::tuning::Tuning::load(&app.state.library).await;
        assert_eq!(serde_json::to_value(&reloaded).unwrap(), settings);

        settings["segment_seconds"] = json!(60);
        let refused = app
            .call("PUT", "/api/admin/tuning", Some(&admin), Some(settings))
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        assert_eq!(refused.json()["field"], "segment_seconds");
    }

    #[tokio::test]
    async fn activity_is_admin_only_and_starts_quiet() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let got = app
            .call("GET", "/api/admin/activity", Some(&admin), None)
            .await;
        assert_eq!(got.status, StatusCode::OK);
        let body: Value = got.json();
        assert_eq!(body["streams"], json!([]));
        assert_eq!(body["previews"]["enabled"], true);
        assert!(body["scan"]["state"].is_string());
        let anonymous = app.call("GET", "/api/admin/activity", None, None).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    }
}
