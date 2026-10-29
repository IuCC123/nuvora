//! MP4's own index. An MP4 or QuickTime file's `moov` box lists every video
//! sample and which of them are keyframes (`stss`), the table FFmpeg itself
//! reads them from. Reading it takes a few megabytes; finding the keyframes
//! with ffprobe reads every packet of the file, 13 to 24 s for a 600 MB
//! episode on a network share before its first segment could be made.
//!
//! The times are worked out the way FFmpeg gives them to packets: decode time
//! plus composition offset, less where the edit list starts the track, plus
//! any empty edit before that. A file with more than one edit isn't read
//! here, and [`crate::hls::keyframes`] checks the first keyframes against
//! what ffprobe finds at the start of the file before trusting the rest.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// A `moov` for hours of high frame rate video is a few megabytes.
const MAX_MOOV: u64 = 128 << 20;
/// Top-level boxes looked through for `moov` before giving up.
const MAX_BOXES: usize = 64;
/// What an MP4 or QuickTime file starts with.
const FIRST_BOXES: [&[u8; 4]; 6] = [b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide"];

type Kind = [u8; 4];

/// Source times of every keyframe of the first video track, ascending, from
/// the file's index. `None` when the file isn't MP4 or QuickTime, or its
/// index isn't one this reads (fragmented, several edits, no keyframe table).
pub fn video_keyframes(path: &Path) -> io::Result<Option<Vec<f64>>> {
    let mut file = File::open(path)?;
    let Some(moov) = read_moov(&mut file)? else {
        return Ok(None);
    };
    Ok(keyframes(&moov))
}

fn read_moov(file: &mut File) -> io::Result<Option<Vec<u8>>> {
    let len = file.metadata()?.len();
    let mut at = 0;
    for n in 0..MAX_BOXES {
        if at + 8 > len {
            return Ok(None);
        }
        file.seek(SeekFrom::Start(at))?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8])?;
        let kind: Kind = head[4..8].try_into().expect("4 bytes");
        if n == 0 && !FIRST_BOXES.contains(&&kind) {
            return Ok(None);
        }
        let (size, header) = match u32::from_be_bytes(head[..4].try_into().expect("4 bytes")) {
            0 => (len - at, 8),
            1 => {
                file.read_exact(&mut head[8..])?;
                (
                    u64::from_be_bytes(head[8..].try_into().expect("8 bytes")),
                    16,
                )
            }
            size => (u64::from(size), 8),
        };
        if size < header || at + size > len {
            return Ok(None);
        }
        if &kind == b"moov" {
            if size - header > MAX_MOOV {
                return Ok(None);
            }
            let mut moov = vec![0; (size - header) as usize];
            file.read_exact(&mut moov)?;
            return Ok(Some(moov));
        }
        at += size;
    }
    Ok(None)
}

fn keyframes(moov: &[u8]) -> Option<Vec<f64>> {
    let movie_scale = timescale(child(moov, b"mvhd")?)?;
    let trak = boxes(moov)
        .filter(|(kind, _)| kind == b"trak")
        .map(|(_, body)| body)
        .find(|trak| handler(trak) == Some(*b"vide"))?;
    let mdia = child(trak, b"mdia")?;
    let scale = timescale(child(mdia, b"mdhd")?)?;
    let stbl = child(child(mdia, b"minf")?, b"stbl")?;
    let (empty, start) = match child(trak, b"edts").and_then(|edts| child(edts, b"elst")) {
        Some(elst) => edits(elst)?,
        None => (0, 0),
    };
    let deltas = runs(child(stbl, b"stts")?)?;
    let offsets = match child(stbl, b"ctts") {
        Some(ctts) => runs(ctts)?,
        None => Vec::new(),
    };
    // Without `stss` every sample is a keyframe: not a file to cut at all of them.
    let mut sync = numbers(child(stbl, b"stss")?)?;
    sync.sort_unstable();
    sync.dedup();

    let mut wanted = sync.into_iter().peekable();
    let mut offsets = expand(&offsets);
    let mut decode = 0i64;
    let mut ticks = Vec::new();
    for (i, delta) in expand(&deltas).enumerate() {
        let offset = offsets.next().unwrap_or(0);
        if wanted.peek() == Some(&(i as u64 + 1)) {
            wanted.next();
            ticks.push(decode + offset);
        }
        if wanted.peek().is_none() {
            break;
        }
        decode += delta;
    }
    if wanted.next().is_some() || ticks.is_empty() {
        return None;
    }
    let lead = empty as f64 / movie_scale as f64;
    let mut times: Vec<f64> = ticks
        .into_iter()
        .map(|t| ((t - start) as f64 / scale as f64 + lead) * 1e6)
        .map(|us| us.round() / 1e6)
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    Some(times)
}

/// `mvhd`'s or `mdhd`'s ticks per second.
fn timescale(full_box: &[u8]) -> Option<u64> {
    let at = if *full_box.first()? == 1 { 20 } else { 12 };
    Some(u64::from(be32(full_box, at)?)).filter(|s| *s > 0)
}

/// The track's kind (`vide`, `soun`, ...) from `mdia/hdlr`.
fn handler(trak: &[u8]) -> Option<Kind> {
    child(child(trak, b"mdia")?, b"hdlr")?
        .get(8..12)?
        .try_into()
        .ok()
}

/// The empty time before the track (movie ticks) and the media time it
/// starts at (track ticks). `None` with more than one edit that shows media.
fn edits(elst: &[u8]) -> Option<(u64, i64)> {
    let wide = *elst.first()? == 1;
    let count = be32(elst, 4)? as usize;
    let entry = if wide { 20 } else { 12 };
    let mut empty = 0;
    let mut start = None;
    for i in 0..count {
        let at = 8 + i * entry;
        let (duration, media_time) = if wide {
            (be64(elst, at)?, be64(elst, at + 8)? as i64)
        } else {
            (
                u64::from(be32(elst, at)?),
                i64::from(be32(elst, at + 4)? as i32),
            )
        };
        match (media_time, start) {
            (-1, None) => empty += duration,
            (_, None) => start = Some(media_time),
            // A second edit showing media, or an empty one after it: the track is cut up.
            (_, Some(_)) => return None,
        }
    }
    Some((empty, start.unwrap_or(0)))
}

/// `stts` or `ctts` entries: (sample count, value), the value signed as FFmpeg reads it.
fn runs(table: &[u8]) -> Option<Vec<(u32, i64)>> {
    let count = be32(table, 4)? as usize;
    (0..count)
        .map(|i| {
            Some((
                be32(table, 8 + i * 8)?,
                i64::from(be32(table, 12 + i * 8)? as i32),
            ))
        })
        .collect()
}

/// `stss` entries: 1-based sample numbers.
fn numbers(table: &[u8]) -> Option<Vec<u64>> {
    let count = be32(table, 4)? as usize;
    (0..count)
        .map(|i| be32(table, 8 + i * 4).map(u64::from))
        .collect()
}

fn expand(runs: &[(u32, i64)]) -> impl Iterator<Item = i64> + '_ {
    runs.iter()
        .flat_map(|&(count, value)| std::iter::repeat_n(value, count as usize))
}

fn child<'a>(data: &'a [u8], kind: &Kind) -> Option<&'a [u8]> {
    boxes(data).find(|(k, _)| k == kind).map(|(_, body)| body)
}

/// The boxes in `data`, as (kind, body).
fn boxes(mut data: &[u8]) -> impl Iterator<Item = (Kind, &[u8])> {
    std::iter::from_fn(move || {
        let (size, header) = match be32(data, 0)? {
            0 => (data.len(), 8),
            1 => (usize::try_from(be64(data, 8)?).ok()?, 16),
            size => (size as usize, 8),
        };
        if size < header || size > data.len() {
            return None;
        }
        let kind = data[4..8].try_into().ok()?;
        let body = &data[header..size];
        data = &data[size..];
        Some((kind, body))
    })
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn be64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::MediaTools;

    /// Ten seconds of video with B-frames (so an edit list) and keyframes at
    /// uneven times, muxed by FFmpeg.
    fn movie(dir: &Path, name: &str, extra: &[&str]) -> std::path::PathBuf {
        let out = dir.join(name);
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=320x180:rate=24:duration=10"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=10"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-bf", "2"])
            .args(["-g", "1000", "-sc_threshold", "0"])
            .args(["-force_key_frames", "0,1.5,2,4.25,7,7.5,9"])
            .args(["-c:a", "aac"])
            .args(extra)
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        out
    }

    #[tokio::test]
    async fn the_index_gives_the_keyframes_a_full_scan_finds() {
        let dir = tempfile::tempdir().unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        for (name, extra) in [
            ("movie.mp4", &[][..]),
            ("faststart.mp4", &["-movflags", "+faststart"][..]),
            ("movie.mov", &[][..]),
        ] {
            let path = movie(dir.path(), name, extra);
            let indexed = video_keyframes(&path).unwrap().unwrap();
            let scanned = crate::hls::scan_keyframes(&tools, &path, 0, None)
                .await
                .unwrap();
            assert_eq!(indexed, scanned, "{name}");
            assert_eq!(indexed, [0.0, 1.5, 2.0, 4.25, 7.0, 7.5, 9.0], "{name}");
        }
    }

    #[test]
    fn other_files_have_no_index_here() {
        let dir = tempfile::tempdir().unwrap();
        let fragmented = movie(
            dir.path(),
            "fragmented.mp4",
            &["-movflags", "frag_keyframe+empty_moov"],
        );
        assert_eq!(video_keyframes(&fragmented).unwrap(), None);
        let mkv = movie(dir.path(), "movie.mkv", &[]);
        assert_eq!(video_keyframes(&mkv).unwrap(), None);
    }
}
