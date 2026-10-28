//! Seek previews: a small frame every few seconds of a video, tiled into JPEG
//! sheets, so a player can show where a jump lands while the scrubber moves.
//!
//! Only keyframes are decoded (`-skip_frame nokey`), many times faster than
//! decoding every frame; a preview shows the keyframe at or before its time,
//! which is close enough to aim a jump. HDR frames are not tone-mapped (this
//! FFmpeg has no `zscale`), so their previews look washed out.
//!
//! A video is read through once, except a very big one: each of its previews
//! is a jump to its keyframe instead. A 4K remux is 80 GB, too much to read
//! in the half hour a render is allowed over a network share, while the jumps
//! read a few GB.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::MediaTools;

/// Bump when sheets change shape or look; stores key on it, so old sheets are
/// replaced rather than mixed with new ones.
pub const PREVIEW_VERSION: u32 = 1;

const WIDTH: u32 = 320;
const COLUMNS: u32 = 10;
const ROWS: u32 = 10;
/// A two-hour film takes a minute or two; this only catches a stuck decoder.
const TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Past this size a video's previews are taken one jump at a time.
const SEEK_ABOVE_BYTES: u64 = 10 << 30;
/// Previews taken at once when jumping.
const JUMPS_AT_ONCE: u64 = 2;

/// Where the previews of one video are: sheet `n` holds previews
/// `n * columns * rows ..`, left to right, then top to bottom. Preview `i`
/// shows the video at `i * interval_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewSheets {
    pub interval_ms: u32,
    /// One preview, in pixels.
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    /// Previews in all, the last sheet's included.
    pub count: u32,
    pub sheets: u32,
}

impl PreviewSheets {
    /// The preview for `position_ms`: (sheet, column, row).
    pub fn locate(&self, position_ms: u64) -> (u32, u32, u32) {
        let index =
            (position_ms / u64::from(self.interval_ms)).min(u64::from(self.count - 1)) as u32;
        let per_sheet = self.columns * self.rows;
        let within = index % per_sheet;
        (
            index / per_sheet,
            within % self.columns,
            within / self.columns,
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("ffmpeg timed out after {0:?}")]
    TimedOut(Duration),
    #[error("no previews came out")]
    Empty,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// About 300 previews in a long film, one every 2 seconds at most, every 10 at
/// least, in whole seconds so they line up with the time under the scrubber.
pub fn preview_interval_ms(duration_ms: u64) -> u32 {
    let seconds = (duration_ms / 300_000).clamp(2, 10);
    seconds as u32 * 1000
}

impl MediaTools {
    /// Renders the previews of `input` as `0.jpg`, `1.jpg`, ... into `out_dir`,
    /// which must exist and be empty.
    pub async fn render_previews(
        &self,
        input: &Path,
        duration_ms: u64,
        out_dir: &Path,
    ) -> Result<PreviewSheets, PreviewError> {
        let size = tokio::fs::metadata(input).await?.len();
        self.render_previews_by(size > SEEK_ABOVE_BYTES, input, duration_ms, out_dir)
            .await
    }

    async fn render_previews_by(
        &self,
        jumping: bool,
        input: &Path,
        duration_ms: u64,
        out_dir: &Path,
    ) -> Result<PreviewSheets, PreviewError> {
        let interval_ms = preview_interval_ms(duration_ms);
        if jumping {
            self.jump_through(input, duration_ms, interval_ms, out_dir)
                .await?;
        } else {
            self.read_through(input, interval_ms, out_dir).await?;
        }
        let sheets = (0..)
            .take_while(|n| out_dir.join(format!("{n}.jpg")).is_file())
            .count() as u32;
        if sheets == 0 {
            return Err(PreviewError::Empty);
        }
        let (sheet_width, sheet_height) = image::image_dimensions(out_dir.join("0.jpg"))
            .map_err(|e| PreviewError::Ffmpeg(format!("unreadable sheet: {e}")))?;
        let count = (duration_ms / u64::from(interval_ms) + 1)
            .min(u64::from(sheets * COLUMNS * ROWS)) as u32;
        Ok(PreviewSheets {
            interval_ms,
            width: sheet_width / COLUMNS,
            height: sheet_height / ROWS,
            columns: COLUMNS,
            rows: ROWS,
            count,
            sheets,
        })
    }

    /// One pass over the whole video, tiled by FFmpeg.
    async fn read_through(
        &self,
        input: &Path,
        interval_ms: u32,
        out_dir: &Path,
    ) -> Result<(), PreviewError> {
        // `eof_action=pass` keeps the last frame, or a clip shorter than one
        // interval has none. Square pixels before scaling, so anamorphic video
        // keeps its shape.
        let filter = format!(
            "fps=1000/{interval_ms}:eof_action=pass,scale=iw*sar:ih,scale={WIDTH}:-2,setsar=1,tile={COLUMNS}x{ROWS}"
        );
        let child = Command::new(&self.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-threads",
                "2",
                "-skip_frame",
                "nokey",
                "-i",
            ])
            .arg(input)
            .args(["-map", "0:V:0", "-an", "-sn", "-dn", "-vf", &filter])
            // JPEG wants full-range color; most video is limited-range.
            .args([
                "-pix_fmt",
                "yuvj420p",
                "-q:v",
                "5",
                "-start_number",
                "0",
                "-f",
                "image2",
            ])
            .arg(out_dir.join("%d.jpg"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| PreviewError::TimedOut(TIMEOUT))??;
        if !output.status.success() {
            return Err(PreviewError::Ffmpeg(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        Ok(())
    }

    /// A jump to each preview's keyframe, tiled here. A preview with no
    /// frame (past the video's last keyframe) stays black.
    async fn jump_through(
        &self,
        input: &Path,
        duration_ms: u64,
        interval_ms: u32,
        out_dir: &Path,
    ) -> Result<(), PreviewError> {
        let count = duration_ms / u64::from(interval_ms) + 1;
        let per_sheet = u64::from(COLUMNS * ROWS);
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let mut sheet: Option<image::RgbImage> = None;
        let mut first = 0;
        while first < count {
            let batch = first..(first + JUMPS_AT_ONCE).min(count);
            // Started together, so they run side by side.
            let children = batch
                .clone()
                .map(|i| self.spawn_keyframe(input, i * u64::from(interval_ms)))
                .collect::<Result<Vec<_>, _>>()?;
            let frames = tokio::time::timeout_at(deadline, async {
                let mut frames = Vec::with_capacity(children.len());
                for child in children {
                    frames.push(keyframe(child).await);
                }
                frames
            })
            .await
            .map_err(|_| PreviewError::TimedOut(TIMEOUT))?;
            for (i, frame) in batch.zip(frames) {
                let cell = (i % per_sheet) as u32;
                if let Some(frame) = frame? {
                    let canvas = sheet.get_or_insert_with(|| {
                        image::RgbImage::new(WIDTH * COLUMNS, frame.height() * ROWS)
                    });
                    let height = canvas.height() / ROWS;
                    image::imageops::replace(
                        canvas,
                        &frame,
                        i64::from(cell % COLUMNS * WIDTH),
                        i64::from(cell / COLUMNS * height),
                    );
                }
                if (u64::from(cell) + 1 == per_sheet || i + 1 == count)
                    && let Some(full) = sheet.take()
                {
                    let path = out_dir.join(format!("{}.jpg", i / per_sheet));
                    tokio::task::spawn_blocking(move || save_jpeg(&full, &path))
                        .await
                        .map_err(|e| PreviewError::Ffmpeg(e.to_string()))??;
                }
            }
            first += JUMPS_AT_ONCE;
        }
        Ok(())
    }

    /// FFmpeg writing the keyframe at or before `at_ms`, scaled to a
    /// preview, as PNG on stdout.
    fn spawn_keyframe(&self, input: &Path, at_ms: u64) -> std::io::Result<tokio::process::Child> {
        Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-threads", "1"])
            .args(["-skip_frame", "nokey", "-noaccurate_seek", "-ss"])
            .arg(format!("{:.3}", at_ms as f64 / 1000.0))
            .arg("-i")
            .arg(input)
            .args([
                "-map",
                "0:V:0",
                "-an",
                "-sn",
                "-dn",
                "-frames:v",
                "1",
                "-vf",
            ])
            .arg(format!("scale=iw*sar:ih,scale={WIDTH}:-2,setsar=1"))
            .args(["-f", "image2pipe", "-c:v", "png", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
    }
}

/// The frame a [`MediaTools::spawn_keyframe`] run writes; `None` when it found none.
async fn keyframe(child: tokio::process::Child) -> Result<Option<image::RgbImage>, PreviewError> {
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        return Err(PreviewError::Ffmpeg(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    if output.stdout.is_empty() {
        return Ok(None);
    }
    image::load_from_memory_with_format(&output.stdout, image::ImageFormat::Png)
        .map(|frame| Some(frame.to_rgb8()))
        .map_err(|e| PreviewError::Ffmpeg(format!("unreadable frame: {e}")))
}

fn save_jpeg(sheet: &image::RgbImage, path: &Path) -> Result<(), PreviewError> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    image::codecs::jpeg::JpegEncoder::new_with_quality(file, 80)
        .encode_image(sheet)
        .map_err(|e| PreviewError::Ffmpeg(format!("can't write a sheet: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_grows_with_length_between_two_and_ten_seconds() {
        assert_eq!(preview_interval_ms(30_000), 2000);
        assert_eq!(preview_interval_ms(20 * 60_000), 4000);
        assert_eq!(preview_interval_ms(45 * 60_000), 9000);
        assert_eq!(preview_interval_ms(3 * 3_600_000), 10_000);
    }

    #[test]
    fn a_position_finds_its_sheet_column_and_row() {
        let sheets = PreviewSheets {
            interval_ms: 10_000,
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            count: 250,
            sheets: 3,
        };
        assert_eq!(sheets.locate(0), (0, 0, 0));
        assert_eq!(sheets.locate(9_999), (0, 0, 0));
        assert_eq!(sheets.locate(125_000), (0, 2, 1));
        assert_eq!(sheets.locate(1_000_000), (1, 0, 0));
        assert_eq!(
            sheets.locate(99_000_000),
            (2, 9, 4),
            "past the end is the last preview"
        );
    }

    /// `seconds` of `size` test pattern at 24 fps in the usual limited-range
    /// yuv420p, with a keyframe every `gop` frames.
    fn video(dir: &Path, size: &str, seconds: u32, gop: u32) -> std::path::PathBuf {
        let path = dir.join("in.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={size}:rate=24:duration={seconds}"))
            .args([
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "ultrafast",
                "-g",
            ])
            .arg(gop.to_string())
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        path
    }

    #[tokio::test]
    async fn a_clip_shorter_than_one_interval_still_has_a_preview() {
        let dir = tempfile::tempdir().unwrap();
        let video = video(dir.path(), "320x180", 1, 250);
        let out = dir.path().join("previews");
        std::fs::create_dir(&out).unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();

        let sheets = tools.render_previews(&video, 1000, &out).await.unwrap();

        assert_eq!(
            (sheets.count, sheets.sheets, sheets.width, sheets.height),
            (1, 1, 320, 180)
        );
        assert_eq!(
            image::image_dimensions(out.join("0.jpg")).unwrap(),
            (3200, 1800)
        );
    }

    #[tokio::test]
    async fn renders_sheets_of_previews_from_a_video() {
        let dir = tempfile::tempdir().unwrap();
        // 250 s with a keyframe every second: 126 previews 2 s apart, a full
        // sheet of 100 and one of 26.
        let video = video(dir.path(), "640x360", 250, 24);
        let out = dir.path().join("previews");
        std::fs::create_dir(&out).unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();

        let sheets = tools.render_previews(&video, 250_000, &out).await.unwrap();

        assert_eq!(
            sheets,
            PreviewSheets {
                interval_ms: 2000,
                width: 320,
                height: 180,
                columns: 10,
                rows: 10,
                count: 126,
                sheets: 2,
            }
        );
        assert_eq!(
            image::image_dimensions(out.join("1.jpg")).unwrap(),
            (3200, 1800),
            "the last sheet is padded to full size"
        );
        assert!(!out.join("2.jpg").exists());
    }

    #[tokio::test]
    async fn jumping_to_each_preview_gives_the_same_sheets_as_reading_through() {
        let dir = tempfile::tempdir().unwrap();
        let video = video(dir.path(), "640x360", 250, 24);
        let tools = MediaTools::locate(None, None).await.unwrap();
        let mut made = Vec::new();
        for jumping in [false, true] {
            let out = dir.path().join(format!("jumping-{jumping}"));
            std::fs::create_dir(&out).unwrap();
            let sheets = tools
                .render_previews_by(jumping, &video, 250_000, &out)
                .await
                .unwrap();
            made.push((sheets, image::open(out.join("0.jpg")).unwrap().to_rgb8()));
        }
        let [(read, read_sheet), (jumped, jumped_sheet)] = <[_; 2]>::try_from(made).ok().unwrap();
        assert_eq!(read, jumped);
        // JPEG from two encoders differs a little everywhere; another frame
        // would differ a lot in its cell.
        let cell_difference = |cell: u32| {
            let (x0, y0) = (cell % COLUMNS * read.width, cell / COLUMNS * read.height);
            let mut sum = 0u64;
            for y in y0..y0 + read.height {
                for x in x0..x0 + read.width {
                    let (a, b) = (read_sheet.get_pixel(x, y), jumped_sheet.get_pixel(x, y));
                    sum += (0..3).map(|c| u64::from(a[c].abs_diff(b[c]))).sum::<u64>();
                }
            }
            sum / u64::from(read.width * read.height * 3)
        };
        let worst = (0..COLUMNS * ROWS).map(cell_difference).max().unwrap();
        assert!(worst < 8, "a preview differs by {worst} on average");
    }
}
