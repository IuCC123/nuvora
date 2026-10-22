//! Subtitle tracks and the fonts styled tracks need.
//!
//! | method | path                                          | access |
//! |--------|-----------------------------------------------|--------|
//! | GET    | /api/files/{id}/subtitles/{stream_index}      | member |
//! | GET    | /api/files/{id}/fonts                         | member |
//! | GET    | /api/files/{id}/fonts/{index}                 | member |

use std::future::Future;
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use nuvora_library::{FileId, SubtitleSource};
use nuvora_media::{SubtitleCodec, SubtitleStream, SubtitleText};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, AppState, ErrorBody};

/// ffmpeg extractions running at once, across every request.
static EXTRACTIONS: Semaphore = Semaphore::const_new(2);

const IMMUTABLE: &str = "private, max-age=31536000, immutable";

/// One subtitle track of a version. Fetch a renderable one from
/// `/api/files/{file_id}/subtitles/{index}`.
#[derive(Serialize, ToSchema)]
pub struct SubtitleTrack {
    /// The stream index in the file: `{stream_index}` in the subtitle URL.
    pub index: u32,
    /// Display name: `"ASS"`, `"SRT"`, `"PGS"`.
    pub codec: String,
    pub format: SubtitleFormat,
    /// The server can deliver the track as text. False for `image` and
    /// `other`, which need conversion.
    pub renderable: bool,
    /// ISO 639-2 as probed, lowercased: `"spa"`, `"eng"`.
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
    /// Subtitles for the deaf and hard of hearing (SDH).
    pub is_hearing_impaired: bool,
}

/// What the track is, for choosing how to draw it. Clients treat an unknown
/// format like `other`.
#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleFormat {
    /// SubRip. Plain text: fetch as `vtt`.
    Srt,
    /// ASS/SSA, styled and positioned. Draw `format=ass` with libass, using
    /// the file's fonts; `format=vtt` is the plain-text fallback.
    Ass,
    /// WebVTT. Plain text.
    Vtt,
    /// MP4 timed text. Plain text: fetch as `vtt`.
    MovText,
    /// Bitmaps (PGS, VobSub, DVB). Not renderable: needs conversion.
    Image,
    /// A codec the server does not convert. Not renderable.
    Other,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SubtitleQuery {
    /// Default `vtt`.
    #[param(inline)]
    pub format: Option<SubtitleOutput>,
}

/// The text format a track is delivered in.
#[derive(Deserialize, ToSchema, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleOutput {
    /// `text/vtt`. Every renderable track.
    #[default]
    Vtt,
    /// `text/x-ssa`, the original script for `ass` tracks, a converted one
    /// for the others.
    Ass,
}

/// A font attached to the file (Matroska attachments). Styled tracks name
/// fonts that only these provide.
#[derive(Serialize, ToSchema)]
pub struct FontAttachment {
    /// The attachment's stream index: `{index}` in `url`.
    pub index: u32,
    pub file_name: String,
    /// `"font/ttf"`, `"font/otf"`, `"font/collection"`, `"font/woff"`, `"font/woff2"`.
    pub mime_type: String,
    pub size_bytes: Option<u64>,
    /// `/api/files/{file_id}/fonts/{index}`. Immutable.
    pub url: String,
}

/// The track as text, extracted with ffmpeg on the first request and read
/// from the server's cache after that. Answers with
/// `Cache-Control: private, max-age=31536000, immutable`. 404 when the file
/// has no subtitle stream at `stream_index`; 400 when the track is not
/// renderable.
#[utoipa::path(
    get,
    path = "/api/files/{id}/subtitles/{stream_index}",
    params(("id" = i64, Path), ("stream_index" = u32, Path), SubtitleQuery),
    responses(
        (status = 200, content((String = "text/vtt"), (String = "text/x-ssa"))),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_subtitle(
    State(state): State<AppState>,
    Path((id, stream_index)): Path<(i64, u32)>,
    Query(query): Query<SubtitleQuery>,
) -> Result<Response, ApiError> {
    let source = subtitle_source(&state, id).await?;
    let stream = source
        .subtitles
        .iter()
        .find(|s| s.index == stream_index)
        .ok_or(ApiError::not_found())?;
    if !renderable(subtitle_format(&stream.codec)) {
        return Err(ApiError::bad_request(
            "This subtitle track needs conversion, which isn't available yet",
        ));
    }
    let text = match query.format.unwrap_or_default() {
        SubtitleOutput::Vtt => SubtitleText::WebVtt,
        SubtitleOutput::Ass => SubtitleText::Ass,
    };
    let cached = cache_dir(&state, &source).join(format!("{stream_index}.{}", text.extension()));
    let (tools, media) = (&state.server.tools, &source.path);
    let bytes = cached_or(&cached, media, |out| async move {
        nuvora_media::extract_subtitle(tools, media, stream_index, &stream.codec, text, &out)
            .await
            .map_err(ApiError::internal)
    })
    .await?;
    Ok(immutable(text.mime_type(), bytes))
}

/// The file's font attachments, in stream order. Empty for files without any
/// (every MP4). The ETag is the file's fingerprint.
#[utoipa::path(
    get,
    path = "/api/files/{id}/fonts",
    params(("id" = i64, Path)),
    responses(
        (status = 200, body = Vec<FontAttachment>),
        (status = 304),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn list_fonts(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let source = subtitle_source(&state, id).await?;
    let etag = format!("\"{}\"", source.fingerprint);
    if super::if_none_match(&headers, &etag) {
        return Ok(axum::http::StatusCode::NOT_MODIFIED.into_response());
    }
    if !exists(&source.path).await {
        return Err(ApiError::not_found());
    }
    let fonts: Vec<FontAttachment> =
        nuvora_media::font_attachments(&state.server.tools, &source.path)
            .await
            .map_err(ApiError::internal)?
            .into_iter()
            .map(|f| FontAttachment {
                url: format!("/api/files/{id}/fonts/{}", f.index),
                index: f.index,
                file_name: f.file_name,
                mime_type: f.mime_type.to_owned(),
                size_bytes: f.size_bytes,
            })
            .collect();
    Ok(super::json_with_etag(&etag, &fonts))
}

/// One font attachment's bytes, immutable like a subtitle track. 404 when
/// the stream at `index` is not a font.
#[utoipa::path(
    get,
    path = "/api/files/{id}/fonts/{index}",
    params(("id" = i64, Path), ("index" = u32, Path)),
    responses(
        (status = 200, content_type = "font/*", body = Vec<u8>),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn get_font(
    State(state): State<AppState>,
    Path((id, index)): Path<(i64, u32)>,
) -> Result<Response, ApiError> {
    let source = subtitle_source(&state, id).await?;
    let cached = cache_dir(&state, &source).join(format!("{index}.font"));
    let (tools, media) = (&state.server.tools, &source.path);
    let bytes = cached_or(&cached, media, |out| async move {
        let fonts = nuvora_media::font_attachments(tools, media)
            .await
            .map_err(ApiError::internal)?;
        if !fonts.iter().any(|f| f.index == index) {
            return Err(ApiError::not_found());
        }
        nuvora_media::dump_attachment(tools, media, index, &out)
            .await
            .map_err(ApiError::internal)
    })
    .await?;
    let mime = nuvora_media::font_mime_type(&bytes).unwrap_or("application/octet-stream");
    Ok(immutable(mime, bytes))
}

async fn subtitle_source(state: &AppState, id: i64) -> Result<SubtitleSource, ApiError> {
    state
        .library
        .subtitle_source(FileId::from_raw(id))
        .await
        .map_err(ApiError::internal)?
        .ok_or(ApiError::not_found())
}

/// `<data>/subtitles/<fingerprint>/`. A changed file has a new fingerprint,
/// so nothing here is ever stale.
fn cache_dir(state: &AppState, source: &SubtitleSource) -> PathBuf {
    state
        .server
        .data_dir
        .join("subtitles")
        .join(&source.fingerprint)
}

/// The bytes at `cached`, made by `produce` first when missing. `produce`
/// runs holding an [`EXTRACTIONS`] permit and writes a temporary file that is
/// renamed into place, so readers never see a partial file and a failed run
/// leaves nothing behind. 404 when the media file is gone from disk.
async fn cached_or<F, Fut>(cached: &FsPath, media: &FsPath, produce: F) -> Result<Vec<u8>, ApiError>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<(), ApiError>>,
{
    if let Ok(bytes) = tokio::fs::read(cached).await {
        return Ok(bytes);
    }
    let _permit = EXTRACTIONS.acquire().await.map_err(ApiError::internal)?;
    // A request for the same file may have made it while this one waited.
    if let Ok(bytes) = tokio::fs::read(cached).await {
        return Ok(bytes);
    }
    if !exists(media).await {
        return Err(ApiError::not_found());
    }
    let dir = cached.parent().expect("cache files live in a directory");
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(ApiError::internal)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let partial = dir.join(format!(
        ".{}.{}.partial",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let made = produce(partial.clone()).await;
    if let Err(e) = made {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(e);
    }
    tokio::fs::rename(&partial, cached)
        .await
        .map_err(ApiError::internal)?;
    tokio::fs::read(cached).await.map_err(ApiError::internal)
}

async fn exists(path: &FsPath) -> bool {
    tokio::fs::try_exists(path).await.unwrap_or(false)
}

fn immutable(content_type: &'static str, bytes: Vec<u8>) -> Response {
    let headers = [(CONTENT_TYPE, content_type), (CACHE_CONTROL, IMMUTABLE)];
    (headers, bytes).into_response()
}

impl From<SubtitleStream> for SubtitleTrack {
    fn from(s: SubtitleStream) -> Self {
        let format = subtitle_format(&s.codec);
        SubtitleTrack {
            index: s.index,
            codec: codec_name(&s.codec),
            format,
            renderable: renderable(format),
            is_hearing_impaired: s.title.as_deref().is_some_and(names_sdh),
            language: s.language,
            title: s.title,
            is_default: s.is_default,
            is_forced: s.is_forced,
        }
    }
}

fn subtitle_format(codec: &SubtitleCodec) -> SubtitleFormat {
    match codec {
        SubtitleCodec::Srt => SubtitleFormat::Srt,
        SubtitleCodec::Ass => SubtitleFormat::Ass,
        SubtitleCodec::WebVtt => SubtitleFormat::Vtt,
        SubtitleCodec::MovText => SubtitleFormat::MovText,
        SubtitleCodec::Pgs | SubtitleCodec::VobSub => SubtitleFormat::Image,
        SubtitleCodec::Other(name) => match name.as_str() {
            "srt" => SubtitleFormat::Srt,
            "ssa" => SubtitleFormat::Ass,
            "dvb_subtitle" | "xsub" => SubtitleFormat::Image,
            _ => SubtitleFormat::Other,
        },
    }
}

fn renderable(format: SubtitleFormat) -> bool {
    !matches!(format, SubtitleFormat::Image | SubtitleFormat::Other)
}

/// The probe keeps no hearing-impaired disposition, so the title decides:
/// `"English SDH"`, `"English (CC)"`, `"Hearing Impaired"`.
fn names_sdh(title: &str) -> bool {
    let title = title.to_lowercase();
    title.contains("hearing impaired")
        || title.contains("hard of hearing")
        || title
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word == "sdh" || word == "cc")
}

fn codec_name(c: &SubtitleCodec) -> String {
    match c {
        SubtitleCodec::Srt => "SRT".into(),
        SubtitleCodec::Ass => "ASS".into(),
        SubtitleCodec::WebVtt => "WebVTT".into(),
        SubtitleCodec::MovText => "MP4 text".into(),
        SubtitleCodec::Pgs => "PGS".into(),
        SubtitleCodec::VobSub => "VobSub".into(),
        SubtitleCodec::Other(name) => name.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::time::SystemTime;

    use axum::http::StatusCode;
    use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
    use nuvora_media::{SubtitleCodec, SubtitleStream};
    use serde_json::json;

    use super::super::test_app::{Reply, TestApp};
    use super::SubtitleTrack;

    const SRT: &str = "1\n00:00:00,500 --> 00:00:01,200\nHello <i>there</i>\n\n\
                       2\n00:00:01,300 --> 00:00:01,900\nSecond line\n";
    const ASS: &str = "[Script Info]
; nuvora fixture
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Fake Sans,60,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,1,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.50,0:00:01.50,Default,,0,0,0,,{\\pos(960,100)}Hola {\\b1}mundo{\\b0}
";
    const FONT: &[u8] = b"\x00\x01\x00\x00fake font";
    const SRT_AS_VTT: &str = "WEBVTT\n\n00:00.500 --> 00:01.200\nHello <i>there</i>\n\n\
                              00:01.300 --> 00:01.900\nSecond line\n";

    struct Library {
        app: TestApp,
        admin: String,
        /// `Subs (2020).mkv`: video, English SRT (1), Spanish ASS flagged
        /// default (2), `Fake.ttf` attached (3).
        mkv: i64,
        /// `Text (2021).mp4`: video, English mov_text (1).
        mp4: i64,
    }

    async fn subtitled_library() -> Library {
        let app = TestApp::new().await;
        let admin = app.setup_admin().await;
        let inputs = app.path("inputs");
        std::fs::create_dir_all(&inputs).unwrap();
        std::fs::write(inputs.join("en.srt"), SRT).unwrap();
        std::fs::write(inputs.join("es.ass"), ASS).unwrap();
        std::fs::write(inputs.join("Fake.ttf"), FONT).unwrap();
        let movie = |name: &str, ext: &str| {
            let dir = app.path("movies").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            dir.join(format!("{name}.{ext}"))
        };
        ffmpeg(
            &inputs,
            &[
                "-i",
                "en.srt",
                "-i",
                "es.ass",
                "-map",
                "0:v",
                "-map",
                "1",
                "-map",
                "2",
                "-c:s:0",
                "srt",
                "-c:s:1",
                "copy",
                "-metadata:s:s:0",
                "language=eng",
                "-metadata:s:s:1",
                "language=spa",
                "-metadata:s:s:1",
                "title=Full SDH",
                "-disposition:s:0",
                "0",
                "-disposition:s:1",
                "default",
                "-attach",
                "Fake.ttf",
                "-metadata:s:t",
                "mimetype=font/ttf",
            ],
            &movie("Subs (2020)", "mkv"),
        );
        ffmpeg(
            &inputs,
            &[
                "-i",
                "en.srt",
                "-map",
                "0:v",
                "-map",
                "1",
                "-c:s",
                "mov_text",
                "-metadata:s:s:0",
                "language=eng",
            ],
            &movie("Text (2021)", "mp4"),
        );
        let before = SystemTime::now();
        let mut status = app.state.library.watch_scan();
        app.state.library.request_scan();
        status
            .wait_for(|s| {
                matches!(s.state, nuvora_library::ScanState::Idle)
                    && s.last.as_ref().is_some_and(|l| l.started_at >= before)
            })
            .await
            .unwrap();
        let movies = app.get("/api/rows/all_movies", &admin).await.json();
        let file_of = async |title: &str| {
            let item = movies["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["title"] == title)
                .unwrap_or_else(|| panic!("{title} not scanned: {movies}"));
            let detail = app
                .get(&format!("/api/items/{}", item["id"]), &admin)
                .await
                .json();
            detail["versions"][0]["file_id"].as_i64().unwrap()
        };
        let (mkv, mp4) = (file_of("Subs").await, file_of("Text").await);
        Library {
            app,
            admin,
            mkv,
            mp4,
        }
    }

    /// Two seconds of tiny video from `testsrc2`, plus `args`, run in `inputs`.
    fn ffmpeg(inputs: &Path, args: &[&str], out: &Path) {
        let status = Command::new("ffmpeg")
            .current_dir(inputs)
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=160x90:rate=24:duration=2"])
            .args(args)
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(out)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn text(reply: &Reply) -> &str {
        std::str::from_utf8(&reply.body).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn text_tracks_come_out_as_vtt() {
        let lib = subtitled_library().await;
        let get = |uri: String| {
            let lib = &lib;
            async move { lib.app.get(&uri, &lib.admin).await }
        };
        let srt = get(format!("/api/files/{}/subtitles/1", lib.mkv)).await;
        assert_eq!(srt.status, StatusCode::OK);
        assert_eq!(text(&srt), SRT_AS_VTT);
        assert_eq!(srt.headers[CONTENT_TYPE], "text/vtt; charset=utf-8");
        assert_eq!(
            srt.headers[CACHE_CONTROL],
            "private, max-age=31536000, immutable"
        );
        let ass = get(format!("/api/files/{}/subtitles/2?format=vtt", lib.mkv)).await;
        assert_eq!(
            text(&ass),
            "WEBVTT\n\n00:00.500 --> 00:01.500\nHola <b>mundo</b>\n"
        );
        let mov_text = get(format!("/api/files/{}/subtitles/1", lib.mp4)).await;
        assert_eq!(text(&mov_text), SRT_AS_VTT);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ass_tracks_pass_through_untouched() {
        let lib = subtitled_library().await;
        let reply = lib
            .app
            .get(
                &format!("/api/files/{}/subtitles/2?format=ass", lib.mkv),
                &lib.admin,
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.headers[CONTENT_TYPE], "text/x-ssa; charset=utf-8");
        assert_eq!(text(&reply), ASS);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_cached_track_is_read_not_extracted() {
        let lib = subtitled_library().await;
        let uri = format!("/api/files/{}/subtitles/1", lib.mkv);
        assert_eq!(text(&lib.app.get(&uri, &lib.admin).await), SRT_AS_VTT);
        let cache = lib.app.path("data/subtitles");
        let mut dirs = std::fs::read_dir(&cache).unwrap();
        let dir = dirs.next().unwrap().unwrap().path();
        assert!(dirs.next().is_none(), "one fingerprint directory");
        assert_eq!(dir.file_name().unwrap().len(), 32);
        std::fs::write(dir.join("1.vtt"), "WEBVTT\n\nfrom the cache\n").unwrap();
        let again = lib.app.get(&uri, &lib.admin).await;
        assert_eq!(text(&again), "WEBVTT\n\nfrom the cache\n");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(leftovers, ["1.vtt"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fonts_are_listed_and_served() {
        let lib = subtitled_library().await;
        let list = lib
            .app
            .get(&format!("/api/files/{}/fonts", lib.mkv), &lib.admin)
            .await;
        assert_eq!(list.status, StatusCode::OK);
        assert_eq!(
            list.json(),
            json!([{
                "index": 3,
                "file_name": "Fake.ttf",
                "mime_type": "font/ttf",
                "size_bytes": FONT.len(),
                "url": format!("/api/files/{}/fonts/3", lib.mkv),
            }])
        );
        let font = lib
            .app
            .get(&format!("/api/files/{}/fonts/3", lib.mkv), &lib.admin)
            .await;
        assert_eq!(font.status, StatusCode::OK);
        assert_eq!(font.body, FONT);
        assert_eq!(font.headers[CONTENT_TYPE], "font/ttf");
        assert_eq!(
            font.headers[CACHE_CONTROL],
            "private, max-age=31536000, immutable"
        );
        let not_a_font = lib
            .app
            .get(&format!("/api/files/{}/fonts/2", lib.mkv), &lib.admin)
            .await;
        assert_eq!(not_a_font.status, StatusCode::NOT_FOUND);
        let none = lib
            .app
            .get(&format!("/api/files/{}/fonts", lib.mp4), &lib.admin)
            .await;
        assert_eq!(none.json(), json!([]));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_detail_describes_each_track() {
        let lib = subtitled_library().await;
        let movies = lib.app.get("/api/rows/all_movies", &lib.admin).await.json();
        let subs = movies["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["title"] == "Subs")
            .unwrap();
        let detail = lib
            .app
            .get(&format!("/api/items/{}", subs["id"]), &lib.admin)
            .await
            .json();
        assert_eq!(
            detail["versions"][0]["media"]["subtitles"],
            json!([
                {"index": 1, "codec": "SRT", "format": "srt", "renderable": true,
                 "language": "eng", "title": null, "is_default": false,
                 "is_forced": false, "is_hearing_impaired": false},
                {"index": 2, "codec": "ASS", "format": "ass", "renderable": true,
                 "language": "spa", "title": "Full SDH", "is_default": true,
                 "is_forced": false, "is_hearing_impaired": true},
            ])
        );
    }

    #[test]
    fn image_and_unknown_tracks_are_not_renderable() {
        let track = |codec: SubtitleCodec, title: Option<&str>| {
            let track = SubtitleTrack::from(SubtitleStream {
                index: 4,
                codec,
                language: Some("eng".into()),
                title: title.map(str::to_owned),
                is_default: false,
                is_forced: true,
            });
            (track.format, track.renderable, track.is_hearing_impaired)
        };
        use super::SubtitleFormat::*;
        assert_eq!(track(SubtitleCodec::Pgs, None), (Image, false, false));
        assert_eq!(track(SubtitleCodec::VobSub, None), (Image, false, false));
        assert_eq!(
            track(SubtitleCodec::Other("dvb_subtitle".into()), None),
            (Image, false, false)
        );
        assert_eq!(
            track(SubtitleCodec::Other("eia_608".into()), None),
            (Other, false, false)
        );
        assert_eq!(
            track(SubtitleCodec::MovText, Some("English (CC)")),
            (MovText, true, true)
        );
        assert_eq!(
            track(SubtitleCodec::Srt, Some("Accent marks")),
            (Srt, true, false)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_subtitle_streams_are_served() {
        let lib = subtitled_library().await;
        for uri in [
            format!("/api/files/{}/subtitles/0", lib.mkv),
            format!("/api/files/{}/subtitles/3", lib.mkv),
            format!("/api/files/{}/subtitles/9", lib.mkv),
            "/api/files/999/subtitles/1".to_owned(),
            "/api/files/999/fonts".to_owned(),
            "/api/files/999/fonts/3".to_owned(),
        ] {
            let reply = lib.app.get(&uri, &lib.admin).await;
            assert_eq!(reply.status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(reply.json()["code"], "not_found", "{uri}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_session_is_required_and_members_may_fetch() {
        let lib = subtitled_library().await;
        let uris = [
            format!("/api/files/{}/subtitles/2?format=ass", lib.mkv),
            format!("/api/files/{}/fonts", lib.mkv),
            format!("/api/files/{}/fonts/3", lib.mkv),
        ];
        for uri in &uris {
            let reply = lib.app.call("GET", uri, None, None).await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{uri}");
        }
        let created = lib
            .app
            .call(
                "POST",
                "/api/admin/users",
                Some(&lib.admin),
                Some(json!({"username": "bo", "display_name": "Bo", "password": "password bo", "role": "member"})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED);
        let member = lib.app.login("bo", "password bo").await.cookie();
        for uri in &uris {
            assert_eq!(
                lib.app.get(uri, &member).await.status,
                StatusCode::OK,
                "{uri}"
            );
        }
    }
}
