//! Torrent streams on the wire, for admins: play a release while it
//! downloads ([`crate::streams`]). The status says how this device plays
//! it, as an item's version does: the file as it is, or HLS from the
//! converter, which always converts the picture for a stream.
//!
//! | method | path                                          | purpose                              |
//! |--------|-----------------------------------------------|--------------------------------------|
//! | POST   | /api/admin/streams                            | Start one (or get the one running)   |
//! | GET    | /api/admin/streams/{id}                       | Progress, and how to play it (poll)  |
//! | DELETE | /api/admin/streams/{id}                       | Stop it and delete what it fetched   |
//! | GET    | /api/admin/streams/{id}/file                  | The video as it is, HTTP Range       |
//! | GET    | /api/admin/streams/{id}/hls/{plan}/main.m3u8  | Converted, as for library files      |
//! | GET    | /api/admin/streams/{id}/hls/{plan}/init.mp4   |                                      |
//! | GET    | /api/admin/streams/{id}/hls/{plan}/{segment}  |                                      |

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use nuvora_media::{PlaybackDecision, StreamPlan};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::hls::HlsQuery;
use super::{ApiError, AppState, Device, ErrorBody, JsonBody, MediaDetails, Playback};
use crate::auth::SignedIn;
use crate::streams::{OpenError, Phase, Stream};

#[derive(Deserialize, ToSchema)]
pub struct OpenStreamRequest {
    /// A `magnet:` link, or a search result's `link`.
    pub link: String,
    /// The release's name, shown while it starts.
    pub title: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TorrentStream {
    pub id: String,
    /// The release's name.
    pub title: String,
    /// The movie the name points at, and its year.
    pub movie: String,
    pub year: Option<u16>,
    pub state: TorrentStreamState,
    /// The video being played, once the torrent's details are in.
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    /// Of the video, fetched so far.
    pub done_bytes: Option<u64>,
    /// Bytes per second.
    pub down_bps: u64,
    pub peers: u32,
    /// Once `ready`.
    pub media: Option<MediaDetails>,
    /// What pressing Play does on this device, once `ready`.
    pub playback: Option<Playback>,
    /// Why it stopped, when `failed`.
    pub error: Option<String>,
}

#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TorrentStreamState {
    /// Getting the torrent's details: from the indexer, or from peers for a
    /// magnet link.
    Starting,
    /// Reading the start of the video to learn what it is.
    Probing,
    Ready,
    Failed,
}

fn status(stream: &Stream, device: &Device) -> TorrentStream {
    let title = stream.title();
    let name = super::downloads::movie_name(&title);
    let progress = stream.progress();
    let mut wire = TorrentStream {
        id: stream.id.clone(),
        title,
        movie: name.title,
        year: name.year,
        state: TorrentStreamState::Starting,
        file_name: None,
        size_bytes: None,
        done_bytes: progress.as_ref().map(|p| p.done_bytes),
        down_bps: progress.as_ref().map_or(0, |p| p.down_bps),
        peers: progress.as_ref().map_or(0, |p| p.peers),
        media: None,
        playback: None,
        error: None,
    };
    match stream.phase() {
        Phase::Starting => {}
        Phase::Failed(error) => {
            wire.state = TorrentStreamState::Failed;
            wire.error = Some(error);
        }
        Phase::Probing(file) => {
            wire.state = TorrentStreamState::Probing;
            wire.file_name = Some(file.name);
            wire.size_bytes = Some(file.len);
        }
        Phase::Ready(file, media) => {
            wire.state = TorrentStreamState::Ready;
            wire.file_name = Some(file.name);
            wire.size_bytes = Some(file.len);
            wire.playback = Some(playback(&stream.id, &media, device));
            wire.media = Some(MediaDetails::from((*media).clone()));
        }
    }
    wire
}

/// As for a library file, except that a picture the device can't take as it
/// is gets converted, never copied: copying needs the whole file first.
fn playback(id: &str, media: &nuvora_media::MediaInfo, device: &Device) -> Playback {
    match device.profile.decide(media) {
        PlaybackDecision::DirectPlay => Playback::DirectPlay {
            url: format!("/api/admin/streams/{id}/file"),
            mime_type: media.container.mime_type().to_owned(),
        },
        PlaybackDecision::Convert { plan, reasons } => {
            let audio = plan.audio.map(|a| a.stream);
            match device.profile.plan_converting(media, audio) {
                Some(plan) => Playback::Hls {
                    url: format!("/api/admin/streams/{id}/hls/{}/main.m3u8", plan.code()),
                    reasons: reasons.iter().map(super::reason_text).collect(),
                    converts_video: true,
                    converts_audio: plan
                        .audio
                        .is_some_and(|a| a.output != nuvora_media::AudioOutput::Copy),
                },
                None => Playback::NeedsTranscode {
                    reasons: reasons.iter().map(super::reason_text).collect(),
                },
            }
        }
        PlaybackDecision::Unplayable(reasons) => Playback::NeedsTranscode {
            reasons: reasons.iter().map(super::reason_text).collect(),
        },
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/streams",
    request_body = OpenStreamRequest,
    responses((status = 200, body = TorrentStream), (status = 400, body = ErrorBody)),
)]
pub(super) async fn open_stream(
    State(state): State<AppState>,
    device: Device,
    JsonBody(body): JsonBody<OpenStreamRequest>,
) -> Result<Json<TorrentStream>, ApiError> {
    let link = body.link.trim();
    if link.len() > 8 * 1024 || link.chars().any(char::is_control) {
        return Err(ApiError::validation("link", "That link is too long"));
    }
    let title = body
        .title
        .map(|t| t.trim().chars().take(300).collect::<String>())
        .filter(|t| !t.is_empty())
        .or_else(|| crate::downloads::magnet_name(link));
    let stream = state.streams.open(link, title).map_err(|e| match e {
        OpenError::BadLink => ApiError::validation("link", e.to_string()),
        OpenError::NoEngine(_) => ApiError::refused(e.to_string()),
    })?;
    Ok(Json(status(&stream, &device)))
}

#[utoipa::path(
    get,
    path = "/api/admin/streams/{id}",
    params(("id" = String, Path)),
    responses((status = 200, body = TorrentStream), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_stream(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<String>,
) -> Result<Json<TorrentStream>, ApiError> {
    let stream = state.streams.get(&id).ok_or_else(ApiError::not_found)?;
    Ok(Json(status(&stream, &device)))
}

#[utoipa::path(
    delete,
    path = "/api/admin/streams/{id}",
    params(("id" = String, Path)),
    responses((status = 204), (status = 404, body = ErrorBody)),
)]
pub(super) async fn stop_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state.streams.stop(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

/// The video as it is, as the engine fetches it: the pieces a Range asks
/// for come first.
#[utoipa::path(
    get,
    path = "/api/admin/streams/{id}/file",
    params(("id" = String, Path)),
    responses(
        (status = 200, content_type = "video/*", body = Vec<u8>),
        (status = 206, content_type = "video/*", body = Vec<u8>),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_stream_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let stream = state.streams.get(&id).ok_or_else(ApiError::not_found)?;
    let Phase::Ready(file, media) = stream.phase() else {
        return Err(ApiError::not_found());
    };
    Ok(crate::streams::serve_range(&file, &headers, media.container.mime_type()).await)
}

#[utoipa::path(
    get,
    path = "/api/admin/streams/{id}/hls/{plan}/main.m3u8",
    params(("id" = String, Path), ("plan" = String, Path), HlsQuery),
    responses(
        (status = 200, content_type = "application/vnd.apple.mpegurl", body = String),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_stream_playlist(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan)): Path<(String, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let (key, source) = converting(&state, &id, &plan)?;
    super::hls::playlist(&state, &me, key, &plan, query, source).await
}

#[utoipa::path(
    get,
    path = "/api/admin/streams/{id}/hls/{plan}/init.mp4",
    params(("id" = String, Path), ("plan" = String, Path), HlsQuery),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_stream_init(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan)): Path<(String, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let (key, source) = converting(&state, &id, &plan)?;
    super::hls::init(&state, &me, key, &plan, query, source).await
}

#[utoipa::path(
    get,
    path = "/api/admin/streams/{id}/hls/{plan}/{segment}",
    params(("id" = String, Path), ("plan" = String, Path), ("segment" = String, Path, description = "`{n}.m4s`"), HlsQuery),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_stream_segment(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan, segment)): Path<(String, String, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let (key, source) = converting(&state, &id, &plan)?;
    super::hls::segment(&state, &me, key, &plan, &segment, query, source).await
}

/// A ready stream's converter key and source. 404 for a plan that would
/// copy the picture, which needs the whole file first.
fn converting(
    state: &AppState,
    id: &str,
    plan: &str,
) -> Result<(i64, nuvora_library::StreamSource), ApiError> {
    let converts =
        StreamPlan::parse(plan).is_some_and(|p| p.video != nuvora_media::VideoPlan::Copy);
    if !converts {
        return Err(ApiError::not_found());
    }
    let stream = state.streams.get(id).ok_or_else(ApiError::not_found)?;
    let source = state
        .streams
        .source(&stream)
        .ok_or_else(ApiError::not_found)?;
    Ok((stream.key, source))
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};

    use crate::api::test_app::TestApp;

    /// Starts streaming `link` and waits until it is ready or failed.
    async fn ready(app: &TestApp, admin: &str, link: &str) -> Value {
        let opened = app
            .call(
                "POST",
                "/api/admin/streams",
                Some(admin),
                Some(json!({ "link": link })),
            )
            .await;
        assert_eq!(
            opened.status,
            StatusCode::OK,
            "{:?}",
            String::from_utf8_lossy(&opened.body)
        );
        let id = opened.json()["id"].as_str().unwrap().to_owned();
        let started = Instant::now();
        loop {
            let stream = app
                .get(&format!("/api/admin/streams/{id}"), admin)
                .await
                .json();
            if matches!(stream["state"].as_str(), Some("ready" | "failed")) {
                return stream;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "stuck: {stream}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn encode(app: &TestApp, rel: &str, codecs: &[&str]) {
        let out = app.path(rel);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=320x180:rate=24:duration=14"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=14"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .args(codecs)
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_browser_ready_release_plays_as_it_is_while_it_downloads() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        encode(
            &app,
            "seed/Clip.2020/Clip.2020.mp4",
            &["-c:a", "aac", "-movflags", "+faststart"],
        );
        std::fs::write(app.path("seed/Clip.2020/readme.txt"), "not a video").unwrap();
        let seeder = app.seed("seed/Clip.2020").await;

        let stream = ready(&app, &admin, &seeder.link).await;
        assert_eq!(stream["state"], "ready", "{stream}");
        assert_eq!(stream["file_name"], "Clip.2020.mp4");
        assert_eq!(
            (&stream["movie"], &stream["year"]),
            (&json!("Clip"), &json!(2020))
        );
        let playback = &stream["playback"];
        assert_eq!(playback["kind"], "direct_play", "{playback}");
        let url = playback["url"].as_str().unwrap();

        let source = std::fs::read(app.path("seed/Clip.2020/Clip.2020.mp4")).unwrap();
        let range = app
            .send(
                Request::get(url)
                    .header("cookie", &admin)
                    .header("range", "bytes=100-4195")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(range.status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            range.headers["content-range"],
            format!("bytes 100-4195/{}", source.len()).as_str()
        );
        assert_eq!(range.body, source[100..4196]);
        let whole = app.get(url, &admin).await;
        assert_eq!(whole.status, StatusCode::OK);
        assert_eq!(whole.body, source);

        let id = stream["id"].as_str().unwrap();
        let stopped = app
            .call(
                "DELETE",
                &format!("/api/admin/streams/{id}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(stopped.status, StatusCode::NO_CONTENT);
        let gone = app.get(&format!("/api/admin/streams/{id}"), &admin).await;
        assert_eq!(gone.status, StatusCode::NOT_FOUND);
        assert!(!app.path(&format!("data/downloads/streams/{id}")).exists());
    }

    /// MKV with AC-3 sound plays nowhere as it is: the stream converts, and
    /// its segments land where the playlist says.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_release_the_device_cant_play_converts_while_it_downloads() {
        let app = TestApp::with_downloads().await;
        let admin = app.setup_admin().await;
        encode(&app, "seed/Clip.2020.1080p.mkv", &["-c:a", "ac3"]);
        let seeder = app.seed("seed/Clip.2020.1080p.mkv").await;

        let stream = ready(&app, &admin, &seeder.link).await;
        assert_eq!(stream["state"], "ready", "{stream}");
        assert_eq!(
            stream["media"]["duration_ms"].as_u64().map(|d| d / 1000),
            Some(14)
        );
        let playback = &stream["playback"];
        assert_eq!(playback["kind"], "hls", "{playback}");
        assert_eq!(playback["converts_video"], true);
        let url = playback["url"].as_str().unwrap();
        assert!(url.contains("/hls/h264-180.a1-aac1/"), "{url}");
        let base = url.trim_end_matches("main.m3u8");

        let playlist = app.get(&format!("{url}?session=s1"), &admin).await;
        assert_eq!(playlist.status, StatusCode::OK);
        let text = String::from_utf8(playlist.body).unwrap();
        assert_eq!(text.matches("#EXTINF:").count(), 3, "{text}");
        let init = app.get(&format!("{base}init.mp4?session=s1"), &admin).await;
        assert_eq!(init.status, StatusCode::OK);
        for (n, at) in [(0, 0.0), (2, 12.0)] {
            let segment = app.get(&format!("{base}{n}.m4s?session=s1"), &admin).await;
            assert_eq!(segment.status, StatusCode::OK, "segment {n}");
            let start = first_video_time(&app, &init.body, &segment.body);
            assert!((start - at).abs() < 0.1, "segment {n} starts at {start}");
        }
        let copy = app.get(&url.replace("h264-180", "copy"), &admin).await;
        assert_eq!(
            copy.status,
            StatusCode::NOT_FOUND,
            "copying needs the whole file"
        );

        let activity = app.get("/api/admin/activity", &admin).await.json();
        assert_eq!(activity["streams"][0]["title"], "Clip.2020.1080p.mkv");
    }

    fn first_video_time(app: &TestApp, init: &[u8], segment: &[u8]) -> f64 {
        let file = app.path("probe.mp4");
        std::fs::write(&file, [init, segment].concat()).unwrap();
        let out = Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "v:0"])
            .args(["-show_entries", "packet=pts_time", "-of", "csv=p=0"])
            .arg(&file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<f64>().ok())
            .fold(f64::INFINITY, f64::min)
    }
}
