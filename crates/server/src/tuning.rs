//! The admin panel's Transcoding settings: how video and audio are
//! converted, how far ahead of the player a stream runs and when it stops,
//! and the library's background tasks. Every field has a default, the way
//! Nuvora worked before the setting existed.
//!
//! Saved as JSON in the catalog's `setting` table. A field missing from what
//! was saved takes its default, so settings saved by an older version keep
//! working. The same shape goes over the wire (`/api/admin/tuning`).

use nuvora_library::Library;
use nuvora_media::EncodeOptions;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::HardwareEncoding;

const SETTING_KEY: &str = "tuning";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct Tuning {
    // Video
    /// Which chip converts video. A `hardware_encoding` in `nuvora.toml` or
    /// on the command line wins over this.
    pub hardware_encoding: HardwareEncoding,
    /// The render node VA-API opens (Linux).
    pub vaapi_device: String,
    /// Decode on the graphics chip as well as encode (VideoToolbox).
    pub hardware_decoding: bool,
    /// Converted video is never taller than this: 2160, 1440, 1080, 720 or 480.
    pub max_height: u32,
    pub x264_preset: X264Preset,
    /// x264's constant rate factor, 0 to 51: lower is better and bigger.
    pub x264_crf: u8,
    pub x264_tune: X264Tune,
    /// NVENC's preset, 1 (fastest) to 7 (best).
    pub nvenc_preset: u8,
    /// NVENC's constant quality, 1 to 51: lower is better and bigger.
    pub nvenc_cq: u8,
    /// Mbit/s for VideoToolbox and VA-API, 1 to 200. `null`: by resolution.
    pub target_mbps: Option<u32>,
    /// The bitrate ceiling for every encoder in Mbit/s, 1 to 200. `null`: by
    /// resolution.
    pub max_mbps: Option<u32>,
    /// Deinterlace frames flagged interlaced (old TV recordings, DVDs).
    pub deinterlace: bool,
    /// Encoder threads, 0 to 64; 0 lets FFmpeg decide.
    pub threads: u32,

    // Audio
    /// AAC bitrate for mono and stereo, kbit/s: 64 to 512.
    pub stereo_kbps: u32,
    /// AAC bitrate per surround channel, kbit/s: 32 to 192.
    pub surround_kbps: u32,
    /// Converted audio is never more than stereo.
    pub downmix_stereo: bool,
    /// Converted audio is compressed: loud scenes quieter, speech clearer.
    pub night_mode: bool,

    // Streaming
    /// Length of a converted segment, 2 to 10 seconds.
    pub segment_seconds: u32,
    /// Pause converting once this far ahead of the player.
    pub throttle: bool,
    /// How far ahead, 30 to 3600 seconds; converting resumes at half of it.
    pub buffer_ahead_s: u32,
    /// Segments this far behind the player are deleted: 60 to 7200 seconds.
    pub keep_behind_s: u32,
    /// A jump at most this far past what is made waits for it instead of
    /// restarting FFmpeg: 0 to 120 seconds.
    pub catch_up_s: u32,
    /// A stream nobody asks anything of for this long stops: 30 to 1800 s.
    pub idle_timeout_s: u32,
    /// Streams one person may have open, 1 to 10; one more stops their oldest.
    pub streams_per_user: u32,
    /// Videos converted at once across the server, 0 (no limit) to 32.
    pub max_conversions: u32,
    pub ffmpeg_log_level: FfmpegLogLevel,

    // Library
    /// Scan every library this often, 0 (never; only at startup and when
    /// asked) to 168 hours.
    pub scan_every_hours: u32,
    /// Make seek previews for every video.
    pub seek_previews: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        let encode = EncodeOptions::default();
        Tuning {
            hardware_encoding: HardwareEncoding::Auto,
            vaapi_device: nuvora_media::DEFAULT_VAAPI_DEVICE.to_owned(),
            hardware_decoding: encode.hardware_decoding,
            max_height: encode.max_height,
            x264_preset: X264Preset::Veryfast,
            x264_crf: encode.x264_crf,
            x264_tune: X264Tune::None,
            nvenc_preset: encode.nvenc_preset,
            nvenc_cq: encode.nvenc_cq,
            target_mbps: encode.target_mbps,
            max_mbps: encode.max_mbps,
            deinterlace: encode.deinterlace,
            threads: encode.threads,
            stereo_kbps: encode.stereo_kbps,
            surround_kbps: encode.surround_kbps,
            downmix_stereo: encode.downmix_stereo,
            night_mode: encode.night_mode,
            segment_seconds: nuvora_media::SEGMENT_SECONDS as u32,
            throttle: true,
            buffer_ahead_s: 180,
            keep_behind_s: 600,
            catch_up_s: 20,
            idle_timeout_s: 90,
            streams_per_user: 3,
            max_conversions: 0,
            ffmpeg_log_level: FfmpegLogLevel::Error,
            scan_every_hours: 0,
            seek_previews: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum X264Preset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum X264Tune {
    None,
    Film,
    Animation,
    Grain,
    Stillimage,
    Fastdecode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FfmpegLogLevel {
    Error,
    Warning,
    Info,
    Verbose,
}

/// A field out of range: its name and why.
pub type Invalid = (&'static str, String);

impl Tuning {
    /// Every number in its range.
    pub fn validate(&self) -> Result<(), Invalid> {
        fn range(field: &'static str, value: u32, min: u32, max: u32) -> Result<(), Invalid> {
            if (min..=max).contains(&value) {
                Ok(())
            } else {
                Err((field, format!("Use {min} to {max}")))
            }
        }
        if ![2160, 1440, 1080, 720, 480].contains(&self.max_height) {
            return Err(("max_height", "Use 2160, 1440, 1080, 720 or 480".into()));
        }
        let device = self.vaapi_device.trim();
        if !device.starts_with('/') || device.len() > 256 || device.chars().any(char::is_control) {
            return Err((
                "vaapi_device",
                "Use a full path, like /dev/dri/renderD128".into(),
            ));
        }
        range("x264_crf", self.x264_crf.into(), 0, 51)?;
        range("nvenc_preset", self.nvenc_preset.into(), 1, 7)?;
        range("nvenc_cq", self.nvenc_cq.into(), 1, 51)?;
        if let Some(target) = self.target_mbps {
            range("target_mbps", target, 1, 200)?;
        }
        if let Some(max) = self.max_mbps {
            range("max_mbps", max, 1, 200)?;
        }
        range("threads", self.threads, 0, 64)?;
        range("stereo_kbps", self.stereo_kbps, 64, 512)?;
        range("surround_kbps", self.surround_kbps, 32, 192)?;
        range("segment_seconds", self.segment_seconds, 2, 10)?;
        range("buffer_ahead_s", self.buffer_ahead_s, 30, 3600)?;
        range("keep_behind_s", self.keep_behind_s, 60, 7200)?;
        range("catch_up_s", self.catch_up_s, 0, 120)?;
        range("idle_timeout_s", self.idle_timeout_s, 30, 1800)?;
        range("streams_per_user", self.streams_per_user, 1, 10)?;
        range("max_conversions", self.max_conversions, 0, 32)?;
        range("scan_every_hours", self.scan_every_hours, 0, 168)?;
        Ok(())
    }

    pub fn encode_options(&self) -> EncodeOptions {
        EncodeOptions {
            max_height: self.max_height,
            hardware_decoding: self.hardware_decoding,
            x264_preset: match self.x264_preset {
                X264Preset::Ultrafast => "ultrafast",
                X264Preset::Superfast => "superfast",
                X264Preset::Veryfast => "veryfast",
                X264Preset::Faster => "faster",
                X264Preset::Fast => "fast",
                X264Preset::Medium => "medium",
                X264Preset::Slow => "slow",
                X264Preset::Slower => "slower",
                X264Preset::Veryslow => "veryslow",
            },
            x264_crf: self.x264_crf,
            x264_tune: match self.x264_tune {
                X264Tune::None => None,
                X264Tune::Film => Some("film"),
                X264Tune::Animation => Some("animation"),
                X264Tune::Grain => Some("grain"),
                X264Tune::Stillimage => Some("stillimage"),
                X264Tune::Fastdecode => Some("fastdecode"),
            },
            nvenc_preset: self.nvenc_preset,
            nvenc_cq: self.nvenc_cq,
            target_mbps: self.target_mbps,
            max_mbps: self.max_mbps,
            deinterlace: self.deinterlace,
            threads: self.threads,
            stereo_kbps: self.stereo_kbps,
            surround_kbps: self.surround_kbps,
            downmix_stereo: self.downmix_stereo,
            night_mode: self.night_mode,
            log_level: match self.ffmpeg_log_level {
                FfmpegLogLevel::Error => "error",
                FfmpegLogLevel::Warning => "warning",
                FfmpegLogLevel::Info => "info",
                FfmpegLogLevel::Verbose => "verbose",
            },
        }
    }

    /// What was saved, or the defaults. Saved settings that no longer parse
    /// or validate are logged and replaced by the defaults, never fatal.
    pub async fn load(library: &Library) -> Tuning {
        let saved = match library.setting(SETTING_KEY).await {
            Ok(saved) => saved,
            Err(e) => {
                tracing::warn!("cannot read transcoding settings, using defaults: {e}");
                return Tuning::default();
            }
        };
        let Some(json) = saved else {
            return Tuning::default();
        };
        match serde_json::from_str::<Tuning>(&json) {
            Ok(tuning) if tuning.validate().is_ok() => tuning,
            Ok(_) => {
                tracing::warn!("saved transcoding settings are out of range; using defaults");
                Tuning::default()
            }
            Err(e) => {
                tracing::warn!("saved transcoding settings don't parse ({e}); using defaults");
                Tuning::default()
            }
        }
    }

    pub async fn save(&self, library: &Library) -> Result<(), nuvora_library::QueryError> {
        let json = serde_json::to_string(self).expect("settings serialize");
        library.set_setting(SETTING_KEY, json).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_convert_the_way_nuvora_always_did() {
        let defaults = Tuning::default();
        assert_eq!(defaults.validate(), Ok(()));
        assert_eq!(defaults.encode_options(), EncodeOptions::default());
    }

    #[test]
    fn missing_fields_take_their_defaults_and_ranges_are_checked() {
        let saved: Tuning = serde_json::from_str(r#"{"x264_crf": 18}"#).unwrap();
        assert_eq!(
            saved,
            Tuning {
                x264_crf: 18,
                ..Tuning::default()
            }
        );
        let bad = Tuning {
            segment_seconds: 30,
            ..Tuning::default()
        };
        assert_eq!(
            bad.validate(),
            Err(("segment_seconds", "Use 2 to 10".into()))
        );
        let bad = Tuning {
            max_height: 1000,
            ..Tuning::default()
        };
        assert_eq!(bad.validate().unwrap_err().0, "max_height");
    }
}
