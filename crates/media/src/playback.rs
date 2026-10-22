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
    Resolution { width: u32, height: u32 },
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
        todo!()
    }

    /// Pure. Checks container, the primary video stream, and the default audio
    /// stream. Subtitles never block direct play in Step 1 (not delivered yet).
    pub fn decide(&self, media: &MediaInfo) -> PlaybackDecision {
        let _ = media;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    //! Table: (fixture MediaInfo, profile, expected decision). Rows cover HEVC
    //! on a no-HEVC browser, Main10 on 8-bit-only, HDR10 on SDR, DTS default audio
    //! with an AAC secondary (=> NeedsTranscode(AudioCodec(Dts))), MKV on a
    //! WebM-only browser, 4K on a 1080p-max device, and the happy MP4 path.
}
