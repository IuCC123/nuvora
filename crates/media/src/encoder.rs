//! The H.264 encoder for converted video: x264 on the processor, or the
//! graphics chip's encoder when FFmpeg can use it, which converts many times
//! faster and leaves the processor free.
//!
//! Whatever the encoder, a converted stream needs keyframes exactly at the
//! times it is told and nowhere else: the HLS muxer starts a segment at every
//! keyframe, and every run must cut at the same times (see `hls`). Each
//! encoder's options say so in its own terms, and [`MediaTools::try_encoder`]
//! converts a test clip with exactly the options a stream uses, so a hardware
//! encoder is only used once it has worked here.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::MediaTools;

/// The render node VA-API drivers open on Linux.
pub const DEFAULT_VAAPI_DEVICE: &str = "/dev/dri/renderD128";

/// Far beyond any video: keyframes come only when forced.
const NO_GOP: &str = "100000";
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoEncoder {
    /// x264 on the processor: works everywhere, the slowest.
    Software,
    /// Apple's VideoToolbox, on Macs.
    VideoToolbox,
    /// NVIDIA's NVENC.
    Nvenc,
    /// VA-API through a render node: Intel and AMD graphics on Linux.
    Vaapi { device: PathBuf },
}

impl VideoEncoder {
    /// For logs and the admin panel.
    pub fn label(&self) -> &'static str {
        match self {
            VideoEncoder::Software => "x264 (processor)",
            VideoEncoder::VideoToolbox => "VideoToolbox (Apple graphics)",
            VideoEncoder::Nvenc => "NVENC (NVIDIA graphics)",
            VideoEncoder::Vaapi { .. } => "VA-API (Intel or AMD graphics)",
        }
    }

    pub fn is_hardware(&self) -> bool {
        *self != VideoEncoder::Software
    }

    /// Options before the input. VideoToolbox decodes on the chip as well
    /// (unless `options` say not to): converting a 1080p HEVC episode, that
    /// cut processor time by more than half again. FFmpeg decodes on the
    /// processor when the chip can't.
    pub(crate) fn input_args(&self, options: &EncodeOptions) -> Vec<OsString> {
        match self {
            VideoEncoder::VideoToolbox if options.hardware_decoding => {
                vec!["-hwaccel".into(), "videotoolbox".into()]
            }
            VideoEncoder::Vaapi { device } => {
                vec!["-vaapi_device".into(), device.as_os_str().to_owned()]
            }
            _ => vec![],
        }
    }

    /// The video encode: scaled to at most `max_height` lines (and the
    /// options' own ceiling), with keyframes at the `forced` times
    /// (comma-separated seconds) and nowhere else.
    pub(crate) fn output_args(
        &self,
        max_height: u32,
        forced: &str,
        options: &EncodeOptions,
    ) -> Vec<OsString> {
        let max_height = options.video_height(max_height);
        let mut filters = Vec::new();
        if options.deinterlace {
            // Only frames flagged interlaced are touched; one frame out per frame in.
            filters.push("yadif=mode=send_frame:deint=interlaced".to_owned());
        }
        filters.push(format!("scale=-2:min(ih\\,{max_height})"));
        filters.push(match self {
            VideoEncoder::Vaapi { .. } => "format=nv12,hwupload".to_owned(),
            _ => "format=yuv420p".to_owned(),
        });
        let filters = filters.join(",");
        let (auto_target, auto_max) = rates_for(max_height);
        let target = options
            .target_mbps
            .map_or(auto_target.to_owned(), |m| format!("{m}M"));
        let max_mbps = options.max_mbps.unwrap_or(auto_max);
        let (max, buffer) = (format!("{max_mbps}M"), format!("{}M", max_mbps * 2));
        let mut args: Vec<String> = match self {
            VideoEncoder::Software => {
                let mut a = vec![
                    "-c:v".into(),
                    "libx264".into(),
                    "-preset".into(),
                    options.x264_preset.into(),
                    "-crf".into(),
                    options.x264_crf.to_string(),
                ];
                if let Some(tune) = options.x264_tune {
                    a.extend(["-tune".into(), tune.into()]);
                }
                a.extend(
                    [
                        "-maxrate",
                        &max,
                        "-bufsize",
                        &buffer,
                        "-profile:v",
                        "high",
                        "-vf",
                        &filters,
                        "-x264-params",
                        "scenecut=0:keyint=100000:min-keyint=1",
                        "-forced-idr",
                        "1",
                    ]
                    .map(String::from),
                );
                a
            }
            VideoEncoder::VideoToolbox => [
                "-c:v",
                "h264_videotoolbox",
                "-b:v",
                &target,
                "-maxrate",
                &max,
                "-profile:v",
                "high",
                "-vf",
                &filters,
                "-g",
                NO_GOP,
            ]
            .map(String::from)
            .to_vec(),
            VideoEncoder::Nvenc => [
                "-c:v",
                "h264_nvenc",
                "-preset",
                &format!("p{}", options.nvenc_preset.clamp(1, 7)),
                "-rc",
                "vbr",
                "-cq",
                &options.nvenc_cq.to_string(),
                "-b:v",
                "0",
                "-maxrate",
                &max,
                "-bufsize",
                &buffer,
                "-profile:v",
                "high",
                "-vf",
                &filters,
                "-g",
                NO_GOP,
                "-no-scenecut",
                "1",
                "-forced-idr",
                "1",
            ]
            .map(String::from)
            .to_vec(),
            VideoEncoder::Vaapi { .. } => [
                "-c:v",
                "h264_vaapi",
                "-b:v",
                &target,
                "-maxrate",
                &max,
                "-profile:v",
                "high",
                "-vf",
                &filters,
                "-g",
                NO_GOP,
            ]
            .map(String::from)
            .to_vec(),
        };
        if options.threads > 0 {
            args.extend(["-threads".into(), options.threads.to_string()]);
        }
        let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
        args.extend(["-force_key_frames".into(), OsString::from(forced)]);
        args
    }
}

/// How a conversion is done, beyond the encoder: the admin panel's
/// Transcoding settings. The default is what Nuvora did before they existed.
/// Names are FFmpeg's own (`veryfast`, `animation`, `error`).
#[derive(Debug, Clone, PartialEq)]
pub struct EncodeOptions {
    /// Converted video is never taller than this, whatever the device takes.
    pub max_height: u32,
    /// Decode on the graphics chip too (VideoToolbox only).
    pub hardware_decoding: bool,
    pub x264_preset: &'static str,
    /// x264's quality: lower is better and bigger.
    pub x264_crf: u8,
    pub x264_tune: Option<&'static str>,
    /// NVENC's `p1` (fastest) to `p7` (best).
    pub nvenc_preset: u8,
    pub nvenc_cq: u8,
    /// The bitrate for encoders that take one (VideoToolbox, VA-API), in
    /// Mbit/s. `None`: by resolution.
    pub target_mbps: Option<u32>,
    /// The ceiling for every encoder, in Mbit/s. `None`: by resolution.
    pub max_mbps: Option<u32>,
    /// Deinterlace frames flagged interlaced.
    pub deinterlace: bool,
    /// Encoder threads; 0 lets FFmpeg decide.
    pub threads: u32,
    /// AAC bitrate for mono and stereo, kbit/s.
    pub stereo_kbps: u32,
    /// AAC bitrate per channel for surround, kbit/s.
    pub surround_kbps: u32,
    /// Converted audio is stereo at most.
    pub downmix_stereo: bool,
    /// Converted audio is compressed: loud scenes quieter, speech easier to hear.
    pub night_mode: bool,
    /// FFmpeg's `-loglevel` for stream runs, written to the session's log.
    pub log_level: &'static str,
}

impl EncodeOptions {
    /// The most lines converted video has, for a plan of `plan_height`.
    pub fn video_height(&self, plan_height: u32) -> u32 {
        plan_height.min(self.max_height.max(144))
    }

    /// Channels and kbit/s of converted audio, for a plan of `channels`.
    pub fn aac(&self, channels: u8) -> (u8, u32) {
        let channels = if self.downmix_stereo {
            channels.min(2)
        } else {
            channels
        };
        let kbps = if channels <= 2 {
            self.stereo_kbps
        } else {
            self.surround_kbps * u32::from(channels)
        };
        (channels, kbps)
    }
}

impl Default for EncodeOptions {
    fn default() -> Self {
        EncodeOptions {
            max_height: 2160,
            hardware_decoding: true,
            x264_preset: "veryfast",
            x264_crf: 21,
            x264_tune: None,
            nvenc_preset: 4,
            nvenc_cq: 23,
            target_mbps: None,
            max_mbps: None,
            deinterlace: false,
            threads: 0,
            stereo_kbps: 192,
            surround_kbps: 64,
            downmix_stereo: false,
            night_mode: false,
            log_level: "error",
        }
    }
}

/// Target and ceiling (Mbit/s for the ceiling). The target is for the
/// encoders that take a bitrate instead of a quality; the ceiling holds every
/// encoder's busiest scenes to what a home network carries.
fn rates_for(max_height: u32) -> (&'static str, u32) {
    match max_height {
        0..=480 => ("2500k", 12),
        481..=720 => ("5M", 12),
        721..=1080 => ("8M", 12),
        1081..=1440 => ("16M", 24),
        _ => ("30M", 40),
    }
}

/// The graphics encoders worth trying on this system, most capable first.
pub fn hardware_candidates() -> Vec<VideoEncoder> {
    if cfg!(target_os = "macos") {
        vec![VideoEncoder::VideoToolbox]
    } else if cfg!(target_os = "linux") {
        vec![
            VideoEncoder::Nvenc,
            VideoEncoder::Vaapi {
                device: DEFAULT_VAAPI_DEVICE.into(),
            },
        ]
    } else {
        vec![]
    }
}

impl MediaTools {
    /// Two seconds of test pattern, converted the way a stream is (with
    /// `options`), to nowhere.
    /// An encoder that isn't there (no such chip, no driver, an FFmpeg built
    /// without it) fails with FFmpeg's last word on why.
    pub async fn try_encoder(
        &self,
        encoder: &VideoEncoder,
        options: &EncodeOptions,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.ffmpeg);
        command
            .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
            .args(encoder.input_args(options))
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1280x720:rate=24:duration=2",
            ])
            .args(encoder.output_args(720, "0.000,1.000", options))
            .args(["-f", "null", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command.spawn().map_err(|e| e.to_string())?;
        let output = tokio::time::timeout(PROBE_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| format!("timed out after {PROBE_TIMEOUT:?}"))?
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr.lines().last().unwrap_or("failed").trim().to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes 20 s of moving test pattern with keyframes forced at 0, 6, 12
    /// and 18 s, and returns the keyframe times in the result.
    async fn keyframes_after(tools: &MediaTools, encoder: &VideoEncoder) -> Vec<f64> {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.mp4");
        let status = Command::new(&tools.ffmpeg)
            .args(["-v", "error", "-nostdin", "-y"])
            .args(encoder.input_args(&EncodeOptions::default()))
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1280x720:rate=24:duration=20",
            ])
            .args(encoder.output_args(720, "0.000,6.000,12.000,18.000", &EncodeOptions::default()))
            .arg(&out)
            .status()
            .await
            .unwrap();
        assert!(status.success(), "{} failed", encoder.label());
        crate::keyframes(tools, &out, 0).await.unwrap()
    }

    #[tokio::test]
    async fn x264_puts_keyframes_only_where_told() {
        let tools = MediaTools::locate(None, None).await.unwrap();
        assert_eq!(
            keyframes_after(&tools, &VideoEncoder::Software).await,
            [0.0, 6.0, 12.0, 18.0]
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn videotoolbox_works_here_and_puts_keyframes_only_where_told() {
        let tools = MediaTools::locate(None, None).await.unwrap();
        tools
            .try_encoder(&VideoEncoder::VideoToolbox, &EncodeOptions::default())
            .await
            .unwrap();
        assert_eq!(
            keyframes_after(&tools, &VideoEncoder::VideoToolbox).await,
            [0.0, 6.0, 12.0, 18.0]
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn an_encoder_this_machine_lacks_fails_the_test_clip() {
        let tools = MediaTools::locate(None, None).await.unwrap();
        let options = EncodeOptions::default();
        assert!(
            tools
                .try_encoder(&VideoEncoder::Nvenc, &options)
                .await
                .is_err()
        );
        let vaapi = VideoEncoder::Vaapi {
            device: DEFAULT_VAAPI_DEVICE.into(),
        };
        assert!(tools.try_encoder(&vaapi, &options).await.is_err());
    }
}
