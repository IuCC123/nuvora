//! "Can this device play this file as-is?" The rules live here, server-side, as
//! one pure function over the probe result and a device's declared capabilities.
//!
//! Clients declare *facts* (what their decoder handles), measured with their own
//! platform API: the web app with `MediaSource.isTypeSupported`, Fire TV with
//! `MediaCodecList`, Vega with its media capabilities API. The server makes the
//! *decision*, because Step 3 must turn the same incompatibility list into a
//! remux/transcode plan, and three clients must not re-implement codec rules.

use crate::{AudioCodec, Container, HdrFormat, MediaInfo, VideoCodec};

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

/// The outcome. Step 3 adds `Remux { .. }` (container only) and `Transcode { .. }`
/// variants; every client already renders "unknown variant => needs transcoding",
/// so adding them is not a breaking change on the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackDecision {
    DirectPlay,
    /// Every reason, not just the first: Step 3 needs the full list to decide
    /// copy-video-transcode-audio vs full transcode.
    NeedsTranscode(Vec<Incompatibility>),
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
    /// stream. Subtitles never block direct play in Step 1 (not delivered yet).
    pub fn decide(&self, media: &MediaInfo) -> PlaybackDecision {
        let mut why = Vec::new();
        if !self.containers.contains(&media.container) {
            why.push(Incompatibility::Container(media.container));
        }
        match media.video.first() {
            None => why.push(Incompatibility::Unreadable),
            Some(video) => match self.video.iter().find(|s| s.codec == video.codec) {
                None => why.push(Incompatibility::VideoCodec(video.codec.clone())),
                Some(support) => {
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
                }
            },
        }
        if let Some(audio) = media.default_audio() {
            match self.audio.iter().find(|s| s.codec == audio.codec) {
                None => why.push(Incompatibility::AudioCodec(audio.codec.clone())),
                Some(support) if audio.channels > support.max_channels => {
                    why.push(Incompatibility::AudioChannels(audio.channels));
                }
                Some(_) => {}
            }
        }
        if why.is_empty() {
            PlaybackDecision::DirectPlay
        } else {
            PlaybackDecision::NeedsTranscode(why)
        }
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
        let cases: Vec<(&str, MediaInfo, DeviceProfile, PlaybackDecision)> = vec![
            (
                "happy mp4",
                media(Container::Mp4, h264_1080(), aac2()),
                DeviceProfile::baseline(),
                PlaybackDecision::DirectPlay,
            ),
            (
                "hevc on baseline",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 1920, 1080, 8, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::VideoCodec(
                    VideoCodec::Hevc,
                )]),
            ),
            (
                "main10 on 8-bit-only h264",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 1920, 1080, 10, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::BitDepth(10)]),
            ),
            (
                "hdr10 on an sdr h264 device",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 1920, 1080, 8, HdrFormat::Hdr10),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::Hdr(HdrFormat::Hdr10)]),
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
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::AudioCodec(
                    AudioCodec::Dts,
                )]),
            ),
            (
                "mkv on an mp4-only device",
                media(Container::Matroska, h264_1080(), aac2()),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::Container(
                    Container::Matroska,
                )]),
            ),
            (
                "4k on a 1080p device",
                media(
                    Container::Mp4,
                    video(VideoCodec::H264, 3840, 2160, 8, HdrFormat::Sdr),
                    aac2(),
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::Resolution {
                    width: 3840,
                    height: 2160,
                }]),
            ),
            (
                "5.1 aac on a stereo device",
                media(
                    Container::Mp4,
                    h264_1080(),
                    vec![audio(1, AudioCodec::Aac, 6, true)],
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::AudioChannels(6)]),
            ),
            (
                "every reason is listed",
                media(
                    Container::Matroska,
                    video(VideoCodec::Hevc, 3840, 2160, 10, HdrFormat::Hdr10),
                    vec![audio(1, AudioCodec::TrueHd, 8, true)],
                ),
                DeviceProfile::baseline(),
                PlaybackDecision::NeedsTranscode(vec![
                    Incompatibility::Container(Container::Matroska),
                    Incompatibility::VideoCodec(VideoCodec::Hevc),
                    Incompatibility::AudioCodec(AudioCodec::TrueHd),
                ]),
            ),
            (
                "chromium plays hevc main10 hdr10 mkv",
                media(
                    Container::Matroska,
                    video(VideoCodec::Hevc, 3840, 2160, 10, HdrFormat::Hdr10),
                    aac2(),
                ),
                chromium(),
                PlaybackDecision::DirectPlay,
            ),
            (
                "dolby vision 8.1 plays as hdr10",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 3840, 2160, 10, dv81),
                    aac2(),
                ),
                chromium(),
                PlaybackDecision::DirectPlay,
            ),
            (
                "dolby vision profile 5 does not",
                media(
                    Container::Mp4,
                    video(VideoCodec::Hevc, 3840, 2160, 10, dv5),
                    aac2(),
                ),
                chromium(),
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::Hdr(dv5)]),
            ),
            (
                "video-only file plays",
                media(Container::Mp4, h264_1080(), vec![]),
                DeviceProfile::baseline(),
                PlaybackDecision::DirectPlay,
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
                PlaybackDecision::NeedsTranscode(vec![Incompatibility::VideoCodec(
                    VideoCodec::Other("prores".into()),
                )]),
            ),
        ];
        for (name, media, profile, want) in cases {
            assert_eq!(profile.decide(&media), want, "{name}");
        }
    }
}
