//! Subtitle tracks and the fonts styled tracks need.
//!
//! | method | path                                          | access |
//! |--------|-----------------------------------------------|--------|
//! | GET    | /api/files/{id}/subtitles/{stream_index}      | member |
//! | GET    | /api/files/{id}/fonts                         | member |
//! | GET    | /api/files/{id}/fonts/{index}                 | member |

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use nuvora_media::{SubtitleCodec, SubtitleStream};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::ErrorBody;

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
    Path((_id, _stream_index)): Path<(i64, u32)>,
    Query(query): Query<SubtitleQuery>,
) -> StatusCode {
    let _format = query.format.unwrap_or_default();
    StatusCode::NOT_IMPLEMENTED
}

/// The file's font attachments, in stream order. Empty for files without any
/// (every MP4).
#[utoipa::path(
    get,
    path = "/api/files/{id}/fonts",
    params(("id" = i64, Path)),
    responses((status = 200, body = Vec<FontAttachment>), (status = 404, body = ErrorBody)),
)]
pub(super) async fn list_fonts(Path(_id): Path<i64>) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
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
pub(super) async fn get_font(Path((_id, _index)): Path<(i64, u32)>) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
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
