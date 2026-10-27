//! "Can this device play this file as-is, and if not, how does the server
//! stream it?" The rules live here, server-side, as pure functions over the
//! probe result and a device's declared capabilities.
//!
//! Clients declare *facts* (what their decoder handles), measured with their own
//! platform API: the web app with `MediaSource.isTypeSupported`, Fire TV with
//! `MediaCodecList`, Vega with its media capabilities API. The server makes the
//! *decision* and the plan, so three clients never re-implement codec rules.

use crate::{AudioCodec, AudioStream, Container, HdrFormat, MediaInfo, VideoCodec, VideoStream};

/// What a device can decode natively. Sent by every client on every API call
/// (header `Nuvora-Device`), so the server holds no per-device state.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceProfile {
    pub containers: Vec<Container>,
    pub video: Vec<VideoSupport>,
    pub audio: Vec<AudioSupport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoSupport {
    pub codec: VideoCodec,
    pub max_width: u32,
    pub max_height: u32,
    /// 8 = no Hi10P/Main10. Browsers report this through the codec string probe
    /// (`avc1.6e0028` vs `avc1.640028`, `hvc1.2.4.L153` for Main10).
    pub max_bit_depth: u8,
    /// HDR formats rendered correctly. Empty = SDR only; HDR on an SDR-only
    /// device counts as an incompatibility (washed-out colours are a broken player).
    pub hdr: Vec<HdrFormat>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSupport {
    pub codec: AudioCodec,
    pub max_channels: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackDecision {
    DirectPlay,
    /// Plays through the server instead: remuxed to HLS, with audio and/or
    /// video converted as `plan` says. `reasons` lists every incompatibility.
    Convert {
        plan: StreamPlan,
        reasons: Vec<Incompatibility>,
    },
    /// Nothing the server can do makes it play on this device.
    Unplayable(Vec<Incompatibility>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incompatibility {
    Container(Container),
    VideoCodec(VideoCodec),
    Resolution {
        width: u32,
        height: u32,
    },
    BitDepth(u8),
    Hdr(HdrFormat),
    /// Judged on the *default* audio track: that is what `<video>` plays, and a
    /// silent movie is a broken player.
    AudioCodec(AudioCodec),
    AudioChannels(u8),
    /// ffprobe failed on this file; nothing can be decided.
    Unreadable,
}

/// How the server streams a file a device can't play as-is: HLS with fMP4
/// segments, so any codec the device decodes can pass through untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamPlan {
    pub video: VideoPlan,
    /// `None` for a file without audio.
    pub audio: Option<AudioPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoPlan {
    /// The video stream as it is; segments are cut at its keyframes.
    Copy,
    /// H.264 High, 8-bit SDR, scaled down to at most `max_height`.
    H264 { max_height: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioPlan {
    /// The audio stream's index in the file.
    pub stream: u32,
    pub output: AudioOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioOutput {
    Copy,
    Aac { channels: u8 },
}

/// Video codecs fMP4 HLS carries and players accept.
const HLS_VIDEO: [VideoCodec; 4] = [
    VideoCodec::H264,
    VideoCodec::Hevc,
    VideoCodec::Av1,
    VideoCodec::Vp9,
];
/// Audio codecs fMP4 HLS carries on every player (MP3, FLAC and Opus in
/// fMP4 are patchy: hls.js rejects MP3 there).
const HLS_AUDIO: [AudioCodec; 3] = [AudioCodec::Aac, AudioCodec::Ac3, AudioCodec::Eac3];
/// H.264's own 4K ceiling: level 5.2 stops at 4096 wide, and so do the
/// graphics encoders. An 8K source converts to 4K.
const MAX_CONVERT_WIDTH: u32 = 4096;
const MAX_CONVERT_HEIGHT: u32 = 2160;

impl DeviceProfile {
    /// Used when a request carries no profile: H.264 8-bit 1080p SDR + AAC
    /// stereo in MP4. Plays almost nowhere wrong, under-promises everywhere else.
    pub fn baseline() -> DeviceProfile {
        DeviceProfile {
            containers: vec![Container::Mp4],
            video: vec![VideoSupport {
                codec: VideoCodec::H264,
                max_width: 1920,
                max_height: 1080,
                max_bit_depth: 8,
                hdr: vec![],
            }],
            audio: vec![AudioSupport {
                codec: AudioCodec::Aac,
                max_channels: 2,
            }],
        }
    }

    /// Pure. Checks container, the primary video stream, and the default audio
    /// stream; when any fails, plans the conversion.
    pub fn decide(&self, media: &MediaInfo) -> PlaybackDecision {
        let mut why = Vec::new();
        if !self.containers.contains(&media.container) {
            why.push(Incompatibility::Container(media.container));
        }
        match media.video.first() {
            None => why.push(Incompatibility::Unreadable),
            Some(video) => why.extend(self.video_problems(video)),
        }
        if let Some(audio) = media.default_audio() {
            why.extend(self.audio_problem(audio));
        }
        if why.is_empty() {
            return PlaybackDecision::DirectPlay;
        }
        match self.plan(media, media.default_audio().map(|a| a.index)) {
            Some(plan) => PlaybackDecision::Convert { plan, reasons: why },
            None => PlaybackDecision::Unplayable(why),
        }
    }

    /// The HLS plan for `media` with audio stream `audio` (`None`: no audio).
    /// Passes each stream through when this device decodes it, converts it
    /// otherwise. `None` when there is no video or the device takes neither
    /// the video nor H.264 (or needs AAC and takes none).
    pub fn plan(&self, media: &MediaInfo, audio: Option<u32>) -> Option<StreamPlan> {
        self.plan_with(media, audio, true)
    }

    /// Like [`DeviceProfile::plan`], but the picture is always converted: a
    /// copied picture is cut at its keyframes, and finding them reads the
    /// whole file, which a file still downloading doesn't have yet.
    pub fn plan_converting(&self, media: &MediaInfo, audio: Option<u32>) -> Option<StreamPlan> {
        self.plan_with(media, audio, false)
    }

    fn plan_with(&self, media: &MediaInfo, audio: Option<u32>, copy: bool) -> Option<StreamPlan> {
        let video = media.video.first()?;
        let video =
            if copy && HLS_VIDEO.contains(&video.codec) && self.video_problems(video).is_empty() {
                VideoPlan::Copy
            } else {
                let h264 = self.video.iter().find(|s| s.codec == VideoCodec::H264)?;
                VideoPlan::H264 {
                    max_height: converted_height(video, h264),
                }
            };
        let audio = match audio {
            None => None,
            Some(index) => {
                let stream = media.audio.iter().find(|a| a.index == index)?;
                // Converted video starts exactly where asked, copied audio at the
                // keyframe before it: a converted picture gets converted sound.
                let output = if video == VideoPlan::Copy
                    && HLS_AUDIO.contains(&stream.codec)
                    && self.audio_problem(stream).is_none()
                {
                    AudioOutput::Copy
                } else {
                    let aac = self.audio.iter().find(|s| s.codec == AudioCodec::Aac)?;
                    AudioOutput::Aac {
                        channels: stream.channels.clamp(1, aac.max_channels.clamp(1, 6)),
                    }
                };
                Some(AudioPlan {
                    stream: index,
                    output,
                })
            }
        };
        Some(StreamPlan { video, audio })
    }

    fn video_problems(&self, video: &VideoStream) -> Vec<Incompatibility> {
        let Some(support) = self.video.iter().find(|s| s.codec == video.codec) else {
            return vec![Incompatibility::VideoCodec(video.codec.clone())];
        };
        let mut why = Vec::new();
        if video.width > support.max_width || video.height > support.max_height {
            why.push(Incompatibility::Resolution {
                width: video.width,
                height: video.height,
            });
        }
        let bit_depth = video.bit_depth.unwrap_or(8);
        if bit_depth > support.max_bit_depth {
            why.push(Incompatibility::BitDepth(bit_depth));
        }
        if !support.renders(video.hdr) {
            why.push(Incompatibility::Hdr(video.hdr));
        }
        why
    }

    fn audio_problem(&self, audio: &AudioStream) -> Option<Incompatibility> {
        match self.audio.iter().find(|s| s.codec == audio.codec) {
            None => Some(Incompatibility::AudioCodec(audio.codec.clone())),
            Some(support) if audio.channels > support.max_channels => {
                Some(Incompatibility::AudioChannels(audio.channels))
            }
            Some(_) => None,
        }
    }
}

/// The source's own height, unless the device's H.264 decoder or H.264 itself
/// stops short of it. The encoder keeps the shape, so a wide film is held to
/// the width as well. Even, as 4:2:0 video must be.
fn converted_height(video: &VideoStream, h264: &VideoSupport) -> u32 {
    let mut height = h264.max_height.min(MAX_CONVERT_HEIGHT);
    if video.width > 0 && video.height > 0 {
        let max_width = h264.max_width.min(MAX_CONVERT_WIDTH);
        let fits_width = u64::from(max_width) * u64::from(video.height) / u64::from(video.width);
        height = height.min(video.height).min(fits_width as u32);
    }
    height - height % 2
}

impl StreamPlan {
    /// A short, URL-safe name for the plan: `copy.a1`, `h264-1080.a2-aac2`,
    /// `copy.none`. [`StreamPlan::parse`] reads it back.
    pub fn code(&self) -> String {
        let video = match self.video {
            VideoPlan::Copy => "copy".to_owned(),
            VideoPlan::H264 { max_height } => format!("h264-{max_height}"),
        };
        let audio = match self.audio {
            None => "none".to_owned(),
            Some(AudioPlan {
                stream,
                output: AudioOutput::Copy,
            }) => format!("a{stream}"),
            Some(AudioPlan {
                stream,
                output: AudioOutput::Aac { channels },
            }) => {
                format!("a{stream}-aac{channels}")
            }
        };
        format!("{video}.{audio}")
    }

    pub fn parse(code: &str) -> Option<StreamPlan> {
        let (video, audio) = code.split_once('.')?;
        let video = match video {
            "copy" => VideoPlan::Copy,
            _ => {
                let height: u32 = video.strip_prefix("h264-")?.parse().ok()?;
                if !(144..=MAX_CONVERT_HEIGHT).contains(&height) {
                    return None;
                }
                VideoPlan::H264 { max_height: height }
            }
        };
        let audio = match audio {
            "none" => None,
            _ => {
                let rest = audio.strip_prefix('a')?;
                let (stream, output) = match rest.split_once("-aac") {
                    None => (rest, AudioOutput::Copy),
                    Some((stream, channels)) => {
                        let channels: u8 = channels.parse().ok()?;
                        if !(1..=6).contains(&channels) {
                            return None;
                        }
                        (stream, AudioOutput::Aac { channels })
                    }
                };
                Some(AudioPlan {
                    stream: stream.parse().ok()?,
                    output,
                })
            }
        };
        if matches!(video, VideoPlan::H264 { .. })
            && audio.is_some_and(|a| a.output == AudioOutput::Copy)
        {
            return None;
        }
        Some(StreamPlan { video, audio })
    }
}

impl VideoSupport {
    /// HDR formats compare by family (`hdr10`, `hlg`, `dolby_vision`). A Dolby
    /// Vision stream with an HDR10-compatible base layer (profile 8.1) plays
    /// as HDR10 on a device that renders HDR10.
    fn renders(&self, hdr: HdrFormat) -> bool {
        let has = |name: &str| self.hdr.iter().any(|h| h.as_str() == name);
        match hdr {
            HdrFormat::Sdr => true,
            HdrFormat::DolbyVision { .. } if hdr.has_hdr10_base() => {
                has("dolby_vision") || has("hdr10")
            }
            other => has(other.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Table: (fixture MediaInfo, profile, expected decision). Rows cover HEVC
    //! on a no-HEVC browser, Main10 on 8-bit-only, HDR10 on SDR, DTS default audio
    //! with an AAC secondary (=> NeedsTranscode(AudioCodec(Dts))), MKV on a
    //! WebM-only browser, 4K on a 1080p-max device, and the happy MP4 path.

    use super::*;
    use crate::{AudioStream, ColorInfo, VideoStream};

    fn video(
        codec: VideoCodec,
        width: u32,
        height: u32,
        bit_depth: u8,
        hdr: HdrFormat,
    ) -> VideoStream {
        VideoStream {
            index: 0,
            codec,
            profile: None,
            level: None,
            width,
            height,
            bit_depth: Some(bit_depth),
            frame_rate: Some(23.976),
            hdr,
            color: ColorInfo::default(),
            language: None,
            title: None,
            is_default: true,
        }
    }

    fn audio(index: u32, codec: AudioCodec, channels: u8, is_default: bool) -> AudioStream {
        AudioStream {
            index,
            codec,
            profile: None,
            channels,
            channel_layout: None,
            sample_rate: Some(48000),
            language: None,
            title: None,
            is_default,
        }
    }

    fn media(container: Container, video: VideoStream, audio: Vec<AudioStream>) -> MediaInfo {
        MediaInfo {
            container,
            duration_ms: Some(1000),
            bitrate: None,
            video: vec![video],
            audio,
            subtitles: vec![],
        }
    }

    /// A Chromium-like browser profile: H.264/HEVC/VP9/AV1 up to 4K, 10-bit
    /// HEVC with HDR10, AAC/Opus/FLAC, MP4/WebM/MKV.
    fn chromium() -> DeviceProfile {
        DeviceProfile {
            containers: vec![Container::Mp4, Container::WebM, Container::Matroska],
            video: vec![
                VideoSupport {
                    codec: VideoCodec::H264,
                    max_width: 3840,
                    max_height: 2160,
                    max_bit_depth: 8,
                    hdr: vec![],
                },
                VideoSupport {
                    codec: VideoCodec::Hevc,
                    max_width: 3840,
                    max_height: 2160,
                    max_bit_depth: 10,
                    hdr: vec![HdrFormat::Hdr10],
                },
                VideoSupport {
                    codec: VideoCodec::Vp9,
                    max_width: 3840,
                    max_height: 2160,
                    max_bit_depth: 10,
                    hdr: vec![],
                },
            ],
            audio: vec![
                AudioSupport {
                    codec: AudioCodec::Aac,
                    max_channels: 6,
                },
                AudioSupport {
                    codec: AudioCodec::Opus,
                    max_channels: 8,
                },
                AudioSupport {
                    codec: AudioCodec::Flac,
                    max_channels: 8,
                },
            ],
        }
    }

    #[test]
    fn decisions() {
        let h264_1080 = || video(VideoCodec::H264, 1920, 1080, 8, HdrFormat::Sdr);
        let aac2 = || vec![audio(1, AudioCodec::Aac, 2, true)];
        let dv81 = HdrFormat::DolbyVision {
            profile: 8,
            compatibility: 1,
        };
        let dv5 = HdrFormat::DolbyVision {
            profile: 5,
            compatibility: 0,
        };
        let cases: Vec<(&str, MediaInfo, DeviceProfile, Vec<Incompatibility>)> = vec![
            (
                "happy mp4",
                media(Container::Mp4, h264_1080(), aac2()),
                DeviceProfile::baseline(),
                vec![],
            ),
            (
                "hevc on baseline",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 1920, 1080, 8, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::VideoCodec(VideoCodec::Hevc)],
            ),
            (
                "main10 on 8-bit-only h264",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 1920, 1080, 10, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::BitDepth(10)],
            ),
            (
                "hdr10 on an sdr h264 device",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 1920, 1080, 8, HdrFormat::Hdr10),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::Hdr(HdrFormat::Hdr10)],
            ),
            (
                "dts default audio with an aac secondary",
                media(
                    Container::Mp4,
                    h264_1080(),
                    vec![
                        audio(1, AudioCodec::Dts, 6, true),
                        audio(2, AudioCodec::Aac, 2, false),
                    ],
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::AudioCodec(AudioCodec::Dts)],
            ),
            (
                "mkv on an mp4-only device",
                media(Container::Matroska, h264_1080(), aac2()),
                DeviceProfile::baseline(),
                vec![Incompatibility::Container(Container::Matroska)],
            ),
            (
                "4k on a 1080p device",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 3840, 2160, 8, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::Resolution {
                    width: 3840,
                    height: 2160,
                }],
            ),
            (
                "5.1 aac on a stereo device",
                media(
                    Container::Mp4,
                    h264_1080(),
                    vec![audio(1, AudioCodec::Aac, 6, true)],
                ),
                DeviceProfile::baseline(),
                vec![Incompatibility::AudioChannels(6)],
            ),
            (
                "every reason is listed",
                media(
                    Container::Matroska,
                    video(VideoCodec::Hevc, 3840, 2160, 10, HdrFormat::Hdr10),
                    vec![audio(1, AudioCodec::TrueHd, 8, true)],
                ),
                DeviceProfile::baseline(),
                vec![
                    Incompatibility::Container(Container::Matroska),
                    Incompatibility::VideoCodec(VideoCodec::Hevc),
                    Incompatibility::AudioCodec(AudioCodec::TrueHd),
                ],
            ),
            (
                "chromium plays hevc main10 hdr10 mkv",
                media(
                    Container::Matroska,
                    video(VideoCodec::Hevc, 3840, 2160, 10, HdrFormat::Hdr10),
                    aac2(),
                ),
                chromium(),
                vec![],
            ),
            (
                "dolby vision 8.1 plays as hdr10",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 3840, 2160, 10, dv81),
                    aac2(),
                ),
                chromium(),
                vec![],
            ),
            (
                "dolby vision profile 5 does not",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 3840, 2160, 10, dv5),
                    aac2(),
                ),
                chromium(),
                vec![Incompatibility::Hdr(dv5)],
            ),
            (
                "video-only file plays",
                media(Container::Mp4, h264_1080(), vec![]),
                DeviceProfile::baseline(),
                vec![],
            ),
            (
                "unknown codec never matches",
                media(
                    Container::Mp4,
                    video(
                        VideoCodec::Other("prores".into()),
                        1920,
                        1080,
                        8,
                        HdrFormat::Sdr,
                    ),
                    aac2(),
                ),
                chromium(),
                vec![Incompatibility::VideoCodec(VideoCodec::Other(
                    "prores".into(),
                ))],
            ),
        ];
        for (name, media, profile, want) in cases {
            let got = match profile.decide(&media) {
                PlaybackDecision::DirectPlay => vec![],
                PlaybackDecision::Convert { reasons, .. }
                | PlaybackDecision::Unplayable(reasons) => reasons,
            };
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn plans_pass_through_what_the_device_decodes() {
        let aac = |ch| vec![audio(1, AudioCodec::Aac, ch, true)];
        let plan = |media: &MediaInfo, profile: &DeviceProfile| match profile.decide(media) {
            PlaybackDecision::Convert { plan, .. } => Some(plan.code()),
            _ => None,
        };
        let h264 = || video(VideoCodec::H264, 1920, 1080, 8, HdrFormat::Sdr);
        // Container only: remux, nothing converted.
        let mkv = media(Container::Matroska, h264(), aac(2));
        assert_eq!(
            plan(&mkv, &DeviceProfile::baseline()).as_deref(),
            Some("copy.a1")
        );
        // DTS default track: video passes, audio becomes AAC (stereo on a stereo device).
        let dts = media(
            Container::Mp4,
            h264(),
            vec![audio(1, AudioCodec::Dts, 6, true)],
        );
        assert_eq!(
            plan(&dts, &DeviceProfile::baseline()).as_deref(),
            Some("copy.a1-aac2")
        );
        assert_eq!(plan(&dts, &chromium()).as_deref(), Some("copy.a1-aac6"));
        // HEVC on an H.264-only device: converted, no taller than the device
        // decodes, and its sound converted with it, so both start at the same moment.
        let uhd = media(
            Container::Matroska,
            video(VideoCodec::Hevc, 3840, 2160, 10, HdrFormat::Sdr),
            aac(2),
        );
        assert_eq!(
            plan(&uhd, &DeviceProfile::baseline()).as_deref(),
            Some("h264-1080.a1-aac2")
        );
        // A 4K device gets 4K: Dolby Vision 5 has no HDR10 base to pass through.
        let dv5 = media(
            Container::Matroska,
            video(
                VideoCodec::Hevc,
                3840,
                2160,
                10,
                HdrFormat::DolbyVision {
                    profile: 5,
                    compatibility: 0,
                },
            ),
            aac(2),
        );
        assert_eq!(
            plan(&dv5, &chromium()).as_deref(),
            Some("h264-2160.a1-aac2")
        );
        // 8K is beyond H.264 itself.
        let eight_k = media(
            Container::Matroska,
            video(VideoCodec::Av1, 7680, 4320, 10, HdrFormat::Sdr),
            aac(2),
        );
        assert_eq!(
            plan(&eight_k, &chromium()).as_deref(),
            Some("h264-2160.a1-aac2")
        );
        // A 2.40:1 4K film on a 1920-wide device: 1920x800, not 2592x1080.
        let scope = media(
            Container::Matroska,
            video(VideoCodec::Hevc, 3840, 1600, 10, HdrFormat::Sdr),
            aac(2),
        );
        assert_eq!(
            plan(&scope, &DeviceProfile::baseline()).as_deref(),
            Some("h264-800.a1-aac2")
        );
        let sd = media(
            Container::Avi,
            video(VideoCodec::Mpeg4, 720, 480, 8, HdrFormat::Sdr),
            vec![audio(1, AudioCodec::Mp3, 2, true)],
        );
        assert_eq!(plan(&sd, &chromium()).as_deref(), Some("h264-480.a1-aac2"));
        // No video stream: nothing to stream.
        let mut silent = media(Container::Matroska, h264(), aac(2));
        silent.video.clear();
        assert!(matches!(
            DeviceProfile::baseline().decide(&silent),
            PlaybackDecision::Unplayable(_)
        ));
    }

    #[test]
    fn plan_codes_round_trip() {
        for code in [
            "copy.a1",
            "copy.none",
            "h264-1080.a2-aac2",
            "h264-2160.a2-aac6",
            "h264-720.a0-aac6",
            "copy.a12-aac1",
        ] {
            assert_eq!(
                StreamPlan::parse(code).map(|p| p.code()).as_deref(),
                Some(code)
            );
        }
        for bad in [
            "",
            "copy",
            "h264-4000.a1",
            "h264-x.a1",
            "copy.a1-aac9",
            "copy.b1",
            "copy.a-1",
            "vp9.a1",
        ] {
            assert_eq!(StreamPlan::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn converting_plans_never_copy_the_picture() {
        let movie = media(
            Container::Matroska,
            video(VideoCodec::H264, 1920, 1080, 8, HdrFormat::Sdr),
            vec![audio(1, AudioCodec::Ac3, 6, true)],
        );
        let device = DeviceProfile::baseline();
        assert_eq!(device.plan(&movie, Some(1)).unwrap().code(), "copy.a1-aac2");
        assert_eq!(
            device.plan_converting(&movie, Some(1)).unwrap().code(),
            "h264-1080.a1-aac2"
        );
    }
}
