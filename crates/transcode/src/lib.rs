//! Converting video while it plays.
//!
//! - [`transcode`]: live HLS sessions, one FFmpeg run per player, paced to
//!   stay ahead of it and restarted where it seeks.
//! - [`copies`]: a 1080p copy of every video too heavy to stream as it is,
//!   made in the background.
//! - [`tuning`]: the admin panel's Transcoding settings, which both follow.
//!
//! What to convert, and how, comes from `nuvora_media`; this crate runs it.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod copies;
pub mod transcode;
pub mod tuning;

/// Which encoder converts video; see `nuvora_media::VideoEncoder`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HardwareEncoding {
    #[default]
    Auto,
    Off,
    Videotoolbox,
    Nvenc,
    Vaapi,
}

impl HardwareEncoding {
    pub const ALL: [HardwareEncoding; 5] = [
        HardwareEncoding::Auto,
        HardwareEncoding::Off,
        HardwareEncoding::Videotoolbox,
        HardwareEncoding::Nvenc,
        HardwareEncoding::Vaapi,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HardwareEncoding::Auto => "auto",
            HardwareEncoding::Off => "off",
            HardwareEncoding::Videotoolbox => "videotoolbox",
            HardwareEncoding::Nvenc => "nvenc",
            HardwareEncoding::Vaapi => "vaapi",
        }
    }
}

impl fmt::Display for HardwareEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for HardwareEncoding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        HardwareEncoding::ALL
            .into_iter()
            .find(|e| e.name() == s)
            .ok_or_else(|| {
                let names: Vec<_> = HardwareEncoding::ALL.iter().map(|e| e.name()).collect();
                format!("use one of {}", names.join(", "))
            })
    }
}
