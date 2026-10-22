//! ffprobe: run it, parse its JSON into [`MediaInfo`]. The JSON types live in the
//! private `raw` module; nothing outside this file sees ffprobe's field names.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

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
    let child = Command::new(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = tokio::time::timeout(tools.probe_timeout, child.wait_with_output())
        .await
        .map_err(|_| ProbeError::Timeout(tools.probe_timeout))??;
    if !output.status.success() {
        return Err(ProbeError::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse(&output.stdout)
}

/// Pure parse step, separate so tests can feed captured ffprobe JSON.
pub(crate) fn parse(json: &[u8]) -> Result<MediaInfo, ProbeError> {
    let raw: raw::Output =
        serde_json::from_slice(json).map_err(|e| ProbeError::Unparseable(e.to_string()))?;
    let format = raw
        .format
        .ok_or_else(|| ProbeError::Unparseable("no format section".into()))?;
    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitles = Vec::new();
    for s in raw.streams {
        if s.flag("attached_pic") {
            continue;
        }
        let codec_name = s.codec_name.as_deref().unwrap_or("");
        let language = s.tag("language").map(|l| l.to_lowercase());
        let title = s.tag("title").map(str::to_owned);
        let is_default = s.flag("default");
        match s.codec_type.as_deref() {
            Some("video") => video.push(VideoStream {
                index: s.index,
                codec: VideoCodec::from_name(codec_name),
                profile: s.profile.clone(),
                level: s.level,
                width: s.width.unwrap_or(0),
                height: s.height.unwrap_or(0),
                bit_depth: bit_depth(&s),
                frame_rate: s.r_frame_rate.as_deref().and_then(frame_rate),
                hdr: hdr_format(&s),
                color: ColorInfo {
                    primaries: s.color_primaries.clone(),
                    transfer: s.color_transfer.clone(),
                    space: s.color_space.clone(),
                },
                language,
                title,
                is_default,
            }),
            Some("audio") => audio.push(AudioStream {
                index: s.index,
                codec: if codec_name.starts_with("pcm_") {
                    AudioCodec::Pcm
                } else {
                    AudioCodec::from_name(codec_name)
                },
                profile: s.profile.clone(),
                channels: s.channels.unwrap_or(0),
                channel_layout: s.channel_layout.clone(),
                sample_rate: s.sample_rate.as_deref().and_then(|r| r.parse().ok()),
                language,
                title,
                is_default,
            }),
            Some("subtitle") => subtitles.push(SubtitleStream {
                index: s.index,
                codec: SubtitleCodec::from_name(codec_name),
                language,
                title,
                is_default,
                is_forced: s.flag("forced"),
            }),
            _ => {}
        }
    }
    if video.is_empty() {
        return Err(ProbeError::NoVideo);
    }
    let container = container(&format, &video, &audio, &subtitles);
    Ok(MediaInfo {
        container,
        duration_ms: format
            .duration
            .as_deref()
            .and_then(|d| d.parse::<f64>().ok())
            .map(|d| (d * 1000.0) as u64),
        bitrate: format.bit_rate.as_deref().and_then(|b| b.parse().ok()),
        video,
        audio,
        subtitles,
    })
}

/// ffprobe reports `matroska,webm` for both; WebM allows only VP8/VP9/AV1 +
/// Vorbis/Opus (+ WebVTT), so anything else inside means Matroska.
fn container(
    format: &raw::Format,
    video: &[VideoStream],
    audio: &[AudioStream],
    subs: &[SubtitleStream],
) -> Container {
    let name = format.format_name.as_str();
    if name.starts_with("mov,") {
        let quicktime = format
            .tags
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("major_brand") && v.trim() == "qt");
        return if quicktime {
            Container::Mov
        } else {
            Container::Mp4
        };
    }
    match name {
        "matroska,webm" => {
            let webm = video
                .iter()
                .all(|v| matches!(v.codec, VideoCodec::Vp8 | VideoCodec::Vp9 | VideoCodec::Av1))
                && audio
                    .iter()
                    .all(|a| matches!(a.codec, AudioCodec::Vorbis | AudioCodec::Opus))
                && subs.iter().all(|s| s.codec == SubtitleCodec::WebVtt);
            if webm {
                Container::WebM
            } else {
                Container::Matroska
            }
        }
        "mpegts" => Container::MpegTs,
        "avi" => Container::Avi,
        _ => Container::Other,
    }
}

fn bit_depth(s: &raw::Stream) -> Option<u8> {
    if let Some(bits) = s
        .bits_per_raw_sample
        .as_deref()
        .and_then(|b| b.parse::<u8>().ok())
    {
        return Some(bits);
    }
    let pix_fmt = s.pix_fmt.as_deref()?;
    Some(if pix_fmt.contains("16") {
        16
    } else if pix_fmt.contains("12") {
        12
    } else if pix_fmt.contains("10") {
        10
    } else {
        8
    })
}

/// `24000/1001` -> 23.976, rounded to a millihertz.
fn frame_rate(ratio: &str) -> Option<f64> {
    let (num, den) = ratio.split_once('/').unwrap_or((ratio, "1"));
    let (num, den): (f64, f64) = (num.parse().ok()?, den.parse().ok()?);
    (num > 0.0 && den > 0.0).then(|| (num / den * 1000.0).round() / 1000.0)
}

fn hdr_format(s: &raw::Stream) -> HdrFormat {
    let dovi = s.side_data_list.iter().find(|d| {
        d.get("side_data_type").and_then(|t| t.as_str()) == Some("DOVI configuration record")
    });
    if let Some(dovi) = dovi {
        let profile = dovi.get("dv_profile").and_then(|p| p.as_u64()).unwrap_or(0) as u8;
        let compat = dovi
            .get("dv_bl_signal_compatibility_id")
            .and_then(|c| c.as_u64());
        return HdrFormat::DolbyVision {
            profile,
            compatible_with_hdr10: matches!(compat, Some(1 | 6)),
        };
    }
    match s.color_transfer.as_deref() {
        Some("smpte2084") => HdrFormat::Hdr10,
        Some("arib-std-b67") => HdrFormat::Hlg,
        _ => HdrFormat::Sdr,
    }
}

impl HdrFormat {
    /// The family name stored in `media_stream.hdr` and sent by devices in
    /// `VideoSupport.hdr`; the Dolby Vision profile travels separately.
    pub fn as_str(self) -> &'static str {
        match self {
            HdrFormat::Sdr => "sdr",
            HdrFormat::Hdr10 => "hdr10",
            HdrFormat::Hlg => "hlg",
            HdrFormat::DolbyVision { .. } => "dolby_vision",
        }
    }

    /// Inverse of [`HdrFormat::as_str`]; unknown names are SDR.
    pub fn from_name(name: &str, dv_profile: Option<u8>) -> HdrFormat {
        match name {
            "hdr10" => HdrFormat::Hdr10,
            "hlg" => HdrFormat::Hlg,
            "dolby_vision" => {
                let profile = dv_profile.unwrap_or(0);
                HdrFormat::DolbyVision {
                    profile,
                    compatible_with_hdr10: profile == 8,
                }
            }
            _ => HdrFormat::Sdr,
        }
    }
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
        #[serde(default)]
        pub tags: std::collections::HashMap<String, String>,
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

    impl Stream {
        pub fn flag(&self, disposition: &str) -> bool {
            self.disposition.get(disposition) == Some(&1)
        }

        /// Tag keys vary in case between containers (`language` vs `LANGUAGE`).
        pub fn tag(&self, key: &str) -> Option<&str> {
            self.tags
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str())
        }
    }
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
        match s {
            "mp4" => Container::Mp4,
            "mov" => Container::Mov,
            "mkv" => Container::Matroska,
            "webm" => Container::WebM,
            "ts" => Container::MpegTs,
            "avi" => Container::Avi,
            _ => Container::Other,
        }
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

#[cfg(test)]
mod tests {
    //! Fixtures are generated per test with ffmpeg (`testsrc2` + `sine`, 2 s
    //! each) into a temp dir, then probed for real:
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

    use super::*;
    use crate::test_fixtures::{Fixture, ffmpeg};

    async fn probe_fixture(fixture: Fixture) -> MediaInfo {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture.generate(dir.path());
        let tools = MediaTools::locate(None, None).await.unwrap();
        probe(&tools, &path)
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[tokio::test]
    async fn h264_aac_mp4() {
        let info = probe_fixture(Fixture::H264AacMp4).await;
        assert_eq!(info.container, Container::Mp4);
        assert_eq!(
            info.duration_ms.map(|d| d / 100),
            Some(20),
            "about 2 s: {:?}",
            info.duration_ms
        );
        assert!(info.bitrate.is_some_and(|b| b > 0));
        let v = &info.video[0];
        assert_eq!(
            (v.codec.clone(), v.profile.as_deref(), v.width, v.height),
            (VideoCodec::H264, Some("High"), 1280, 720)
        );
        assert_eq!(
            (v.bit_depth, v.frame_rate, v.hdr),
            (Some(8), Some(23.976), HdrFormat::Sdr)
        );
        let a = info.default_audio().unwrap();
        assert_eq!(
            (a.codec.clone(), a.channels, a.sample_rate),
            (AudioCodec::Aac, 2, Some(48000))
        );
        assert!(info.subtitles.is_empty());
    }

    #[tokio::test]
    async fn hevc_main10_hdr10_eac3_mkv() {
        let info = probe_fixture(Fixture::Hevc10Eac3Mkv).await;
        assert_eq!(info.container, Container::Matroska);
        let v = &info.video[0];
        assert_eq!(
            (v.codec.clone(), v.bit_depth, v.hdr),
            (VideoCodec::Hevc, Some(10), HdrFormat::Hdr10)
        );
        assert_eq!(v.profile.as_deref(), Some("Main 10"));
        assert_eq!(v.color.transfer.as_deref(), Some("smpte2084"));
        let a = info.default_audio().unwrap();
        assert_eq!((a.codec.clone(), a.channels), (AudioCodec::Eac3, 6));
    }

    #[tokio::test]
    async fn vp9_opus_webm() {
        let info = probe_fixture(Fixture::Vp9OpusWebm).await;
        assert_eq!(info.container, Container::WebM);
        assert_eq!(info.video[0].codec, VideoCodec::Vp9);
        assert_eq!(info.default_audio().unwrap().codec, AudioCodec::Opus);
    }

    #[tokio::test]
    async fn av1_flac_mkv() {
        let info = probe_fixture(Fixture::Av1FlacMkv).await;
        assert_eq!(
            info.container,
            Container::Matroska,
            "flac is not a WebM codec"
        );
        assert_eq!(info.video[0].codec, VideoCodec::Av1);
        assert_eq!(info.default_audio().unwrap().codec, AudioCodec::Flac);
    }

    #[tokio::test]
    async fn mpeg4_ac3_avi() {
        let info = probe_fixture(Fixture::Mpeg4Ac3Avi).await;
        assert_eq!(info.container, Container::Avi);
        assert_eq!(info.video[0].codec, VideoCodec::Mpeg4);
        assert_eq!(info.default_audio().unwrap().codec, AudioCodec::Ac3);
    }

    #[tokio::test]
    async fn default_audio_and_forced_subtitles() {
        let info = probe_fixture(Fixture::SubsMkv).await;
        assert_eq!(info.audio.len(), 2);
        let a = info.default_audio().unwrap();
        assert_eq!(
            (a.index, a.language.as_deref(), a.is_default),
            (2, Some("fra"), true)
        );
        assert!(!info.audio[0].is_default);
        let s = &info.subtitles[0];
        assert_eq!(
            (
                s.codec.clone(),
                s.language.as_deref(),
                s.is_forced,
                s.is_default
            ),
            (SubtitleCodec::Srt, Some("eng"), true, true)
        );
    }

    #[tokio::test]
    async fn attached_cover_art_is_not_a_video_stream() {
        let info = probe_fixture(Fixture::CoverMp4).await;
        assert_eq!(info.video.len(), 1);
        assert_eq!(info.video[0].codec, VideoCodec::H264);
    }

    #[tokio::test]
    async fn unreadable_file_is_a_failed_probe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("garbage.mkv");
        std::fs::write(&path, b"not a movie").unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        assert!(matches!(
            probe(&tools, &path).await,
            Err(ProbeError::Failed { .. })
        ));
    }

    #[tokio::test]
    async fn audio_only_file_has_no_video() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.mp4");
        ffmpeg(
            &["-f", "lavfi", "-i", "sine=duration=1", "-c:a", "aac"],
            &path,
        );
        let tools = MediaTools::locate(None, None).await.unwrap();
        assert!(matches!(
            probe(&tools, &path).await,
            Err(ProbeError::NoVideo)
        ));
    }

    #[test]
    fn dolby_vision_from_side_data() {
        let json = r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc","width":3840,"height":2160,
            "pix_fmt":"yuv420p10le","color_transfer":"smpte2084","r_frame_rate":"24000/1001",
            "side_data_list":[{"side_data_type":"DOVI configuration record","dv_profile":8,"dv_bl_signal_compatibility_id":1}]}],
            "format":{"format_name":"matroska,webm","duration":"7200.5"}}"#;
        let info = parse(json.as_bytes()).unwrap();
        assert_eq!(
            info.video[0].hdr,
            HdrFormat::DolbyVision {
                profile: 8,
                compatible_with_hdr10: true
            }
        );
        assert_eq!(info.video[0].bit_depth, Some(10));
        assert_eq!(info.duration_ms, Some(7_200_500));
        assert_eq!(info.container, Container::Matroska);
    }

    #[test]
    fn quicktime_brand_is_mov() {
        let json = r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264"}],
            "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","tags":{"major_brand":"qt  "}}}"#;
        assert_eq!(parse(json.as_bytes()).unwrap().container, Container::Mov);
    }

    #[test]
    fn codec_names_round_trip_through_the_database_form() {
        for name in ["h264", "hevc", "av1", "vp9", "mpeg2video", "weird"] {
            assert_eq!(VideoCodec::from_name(name).as_str(), name);
        }
        for name in ["aac", "eac3", "truehd", "dts", "pcm"] {
            assert_eq!(AudioCodec::from_name(name).as_str(), name);
        }
        for c in [
            Container::Mp4,
            Container::Mov,
            Container::Matroska,
            Container::WebM,
            Container::MpegTs,
            Container::Avi,
        ] {
            assert_eq!(Container::from_name(c.as_str()), c);
        }
        for h in [HdrFormat::Sdr, HdrFormat::Hdr10, HdrFormat::Hlg] {
            assert_eq!(HdrFormat::from_name(h.as_str(), None), h);
        }
    }
}
