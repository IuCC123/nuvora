//! ffprobe: run it, parse its JSON into [`MediaInfo`]. The JSON types live in the
//! private `raw` module; nothing outside this file sees ffprobe's field names.

use std::path::Path;

use crate::MediaTools;

/// Bump when parsing changes in a way stored rows should pick up (new field,
/// better HDR detection). The scanner re-probes every file whose stored
/// `probe_version` differs, once.
pub const PROBE_VERSION: u32 = 1;

/// What is inside one media file.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub container: Container,
    pub duration_ms: Option<u64>,
    /// Overall bitrate in bits/s.
    pub bitrate: Option<u64>,
    /// In ffprobe stream order. Index 0 is the primary picture (attached cover
    /// art streams are dropped during parsing).
    pub video: Vec<VideoStream>,
    pub audio: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
}

impl MediaInfo {
    /// The audio track a `<video>` element (or ExoPlayer) plays without being
    /// told otherwise: the `default` disposition, else the first.
    pub fn default_audio(&self) -> Option<&AudioStream> {
        self.audio
            .iter()
            .find(|a| a.is_default)
            .or(self.audio.first())
    }
}

/// Container, normalized. ffprobe reports `matroska,webm` for both; parsing
/// tells them apart by codecs (WebM allows only VP8/VP9/AV1 + Vorbis/Opus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Container {
    Mp4,
    Mov,
    Matroska,
    WebM,
    MpegTs,
    Avi,
    Other,
}

/// Video codecs Nuvora reasons about. `Other` keeps the ffprobe name so the UI
/// can still show it and the device profile can never accidentally match it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Vp9,
    Vp8,
    Mpeg2,
    Mpeg4,
    Vc1,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    Aac,
    Mp3,
    Opus,
    Vorbis,
    Flac,
    Ac3,
    Eac3,
    TrueHd,
    Dts,
    Pcm,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubtitleCodec {
    Srt,
    Ass,
    WebVtt,
    MovText,
    Pgs,
    VobSub,
    Other(String),
}

/// Dynamic range of a video stream, from colour transfer + side data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrFormat {
    Sdr,
    Hdr10,
    Hlg,
    /// Dolby Vision. `profile` from the DOVI configuration record; profile 8.1
    /// has an HDR10 base layer, which Step 3 can exploit.
    DolbyVision {
        profile: u8,
        compatible_with_hdr10: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColorInfo {
    pub primaries: Option<String>,
    pub transfer: Option<String>,
    pub space: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoStream {
    pub index: u32,
    pub codec: VideoCodec,
    pub profile: Option<String>,
    pub level: Option<i32>,
    pub width: u32,
    pub height: u32,
    pub bit_depth: Option<u8>,
    pub frame_rate: Option<f64>,
    pub hdr: HdrFormat,
    pub color: ColorInfo,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    pub index: u32,
    pub codec: AudioCodec,
    pub profile: Option<String>,
    pub channels: u8,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<u32>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleStream {
    pub index: u32,
    pub codec: SubtitleCodec,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("ffprobe timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("ffprobe failed: {stderr}")]
    Failed { stderr: String },
    #[error("ffprobe output was not understood: {0}")]
    Unparseable(String),
    #[error("no video stream")]
    NoVideo,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Run `ffprobe -v error -print_format json -show_format -show_streams` on
/// `path` with the tools' timeout (the child is killed on timeout) and parse
/// the result. Concurrency is the caller's job (the scanner holds a semaphore).
pub async fn probe(tools: &MediaTools, path: &Path) -> Result<MediaInfo, ProbeError> {
    let _ = (tools, path);
    todo!("tokio::process::Command, kill_on_drop(true), tokio::time::timeout, then parse(&stdout)")
}

/// Pure parse step, separate so tests can feed captured ffprobe JSON.
pub(crate) fn parse(json: &[u8]) -> Result<MediaInfo, ProbeError> {
    let raw: raw::Output =
        serde_json::from_slice(json).map_err(|e| ProbeError::Unparseable(e.to_string()))?;
    let _ = raw;
    // TODO: drop attached_pic streams; map codec_name -> enums; bits_per_raw_sample or
    // pix_fmt -> bit_depth; color_transfer smpte2084 -> Hdr10, arib-std-b67 -> Hlg;
    // side_data_list "DOVI configuration record" -> DolbyVision{profile, bl_compat_id in {1,6}};
    // r_frame_rate "24000/1001" -> 23.976; tags.language/title; disposition.default/forced;
    // format_name + codecs -> Container (matroska vs webm).
    todo!()
}

/// Stable string forms used by the database. One impl per enum, here, next to
/// the enum; the database never invents its own names.
macro_rules! str_enum {
    ($t:ty { $($v:ident => $s:literal),* $(,)? }) => {
        impl $t {
            pub fn as_str(&self) -> &str {
                match self { $(Self::$v => $s,)* Self::Other(s) => s }
            }
            pub fn from_name(s: &str) -> Self {
                match s { $($s => Self::$v,)* other => Self::Other(other.to_owned()) }
            }
        }
    };
}
str_enum!(VideoCodec { H264 => "h264", Hevc => "hevc", Av1 => "av1", Vp9 => "vp9", Vp8 => "vp8", Mpeg2 => "mpeg2video", Mpeg4 => "mpeg4", Vc1 => "vc1" });
str_enum!(AudioCodec { Aac => "aac", Mp3 => "mp3", Opus => "opus", Vorbis => "vorbis", Flac => "flac", Ac3 => "ac3", Eac3 => "eac3", TrueHd => "truehd", Dts => "dts", Pcm => "pcm" });
str_enum!(SubtitleCodec { Srt => "subrip", Ass => "ass", WebVtt => "webvtt", MovText => "mov_text", Pgs => "hdmv_pgs_subtitle", VobSub => "dvd_subtitle" });

mod raw {
    //! ffprobe's JSON, exactly as much as we read. Private on purpose.
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Output {
        #[serde(default)]
        pub streams: Vec<Stream>,
        pub format: Option<Format>,
    }

    #[derive(Deserialize)]
    pub struct Format {
        pub format_name: String,
        pub duration: Option<String>,
        pub bit_rate: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct Stream {
        pub index: u32,
        pub codec_type: Option<String>,
        pub codec_name: Option<String>,
        pub profile: Option<String>,
        pub level: Option<i32>,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub pix_fmt: Option<String>,
        pub bits_per_raw_sample: Option<String>,
        pub r_frame_rate: Option<String>,
        pub color_primaries: Option<String>,
        pub color_transfer: Option<String>,
        pub color_space: Option<String>,
        pub channels: Option<u8>,
        pub channel_layout: Option<String>,
        pub sample_rate: Option<String>,
        #[serde(default)]
        pub disposition: std::collections::HashMap<String, u8>,
        #[serde(default)]
        pub tags: std::collections::HashMap<String, String>,
        #[serde(default)]
        pub side_data_list: Vec<serde_json::Value>,
    }
}

#[cfg(test)]
mod tests {
    //! Fixtures are generated once per test run with ffmpeg (`testsrc2` + `sine`,
    //! 2 s each) into a temp dir, then probed for real:
    //!
    //! | file            | video                    | audio              | expect                 |
    //! |-----------------|--------------------------|--------------------|------------------------|
    //! | h264_aac.mp4    | libx264 high 8-bit 720p  | aac stereo         | Mp4, H264, Sdr         |
    //! | hevc10_eac3.mkv | libx265 main10 + PQ tags | eac3 5.1           | Matroska, Hevc, Hdr10  |
    //! | vp9_opus.webm   | libvpx-vp9               | libopus            | WebM, Vp9              |
    //! | av1_flac.mkv    | libsvtav1                | flac               | Matroska, Av1          |
    //! | mpeg4_ac3.avi   | mpeg4                    | ac3                | Avi, Mpeg4             |
    //! | subs.mkv        | libx264 + srt + forced   | aac + aac(default) | default_audio = idx 2  |
    //! | cover.mp4       | h264 + attached jpeg     | aac                | 1 video stream, not 2  |
    //!
    //! HDR10 tagging: `-x265-params colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc`.
}

impl Container {
    pub fn as_str(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Matroska => "mkv",
            Container::WebM => "webm",
            Container::MpegTs => "ts",
            Container::Avi => "avi",
            Container::Other => "other",
        }
    }
    pub fn from_name(s: &str) -> Container {
        let _ = s;
        todo!("inverse of as_str; unknown => Other")
    }
    /// MIME type for the direct-play response and the `<video>` element.
    pub fn mime_type(self) -> &'static str {
        match self {
            Container::Mp4 => "video/mp4",
            Container::Mov => "video/quicktime",
            Container::Matroska => "video/x-matroska",
            Container::WebM => "video/webm",
            Container::MpegTs => "video/mp2t",
            Container::Avi => "video/x-msvideo",
            Container::Other => "application/octet-stream",
        }
    }
}
