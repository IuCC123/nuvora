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

/// Upper bound on any converted stream, whatever the encoder.
const MAX_RATE: &str = "12M";
const BUFFER: &str = "24M";
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

    /// Options before the input. VideoToolbox decodes on the chip as well:
    /// converting a 1080p HEVC episode, that cut processor time by more than
    /// half again. FFmpeg decodes on the processor when the chip can't.
    pub(crate) fn input_args(&self) -> Vec<OsString> {
        match self {
            VideoEncoder::VideoToolbox => vec!["-hwaccel".into(), "videotoolbox".into()],
            VideoEncoder::Vaapi { device } => {
                vec!["-vaapi_device".into(), device.as_os_str().to_owned()]
            }
            _ => vec![],
        }
    }

    /// The video encode: scaled to at most `max_height` lines, with keyframes
    /// at the `forced` times (comma-separated seconds) and nowhere else.
    pub(crate) fn output_args(&self, max_height: u32, forced: &str) -> Vec<OsString> {
        let scale = format!("scale=-2:min(ih\\,{max_height})");
        let bitrate = bitrate_for(max_height);
        let args: Vec<String> = match self {
            VideoEncoder::Software => [
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "21",
                "-maxrate",
                MAX_RATE,
                "-bufsize",
                BUFFER,
                "-profile:v",
                "high",
                "-vf",
                &format!("{scale},format=yuv420p"),
                "-x264-params",
                "scenecut=0:keyint=100000:min-keyint=1",
                "-forced-idr",
                "1",
            ]
            .map(String::from)
            .to_vec(),
            VideoEncoder::VideoToolbox => [
                "-c:v",
                "h264_videotoolbox",
                "-b:v",
                bitrate,
                "-maxrate",
                MAX_RATE,
                "-profile:v",
                "high",
                "-vf",
                &format!("{scale},format=yuv420p"),
                "-g",
                NO_GOP,
            ]
            .map(String::from)
            .to_vec(),
            VideoEncoder::Nvenc => [
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p4",
                "-rc",
                "vbr",
                "-cq",
                "23",
                "-b:v",
                "0",
                "-maxrate",
                MAX_RATE,
                "-bufsize",
                BUFFER,
                "-profile:v",
                "high",
                "-vf",
                &format!("{scale},format=yuv420p"),
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
                bitrate,
                "-maxrate",
                MAX_RATE,
                "-profile:v",
                "high",
                "-vf",
                &format!("{scale},format=nv12,hwupload"),
                "-g",
                NO_GOP,
            ]
            .map(String::from)
            .to_vec(),
        };
        let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
        args.extend(["-force_key_frames".into(), OsString::from(forced)]);
        args
    }
}

/// For encoders that take a target bitrate instead of a quality.
fn bitrate_for(max_height: u32) -> &'static str {
    match max_height {
        0..=480 => "2500k",
        481..=720 => "5M",
        721..=1080 => "8M",
        _ => "16M",
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
    /// Two seconds of test pattern, converted the way a stream is, to nowhere.
    /// An encoder that isn't there (no such chip, no driver, an FFmpeg built
    /// without it) fails with FFmpeg's last word on why.
    pub async fn try_encoder(&self, encoder: &VideoEncoder) -> Result<(), String> {
        let mut command = Command::new(&self.ffmpeg);
        command
            .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
            .args(encoder.input_args())
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1280x720:rate=24:duration=2",
            ])
            .args(encoder.output_args(720, "0.000,1.000"))
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
            .args(encoder.input_args())
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1280x720:rate=24:duration=20",
            ])
            .args(encoder.output_args(720, "0.000,6.000,12.000,18.000"))
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
            .try_encoder(&VideoEncoder::VideoToolbox)
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
        assert!(tools.try_encoder(&VideoEncoder::Nvenc).await.is_err());
        let vaapi = VideoEncoder::Vaapi {
            device: DEFAULT_VAAPI_DEVICE.into(),
        };
        assert!(tools.try_encoder(&vaapi).await.is_err());
    }
}
