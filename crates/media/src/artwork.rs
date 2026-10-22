//! Artwork rendering: source bytes (a local JPEG/PNG/WebP or an ffmpeg frame
//! grab) in, the exact sizes the UI uses out, as lossy WebP, plus a ThumbHash.
//!
//! Encode path: decode with `image`, resize with `fast_image_resize` (Lanczos3,
//! SIMD), encode with libwebp (`webp` crate, statically built from source, so it
//! does not depend on the user's FFmpeg build). FFmpeg is only used to grab a
//! backdrop frame when a movie has no local backdrop.
//!
//! Sources: a poster comes from local artwork only; a cropped video frame makes
//! a poor poster. A backdrop comes from local artwork, else a frame at 10% of the
//! runtime, and only from SDR video: this FFmpeg has no `zscale`, so an HDR frame
//! cannot be tone-mapped and would look washed out.

use std::path::Path;
use std::time::Duration;

use crate::MediaTools;

/// Bump to re-render every image (new sizes, new quality). Folded into the
/// content hash by the store, so old and new files never share a name and
/// immutable caching stays correct.
pub const PIPELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtworkRole {
    /// 2:3. Rendered aspect is clamped to 0.6..=0.75 by centre crop. Local
    /// artwork only.
    Poster,
    /// 16:9. Clamped to 1.6..=1.9. Local artwork, else an SDR frame grab.
    Backdrop,
}

/// The two sizes the UI asks for. Pixel widths depend on the role and are
/// defined only in [`ImageSize::width`]; clients never see pixel numbers, they
/// ask for `small` (cards, rows) or `large` (detail hero).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageSize {
    Small,
    Large,
}

impl ImageSize {
    pub const ALL: [ImageSize; 2] = [ImageSize::Small, ImageSize::Large];

    /// Poster: 320 (a ~160 css px card at 2x, a 1080p TV card) / 640 (detail at
    /// 2x, 4K TV card). Backdrop: 960 (blurred row/hero background) / 1920.
    pub const fn width(self, role: ArtworkRole) -> u32 {
        match (role, self) {
            (ArtworkRole::Poster, ImageSize::Small) => 320,
            (ArtworkRole::Poster, ImageSize::Large) => 640,
            (ArtworkRole::Backdrop, ImageSize::Small) => 960,
            (ArtworkRole::Backdrop, ImageSize::Large) => 1920,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ImageSize::Small => "small",
            ImageSize::Large => "large",
        }
    }
}

/// ThumbHash bytes (~25). Rendered by clients into a blurred placeholder that
/// paints in the same frame as the card, before any image request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder(pub Vec<u8>);

/// One rendered artwork: every [`ImageSize`] as WebP, plus metadata the API
/// returns inline. Sizes are never upscaled: a 500 px source yields a 500 px
/// `large`, and `width`/`height` report what was produced.
#[derive(Debug, Clone)]
pub struct RenderedArtwork {
    pub role: ArtworkRole,
    /// Dimensions of the `large` variant (its aspect ratio is what clients lay out with).
    pub width: u32,
    pub height: u32,
    pub placeholder: Placeholder,
    /// Exactly one entry per [`ImageSize::ALL`], in that order.
    pub files: Vec<(ImageSize, Vec<u8>)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtworkError {
    #[error("image could not be decoded: {0}")]
    Decode(String),
    #[error("image is too small ({width}x{height})")]
    TooSmall { width: u32, height: u32 },
    #[error("ffmpeg frame grab failed: {0}")]
    FrameGrab(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What artwork is rendered from.
pub enum ArtworkSource<'a> {
    /// Bytes of a local image file (JPEG, PNG or WebP).
    File(&'a [u8]),
    /// A decoded SDR video frame from [`MediaTools::grab_frame`]. Backdrops only.
    Frame(&'a Frame),
}

impl ArtworkSource<'_> {
    /// The bytes the content hash is computed over (file bytes, or frame pixels).
    pub fn hash_input(&self) -> &[u8] {
        match self {
            ArtworkSource::File(b) => b,
            ArtworkSource::Frame(f) => &f.rgb,
        }
    }
}

/// One SDR frame as 8-bit RGB.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// RGB24, `width * height * 3` bytes.
    pub rgb: Vec<u8>,
}

impl RenderedArtwork {
    /// CPU-bound; call from `spawn_blocking`. Decode, clamp aspect by centre crop, resize to each size (never upscale), encode WebP
    /// q=82 method 4, ThumbHash from a 100 px-max downscale.
    pub fn render(
        role: ArtworkRole,
        source: ArtworkSource<'_>,
    ) -> Result<RenderedArtwork, ArtworkError> {
        let _ = (role, source);
        todo!()
    }
}

impl MediaTools {
    /// Grab one frame for a backdrop, scaled to 1920 px wide, as raw RGB24:
    /// `ffmpeg -ss <at> -i <path> -frames:v 1 -vf scale=1920:-2 -pix_fmt rgb24 -f rawvideo -`.
    /// `-ss` before `-i` = keyframe seek, ~100 ms even on 4K HEVC. Callers pass
    /// `at` = 10% of the runtime, and only for SDR video.
    pub async fn grab_frame(&self, path: &Path, at: Duration) -> Result<Frame, ArtworkError> {
        let _ = (path, at, &self.ffmpeg);
        todo!()
    }
}
