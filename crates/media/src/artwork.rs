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
use std::process::Stdio;
use std::time::Duration;

use fast_image_resize::images::Image;
use fast_image_resize::{PixelType, Resizer};
use image::RgbImage;
use tokio::process::Command;

use crate::MediaTools;

/// Bump to re-render every image (new sizes, new quality). Folded into the
/// content hash by the store, so old and new files never share a name and
/// immutable caching stays correct.
pub const PIPELINE_VERSION: u32 = 1;

/// Sources with an edge under this are rejected: an icon-sized poster is worse
/// than the typographic card the UI draws without one.
const MIN_EDGE: u32 = 50;
const WEBP_QUALITY: f32 = 82.0;
const WEBP_METHOD: i32 = 4;
const THUMBHASH_MAX_EDGE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtworkRole {
    /// 2:3. Rendered aspect is clamped to 0.6..=0.75 by centre crop. Local
    /// artwork only.
    Poster,
    /// 16:9. Clamped to 1.6..=1.9. Local artwork, else an SDR frame grab.
    Backdrop,
}

impl ArtworkRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            ArtworkRole::Poster => "poster",
            ArtworkRole::Backdrop => "backdrop",
        }
    }

    pub fn from_name(name: &str) -> Option<ArtworkRole> {
        match name {
            "poster" => Some(ArtworkRole::Poster),
            "backdrop" => Some(ArtworkRole::Backdrop),
            _ => None,
        }
    }

    /// Allowed width/height range; outside it the source is centre-cropped.
    const fn aspect_bounds(self) -> (f64, f64) {
        match self {
            ArtworkRole::Poster => (0.6, 0.75),
            ArtworkRole::Backdrop => (1.6, 1.9),
        }
    }
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
        let decoded: RgbImage = match source {
            ArtworkSource::File(bytes) => image::load_from_memory(bytes)
                .map_err(|e| ArtworkError::Decode(e.to_string()))?
                .to_rgb8(),
            ArtworkSource::Frame(frame) => {
                RgbImage::from_raw(frame.width, frame.height, frame.rgb.clone()).ok_or_else(
                    || ArtworkError::Decode("frame buffer does not match its dimensions".into()),
                )?
            }
        };
        let (width, height) = decoded.dimensions();
        if width < MIN_EDGE || height < MIN_EDGE {
            return Err(ArtworkError::TooSmall { width, height });
        }
        let (x, y, crop_w, crop_h) = centre_crop(width, height, role.aspect_bounds());
        let cropped = if (crop_w, crop_h) == (width, height) {
            decoded
        } else {
            image::imageops::crop_imm(&decoded, x, y, crop_w, crop_h).to_image()
        };
        let src = Image::from_vec_u8(crop_w, crop_h, cropped.into_raw(), PixelType::U8x3)
            .map_err(|e| ArtworkError::Decode(e.to_string()))?;
        let mut resizer = Resizer::new();

        let mut files = Vec::with_capacity(ImageSize::ALL.len());
        let (mut large_w, mut large_h) = (crop_w, crop_h);
        for size in ImageSize::ALL {
            let (w, h) = fit_width(crop_w, crop_h, size.width(role));
            let resized = resize(&mut resizer, &src, w, h)?;
            files.push((size, encode_webp(resized.buffer(), w, h)?));
            if size == ImageSize::Large {
                (large_w, large_h) = (w, h);
            }
        }

        let (tw, th) = fit_box(crop_w, crop_h, THUMBHASH_MAX_EDGE);
        let thumb = resize(&mut resizer, &src, tw, th)?;
        let rgba: Vec<u8> = thumb
            .buffer()
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        let placeholder = Placeholder(thumbhash::rgba_to_thumb_hash(
            tw as usize,
            th as usize,
            &rgba,
        ));

        Ok(RenderedArtwork {
            role,
            width: large_w,
            height: large_h,
            placeholder,
            files,
        })
    }
}

/// The largest centred window of `width` x `height` whose aspect lies in `(min, max)`.
fn centre_crop(width: u32, height: u32, (min, max): (f64, f64)) -> (u32, u32, u32, u32) {
    let ratio = f64::from(width) / f64::from(height);
    if ratio > max {
        let w = ((f64::from(height) * max).round() as u32).clamp(1, width);
        ((width - w) / 2, 0, w, height)
    } else if ratio < min {
        let h = ((f64::from(width) / min).round() as u32).clamp(1, height);
        (0, (height - h) / 2, width, h)
    } else {
        (0, 0, width, height)
    }
}

/// Scale to `max_width` wide, never up.
fn fit_width(width: u32, height: u32, max_width: u32) -> (u32, u32) {
    let w = width.min(max_width);
    let h = ((u64::from(height) * u64::from(w)) / u64::from(width)).max(1) as u32;
    (w, h)
}

/// Scale so both edges are at most `max_edge`, never up.
fn fit_box(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    if width >= height {
        fit_width(width, height, max_edge)
    } else {
        let (h, w) = fit_width(height, width, max_edge);
        (w, h)
    }
}

fn resize(
    resizer: &mut Resizer,
    src: &Image<'_>,
    w: u32,
    h: u32,
) -> Result<Image<'static>, ArtworkError> {
    let mut dst = Image::new(w, h, PixelType::U8x3);
    if (w, h) == (src.width(), src.height()) {
        dst.buffer_mut().copy_from_slice(src.buffer());
        return Ok(dst);
    }
    resizer
        .resize(src, &mut dst, None)
        .map_err(|e| ArtworkError::Decode(e.to_string()))?;
    Ok(dst)
}

fn encode_webp(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ArtworkError> {
    let mut config =
        webp::WebPConfig::new().map_err(|()| ArtworkError::Decode("libwebp config".into()))?;
    config.quality = WEBP_QUALITY;
    config.method = WEBP_METHOD;
    let encoded = webp::Encoder::from_rgb(rgb, width, height)
        .encode_advanced(&config)
        .map_err(|e| ArtworkError::Decode(format!("webp encode: {e:?}")))?;
    Ok(encoded.to_vec())
}

impl MediaTools {
    /// Grab one frame for a backdrop, at most 1920 px wide and never upscaled,
    /// as raw RGB24. `-ss` before `-i` = keyframe seek, ~100 ms even on 4K HEVC.
    /// Callers pass `at` = 10% of the runtime, and only for SDR video. The frame
    /// travels as PNG so its dimensions come back with it.
    pub async fn grab_frame(&self, path: &Path, at: Duration) -> Result<Frame, ArtworkError> {
        let child = Command::new(&self.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-ss",
                &format!("{:.3}", at.as_secs_f64()),
                "-i",
            ])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=min(1920\\,iw):-2",
                "-pix_fmt",
                "rgb24",
            ])
            .args(["-f", "image2pipe", "-vcodec", "png", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let output = tokio::time::timeout(self.probe_timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                ArtworkError::FrameGrab(format!("timed out after {:?}", self.probe_timeout))
            })??;
        if !output.status.success() {
            return Err(ArtworkError::FrameGrab(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        if output.stdout.is_empty() {
            return Err(ArtworkError::FrameGrab("no frame decoded".into()));
        }
        let png = image::load_from_memory(&output.stdout)
            .map_err(|e| ArtworkError::FrameGrab(e.to_string()))?
            .to_rgb8();
        let (width, height) = png.dimensions();
        Ok(Frame {
            width,
            height,
            rgb: png.into_raw(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::Fixture;

    /// A PNG with a diagonal gradient, so resizing has real content to filter.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, 128])
        });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn dims(webp: &[u8]) -> (u32, u32) {
        image::load_from_memory(webp)
            .unwrap()
            .to_rgb8()
            .dimensions()
    }

    fn render_file(role: ArtworkRole, width: u32, height: u32) -> RenderedArtwork {
        RenderedArtwork::render(role, ArtworkSource::File(&png(width, height))).unwrap()
    }

    #[test]
    fn poster_sizes() {
        let art = render_file(ArtworkRole::Poster, 1000, 1500);
        assert_eq!((art.width, art.height), (640, 960));
        assert_eq!(
            art.files.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            ImageSize::ALL
        );
        assert_eq!(dims(&art.files[0].1), (320, 480));
        assert_eq!(dims(&art.files[1].1), (640, 960));
        assert!(art.files[1].1.starts_with(b"RIFF"), "webp container");
        assert!(art.placeholder.0.len() >= 5);
    }

    #[test]
    fn never_upscaled() {
        let art = render_file(ArtworkRole::Poster, 400, 600);
        assert_eq!((art.width, art.height), (400, 600));
        assert_eq!(dims(&art.files[0].1), (320, 480));
        assert_eq!(dims(&art.files[1].1), (400, 600));
    }

    #[test]
    fn aspect_is_clamped_by_centre_crop() {
        let square_poster = render_file(ArtworkRole::Poster, 1000, 1000);
        assert_eq!((square_poster.width, square_poster.height), (640, 853));
        let square_backdrop = render_file(ArtworkRole::Backdrop, 1000, 1000);
        assert_eq!((square_backdrop.width, square_backdrop.height), (1000, 625));
        let tall_backdrop = render_file(ArtworkRole::Backdrop, 1920, 1440);
        assert_eq!((tall_backdrop.width, tall_backdrop.height), (1920, 1200));
        let wide_poster = render_file(ArtworkRole::Poster, 1200, 500);
        assert_eq!((wide_poster.width, wide_poster.height), (375, 500));
    }

    #[test]
    fn tiny_and_broken_sources_are_errors() {
        let tiny = RenderedArtwork::render(ArtworkRole::Poster, ArtworkSource::File(&png(40, 60)));
        assert!(matches!(
            tiny,
            Err(ArtworkError::TooSmall {
                width: 40,
                height: 60
            })
        ));
        let broken =
            RenderedArtwork::render(ArtworkRole::Poster, ArtworkSource::File(b"not an image"));
        assert!(matches!(broken, Err(ArtworkError::Decode(_))));
    }

    #[test]
    fn placeholder_decodes_to_the_same_shape() {
        let art = render_file(ArtworkRole::Poster, 600, 900);
        let (w, h, _) = thumbhash::thumb_hash_to_rgba(&art.placeholder.0).unwrap();
        assert!((w as f64 / h as f64 - 2.0 / 3.0).abs() < 0.1, "{w}x{h}");
    }

    #[tokio::test]
    async fn frame_grab_renders_a_backdrop() {
        let dir = tempfile::tempdir().unwrap();
        let path = Fixture::H264AacMp4.generate(dir.path());
        let tools = MediaTools::locate(None, None).await.unwrap();
        let frame = tools
            .grab_frame(&path, Duration::from_millis(200))
            .await
            .unwrap();
        assert_eq!(
            (frame.width, frame.height, frame.rgb.len()),
            (1280, 720, 1280 * 720 * 3)
        );
        let art =
            RenderedArtwork::render(ArtworkRole::Backdrop, ArtworkSource::Frame(&frame)).unwrap();
        assert_eq!((art.width, art.height), (1280, 720));
        assert_eq!(dims(&art.files[0].1), (960, 540));
    }

    #[tokio::test]
    async fn frame_grab_past_the_end_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = Fixture::H264AacMp4.generate(dir.path());
        let tools = MediaTools::locate(None, None).await.unwrap();
        let result = tools.grab_frame(&path, Duration::from_secs(600)).await;
        assert!(
            matches!(result, Err(ArtworkError::FrameGrab(_))),
            "{:?}",
            result.map(|f| f.width)
        );
    }
}
