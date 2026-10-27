//! Streams for devices that can't play a file as-is: HLS with fMP4
//! segments, remuxed and converted on the fly by [`crate::transcode`]. The
//! item's `playback.url` names the plan; the player adds a `session` id of
//! its own, one per playback, so its seeks restart only its own run.
//!
//! | method | path                                       | access |
//! |--------|--------------------------------------------|--------|
//! | GET    | /api/files/{id}/hls/{plan}/main.m3u8       | member |
//! | GET    | /api/files/{id}/hls/{plan}/init.mp4        | member |
//! | GET    | /api/files/{id}/hls/{plan}/{segment}       | member |

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use nuvora_library::FileId;
use nuvora_media::StreamPlan;
use serde::Deserialize;
use utoipa::IntoParams;

use super::{ApiError, AppState, ErrorBody};
use crate::auth::SignedIn;
use crate::transcode::{Session, SessionKey, TranscodeError};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HlsQuery {
    /// The player's id for this playback: 1 to 64 letters, digits, `-` or `_`.
    /// Defaults to `default`.
    pub session: Option<String>,
    /// Playlist only: where playback starts, in seconds, so the first
    /// segments made are the ones the player asks for first.
    pub start: Option<f64>,
    /// Init segment only: the segment a first run starts at (written into
    /// the playlist).
    pub from: Option<usize>,
}

/// The media playlist for the whole file, VOD, segment URIs relative.
#[utoipa::path(
    get,
    path = "/api/files/{id}/hls/{plan}/main.m3u8",
    params(("id" = i64, Path), ("plan" = String, Path), HlsQuery),
    responses(
        (status = 200, content_type = "application/vnd.apple.mpegurl", body = String),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_playlist(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan)): Path<(i64, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let (session, name) = open(&state, &me, id, &plan, query.session).await?;
    let start = query.start.filter(|s| s.is_finite() && *s > 0.0);
    let body = session.playlist(&format!("session={name}"), start);
    let headers = [
        (CONTENT_TYPE, "application/vnd.apple.mpegurl"),
        (CACHE_CONTROL, "private, no-cache"),
    ];
    Ok((headers, body).into_response())
}

/// The fMP4 init segment.
#[utoipa::path(
    get,
    path = "/api/files/{id}/hls/{plan}/init.mp4",
    params(("id" = i64, Path), ("plan" = String, Path), HlsQuery),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_init(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan)): Path<(i64, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let (session, _) = open(&state, &me, id, &plan, query.session).await?;
    let path = session
        .init(query.from.unwrap_or(0))
        .await
        .map_err(api_error)?;
    serve(&path).await
}

/// Media segment `{n}.m4s`, made on demand: a request past what the current
/// run has made restarts it there.
#[utoipa::path(
    get,
    path = "/api/files/{id}/hls/{plan}/{segment}",
    params(("id" = i64, Path), ("plan" = String, Path), ("segment" = String, Path, description = "`{n}.m4s`"), HlsQuery),
    responses((status = 200, content_type = "video/mp4", body = Vec<u8>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn get_segment(
    State(state): State<AppState>,
    me: SignedIn,
    Path((id, plan, segment)): Path<(i64, String, String)>,
    Query(query): Query<HlsQuery>,
) -> Result<Response, ApiError> {
    let index: usize = segment
        .strip_suffix(".m4s")
        .and_then(|n| n.parse().ok())
        .ok_or(ApiError::not_found())?;
    let (session, _) = open(&state, &me, id, &plan, query.session).await?;
    let path = session.segment(index).await.map_err(api_error)?;
    serve(&path).await
}

/// The session for this user, file, plan and player id. 404 for a plan that
/// doesn't parse or doesn't fit the file (an audio stream it lacks).
async fn open(
    state: &AppState,
    me: &SignedIn,
    id: i64,
    plan: &str,
    session: Option<String>,
) -> Result<(Arc<Session>, String), ApiError> {
    let name = session.unwrap_or_else(|| "default".to_owned());
    let name_ok = (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !name_ok {
        return Err(ApiError::validation(
            "session",
            "Use 1 to 64 letters, digits, - or _",
        ));
    }
    let plan = StreamPlan::parse(plan).ok_or(ApiError::not_found())?;
    let source = state
        .library
        .stream_source(FileId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::not_found())?;
    let fits = !source.media.video.is_empty()
        && plan
            .audio
            .is_none_or(|a| source.media.audio.iter().any(|s| s.index == a.stream));
    if !fits {
        return Err(ApiError::not_found());
    }
    let key = SessionKey {
        user: me.user.id.raw(),
        session: name.clone(),
        file: id,
        plan,
    };
    let session = state
        .transcoder
        .session(key, source)
        .await
        .map_err(api_error)?;
    Ok((session, name))
}

fn api_error(e: TranscodeError) -> ApiError {
    match e {
        TranscodeError::NotFound | TranscodeError::NoVideo => ApiError::not_found(),
        TranscodeError::Busy(most) => ApiError::busy(format!(
            "The server is already converting as many videos as it's set to ({most}). Try again when one ends."
        )),
        other => {
            tracing::warn!(error = %other, "transcode failed");
            ApiError::internal(other)
        }
    }
}

async fn serve(path: &std::path::Path) -> Result<Response, ApiError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::not_found())?;
    let headers = [
        (CONTENT_TYPE, "video/mp4"),
        (CACHE_CONTROL, "private, max-age=600"),
    ];
    Ok((headers, bytes).into_response())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use axum::http::StatusCode;

    use super::super::test_app::TestApp;

    /// Four seconds of H.264 with a keyframe every second and AC-3 audio, in
    /// MKV: the baseline device (MP4, AAC) needs it remuxed and its audio
    /// converted (mono stays mono), but takes the video as it is.
    async fn library() -> (TestApp, String, i64) {
        library_encoded(&["-preset", "ultrafast"]).await
    }

    /// A four-second movie whose H.264 is encoded with `x264`.
    async fn library_encoded(x264: &[&str]) -> (TestApp, String, i64) {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let dir = app.path("movies/Conv (2020)");
        std::fs::create_dir_all(&dir).unwrap();
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=320x180:rate=24:duration=4"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=4"])
            .args(["-c:v", "libx264"])
            .args(x264)
            .args(["-pix_fmt", "yuv420p"])
            .args(["-g", "24", "-keyint_min", "24", "-sc_threshold", "0"])
            .args(["-c:a", "ac3"])
            .arg(dir.join("Conv (2020).mkv"))
            .status()
            .unwrap();
        assert!(status.success());
        app.scan().await;
        let movies = app.get("/api/rows/all_movies", &admin).await.json();
        let id = movies["items"][0]["id"].as_i64().unwrap();
        (app, admin, id)
    }

    /// The first video packet's time in `init` + `segment`, as ffprobe reads it.
    fn first_video_time(app: &TestApp, init: &[u8], segment: &[u8]) -> f64 {
        let file = app.path("probe.mp4");
        std::fs::write(&file, [init, segment].concat()).unwrap();
        let out = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "packet=pts_time",
            ])
            .args(["-of", "csv=p=0"])
            .arg(&file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<f64>().ok())
            .fold(f64::INFINITY, f64::min)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_the_device_cant_play_streams_as_hls_from_any_segment() {
        let (app, admin, id) = library().await;
        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let playback = &detail["versions"][0]["playback"];
        assert_eq!(playback["kind"], "hls", "{playback}");
        assert_eq!(playback["converts_video"], false);
        assert_eq!(playback["converts_audio"], true);
        let url = playback["url"].as_str().unwrap();
        assert!(url.ends_with("/copy.a1-aac1/main.m3u8"), "{url}");
        let base = url.trim_end_matches("main.m3u8");

        let playlist = app
            .get(&format!("{url}?session=t1&start=2.5"), &admin)
            .await;
        assert_eq!(playlist.status, StatusCode::OK);
        let text = String::from_utf8(playlist.body).unwrap();
        assert_eq!(text.matches("#EXTINF:").count(), 4, "{text}");
        assert!(
            text.contains("#EXT-X-MAP:URI=\"init.mp4?session=t1&from=2\""),
            "{text}"
        );
        assert!(
            text.contains("\n3.m4s?session=t1\n#EXT-X-ENDLIST"),
            "{text}"
        );

        // A seek first: the run starts at segment 2, and its init serves every segment.
        let two = app.get(&format!("{base}2.m4s?session=t1"), &admin).await;
        assert_eq!(two.status, StatusCode::OK);
        let init = app
            .get(&format!("{base}init.mp4?session=t1&from=2"), &admin)
            .await;
        assert_eq!(init.status, StatusCode::OK);
        assert_eq!(&init.body[4..8], b"ftyp");
        let at = first_video_time(&app, &init.body, &two.body);
        assert!((at - 2.0).abs() < 0.01, "segment 2 starts at {at}");
        // Back to the start: a new run, timestamps still on the same timeline.
        let zero = app.get(&format!("{base}0.m4s?session=t1"), &admin).await;
        assert_eq!(zero.status, StatusCode::OK);
        let at = first_video_time(&app, &init.body, &zero.body);
        assert!(at.abs() < 0.01, "segment 0 starts at {at}");

        for missing in [
            format!("{base}4.m4s?session=t1"),
            format!("{base}x.m4s?session=t1"),
            url.replace("copy.a1-aac1", "copy.a7"),
            url.replace("copy.a1-aac1", "nonsense"),
        ] {
            assert_eq!(
                app.get(&missing, &admin).await.status,
                StatusCode::NOT_FOUND,
                "{missing}"
            );
        }
        let bad = app.get(&format!("{url}?session=no%20spaces"), &admin).await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_converted_video_is_cut_every_six_seconds() {
        let (app, admin, id) = library().await;
        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let file = detail["versions"][0]["file_id"].as_i64().unwrap();
        let base = format!("/api/files/{file}/hls/h264-180.a1-aac2/");
        let playlist = app
            .get(&format!("{base}main.m3u8?session=t2"), &admin)
            .await;
        let text = String::from_utf8(playlist.body).unwrap();
        assert_eq!(text.matches("#EXTINF:").count(), 1, "{text}");
        let zero = app.get(&format!("{base}0.m4s?session=t2"), &admin).await;
        assert_eq!(zero.status, StatusCode::OK);
        assert!(zero.body.windows(4).any(|w| w == b"moof"));
        // ExoPlayer refuses a fragment that starts before zero; the AAC
        // encoder's lead-in must not put the first audio there.
        let starts = fragment_starts(&zero.body);
        assert_eq!(starts.len(), 2, "a video and an audio fragment");
        assert!(starts.iter().all(|t| *t >= 0), "{starts:?}");
    }

    /// XviD in AVI with B-frames, the way old DVD and BD rips come: its
    /// packets carry a decode time but no presentation time, and seeking it
    /// to 0 lands past the first keyframe. It converts, from the start and
    /// from later on.
    #[tokio::test(flavor = "multi_thread")]
    async fn xvid_in_avi_without_timestamps_converts() {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let dir = app.path("movies/Old Rip (1999)");
        std::fs::create_dir_all(&dir).unwrap();
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=320x180:rate=24:duration=14"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=14"])
            .args(["-c:v", "mpeg4", "-vtag", "XVID", "-bf", "2", "-g", "48"])
            .args(["-c:a", "libmp3lame", "-shortest"])
            .arg(dir.join("Old Rip (1999).avi"))
            .status()
            .unwrap();
        assert!(status.success());
        app.scan().await;
        let movies = app.get("/api/rows/all_movies", &admin).await.json();
        let id = movies["items"][0]["id"].as_i64().unwrap();
        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let playback = &detail["versions"][0]["playback"];
        assert_eq!(playback["kind"], "hls", "{playback}");
        assert_eq!(playback["converts_video"], true);
        let url = playback["url"].as_str().unwrap();
        let base = url.trim_end_matches("main.m3u8");

        let playlist = app.get(&format!("{url}?session=t9"), &admin).await;
        assert_eq!(playlist.status, StatusCode::OK);
        let text = String::from_utf8(playlist.body).unwrap();
        assert_eq!(text.matches("#EXTINF:").count(), 3, "{text}");
        let init = app.get(&format!("{base}init.mp4?session=t9"), &admin).await;
        assert_eq!(init.status, StatusCode::OK);
        let zero = app.get(&format!("{base}0.m4s?session=t9"), &admin).await;
        assert_eq!(
            zero.status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&zero.body)
        );
        let at = first_video_time(&app, &init.body, &zero.body);
        assert!(at.abs() < 0.1, "segment 0 starts at {at}");
        let two = app.get(&format!("{base}2.m4s?session=t9"), &admin).await;
        assert_eq!(two.status, StatusCode::OK);
        let at = first_video_time(&app, &init.body, &two.body);
        assert!((at - 12.0).abs() < 0.1, "segment 2 starts at {at}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copied_video_with_b_frames_never_starts_before_zero() {
        // B-frames decode before they show, so the first decode time of a
        // copied stream is below zero; ExoPlayer refuses such a fragment.
        let (app, admin, id) = library_encoded(&["-preset", "medium", "-bf", "3"]).await;
        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let file = detail["versions"][0]["file_id"].as_i64().unwrap();
        let base = format!("/api/files/{file}/hls/copy.a1-aac2/");
        let playlist = app
            .get(&format!("{base}main.m3u8?session=t3"), &admin)
            .await;
        assert_eq!(playlist.status, StatusCode::OK);
        let zero = app.get(&format!("{base}0.m4s?session=t3"), &admin).await;
        assert_eq!(zero.status, StatusCode::OK);
        let starts = fragment_starts(&zero.body);
        assert_eq!(starts.len(), 2, "a video and an audio fragment");
        assert!(starts.iter().all(|t| *t >= 0), "{starts:?}");
    }

    #[cfg(target_os = "macos")]
    #[tokio::test(flavor = "multi_thread")]
    async fn videotoolbox_converts_into_the_same_six_second_segments() {
        let app = TestApp::with_encoder(crate::config::HardwareEncoding::Videotoolbox).await;
        let admin = app.setup_admin().await;
        let dir = app.path("movies/Long (2020)");
        std::fs::create_dir_all(&dir).unwrap();
        let movie = dir.join("Long (2020).mkv");
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=1280x720:rate=24:duration=20"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=20"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-g",
                "24",
            ])
            .args(["-c:a", "ac3"])
            .arg(&movie)
            .status()
            .unwrap();
        assert!(status.success());
        app.scan().await;
        let movies = app.get("/api/rows/all_movies", &admin).await.json();
        let id = movies["items"][0]["id"].as_i64().unwrap();
        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let file = detail["versions"][0]["file_id"].as_i64().unwrap();
        let base = format!("/api/files/{file}/hls/h264-720.a1-aac2/");

        let playlist = app
            .get(&format!("{base}main.m3u8?session=vt"), &admin)
            .await;
        let text = String::from_utf8(playlist.body).unwrap();
        let lengths: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("#EXTINF:"))
            .collect();
        // The last runs to the end of the source, whose audio makes it 20.006 s.
        assert_eq!(
            lengths[..3],
            ["6.000000,", "6.000000,", "6.000000,"],
            "{text}"
        );
        assert!(
            lengths.len() == 4 && lengths[3].starts_with("2.00"),
            "{text}"
        );

        let mut stream = app
            .get(&format!("{base}init.mp4?session=vt"), &admin)
            .await
            .body;
        for n in 0..4 {
            let segment = app.get(&format!("{base}{n}.m4s?session=vt"), &admin).await;
            assert_eq!(segment.status, StatusCode::OK, "segment {n}");
            stream.extend(segment.body);
        }
        let joined = app.path("joined.mp4");
        std::fs::write(&joined, stream).unwrap();
        let tools = nuvora_media::MediaTools::locate(None, None).await.unwrap();
        let keyframes = nuvora_media::keyframes(&tools, &joined, 0).await.unwrap();
        assert_eq!(
            keyframes,
            [0.0, 6.0, 12.0, 18.0],
            "a keyframe where each segment starts, none elsewhere"
        );

        // A jump into the middle starts a run there, seeking on the chip too.
        let init = app
            .get(&format!("{base}init.mp4?session=vt-jump"), &admin)
            .await
            .body;
        let last = app
            .get(&format!("{base}3.m4s?session=vt-jump"), &admin)
            .await;
        assert_eq!(last.status, StatusCode::OK);
        let jumped = app.path("jumped.mp4");
        std::fs::write(&jumped, [init, last.body].concat()).unwrap();
        assert_eq!(
            nuvora_media::keyframes(&tools, &jumped, 0).await.unwrap(),
            [18.0]
        );
    }

    #[tokio::test]
    async fn settings_shape_new_streams_and_activity_shows_their_progress() {
        let (app, admin, id) = library().await;
        let mut tuning = app.get("/api/admin/tuning", &admin).await.json()["settings"].clone();
        tuning["segment_seconds"] = serde_json::json!(2);
        tuning["stereo_kbps"] = serde_json::json!(128);
        let saved = app
            .call("PUT", "/api/admin/tuning", Some(&admin), Some(tuning))
            .await;
        assert_eq!(saved.status, StatusCode::OK);

        let detail = app.get(&format!("/api/items/{id}"), &admin).await.json();
        let file = detail["versions"][0]["file_id"].as_i64().unwrap();
        let base = format!("/api/files/{file}/hls/h264-180.a1-aac2/");
        let playlist = app
            .get(&format!("{base}main.m3u8?session=a1"), &admin)
            .await;
        let text = String::from_utf8(playlist.body).unwrap();
        assert_eq!(text.matches("#EXTINF:2.000000,").count(), 2, "{text}");
        let zero = app.get(&format!("{base}0.m4s?session=a1"), &admin).await;
        assert_eq!(zero.status, StatusCode::OK);

        // Four seconds convert in a blink; wait for the run's last report.
        let mut stream = serde_json::Value::Null;
        for _ in 0..100 {
            stream = app.get("/api/admin/activity", &admin).await.json()["streams"][0].clone();
            if stream["state"] == "finished" && stream["speed"].is_number() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(stream["state"], "finished", "{stream}");
        assert_eq!(stream["title"], "Conv");
        assert_eq!(stream["user"], "Admin");
        assert_eq!(stream["source_video"]["codec"], "H.264");
        assert_eq!(stream["source_audio"]["codec"], "Dolby Digital");
        assert_eq!(stream["video_output"]["max_height"], 180);
        assert_eq!(stream["video_output"]["encoder"], "x264 (processor)");
        assert_eq!(
            stream["audio_output"],
            serde_json::json!({"kind": "aac", "channels": 2, "kbps": 128})
        );
        // Two of 2 s. Some FFmpeg builds also cut a sliver for the few
        // milliseconds of sound past the last frame (Homebrew's does;
        // Debian's, in the Docker image and CI, doesn't).
        let segments = stream["segments"].as_u64().unwrap();
        assert!((2..=3).contains(&segments), "{stream}");
        let span = serde_json::json!({"start": 0.0, "end": 4.0});
        assert_eq!(stream["made"], serde_json::json!([span]));
        assert_eq!(stream["run"], span);
        assert!(stream["frames"].as_u64().unwrap() >= 90, "{stream}");
        assert!(stream["disk_bytes"].as_u64().unwrap() > 0);
    }

    /// Every `tfdt` (fragment start, in its track's timescale) in a segment.
    fn fragment_starts(segment: &[u8]) -> Vec<i64> {
        let mut out = Vec::new();
        let mut at = 0;
        while let Some(found) = segment[at..].windows(4).position(|w| w == b"tfdt") {
            let box_start = at + found + 4;
            let value = if segment[box_start] == 1 {
                i64::from_be_bytes(segment[box_start + 4..box_start + 12].try_into().unwrap())
            } else {
                i64::from(i32::from_be_bytes(
                    segment[box_start + 4..box_start + 8].try_into().unwrap(),
                ))
            };
            out.push(value);
            at = box_start;
        }
        out
    }
}
