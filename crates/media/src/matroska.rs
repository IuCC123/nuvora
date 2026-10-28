//! Matroska's own index. A Matroska or WebM file ends (or starts) with Cues:
//! where each keyframe of the video is. Reading them takes a few megabytes;
//! finding the keyframes with ffprobe reads every packet of the file, which
//! for an 80 GB remux on a network share is ten minutes.
//!
//! Muxers index every video keyframe (mkvmerge and FFmpeg both do), but
//! nothing requires it, and segments cut at a sample of them would not line
//! up with what FFmpeg cuts. So the index is trusted only once the frames
//! at the start of the file agree with it.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const VIDEO_TRACK: u64 = 1;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CUE_CLUSTER_POSITION: u32 = 0xF1;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const REFERENCE_BLOCK: u32 = 0xFB;

/// The head elements (track list, settings) are small; anything bigger is
/// not a file this understands.
const MAX_HEAD: u64 = 16 << 20;
/// An hour of keyframes indexes in about 1 MB.
const MAX_CUES: u64 = 64 << 20;
/// How much of the file's start is read to check the index against.
const SAMPLE: u64 = 32 << 20;

/// Source times of every keyframe of the first video track, ascending, from
/// the file's index. `None` when the file isn't Matroska, has no index for
/// its video, or its index leaves out keyframes.
pub fn video_keyframes(path: &Path) -> io::Result<Option<Vec<f64>>> {
    let mut file = File::open(path)?;
    let Some(index) = read_index(&mut file)? else {
        return Ok(None);
    };
    if !index_is_complete(&mut file, &index)? {
        return Ok(None);
    }
    let scale = index.scale as f64;
    Ok(Some(
        index
            .points
            .iter()
            .map(|p| p.time as f64 * scale / 1e9)
            .collect(),
    ))
}

struct Index {
    /// Nanoseconds per tick.
    scale: u64,
    track: u64,
    /// Where the segment's data starts: positions in the file are relative to it.
    segment: u64,
    /// Ascending by time, one per time.
    points: Vec<CuePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CuePoint {
    /// In ticks of `Index::scale`.
    time: u64,
    /// The keyframe's cluster, from `Index::segment`.
    cluster: u64,
}

fn read_index(file: &mut File) -> io::Result<Option<Index>> {
    let Some((EBML, Some(size), data)) = header_at(file, 0)? else {
        return Ok(None);
    };
    let Some((SEGMENT, segment_size, segment)) = header_at(file, data + size)? else {
        return Ok(None);
    };
    let file_end = file.metadata()?.len();
    let end = segment_size.map_or(file_end, |s| (segment + s).min(file_end));

    let mut scale = 1_000_000;
    let mut track = None;
    let mut cues_at = None;
    let mut cues = None;
    let mut at = segment;
    // The head elements come before the first cluster; the index is either
    // among them or where the seek head says.
    while at < end {
        let Some((id, Some(size), data)) = header_at(file, at)? else {
            break;
        };
        match id {
            CLUSTER => break,
            SEEK_HEAD | INFO | TRACKS | CUES => {
                let limit = if id == CUES { MAX_CUES } else { MAX_HEAD };
                let Some(body) = read_body(file, data, size, limit)? else {
                    return Ok(None);
                };
                match id {
                    SEEK_HEAD => cues_at = cues_at.or(seek_position(&body, CUES)),
                    INFO => {
                        if let Some((_, s)) = children(&body).find(|(id, _)| *id == TIMESTAMP_SCALE)
                        {
                            scale = uint(s);
                        }
                    }
                    TRACKS => track = first_video_track(&body),
                    _ => cues = Some(body),
                }
            }
            _ => {}
        }
        at = data + size;
    }
    let Some(track) = track else {
        return Ok(None);
    };
    let cues = match (cues, cues_at) {
        (Some(cues), _) => cues,
        (None, Some(position)) => match header_at(file, segment + position)? {
            Some((CUES, Some(size), data)) => match read_body(file, data, size, MAX_CUES)? {
                Some(body) => body,
                None => return Ok(None),
            },
            _ => return Ok(None),
        },
        (None, None) => return Ok(None),
    };
    let points = cue_points(&cues, track);
    if points.is_empty() || scale == 0 {
        return Ok(None);
    }
    Ok(Some(Index {
        scale,
        track,
        segment,
        points,
    }))
}

/// Where the seek head says element `wanted` is, from the segment's data.
fn seek_position(seek_head: &[u8], wanted: u32) -> Option<u64> {
    children(seek_head)
        .filter(|(id, _)| *id == SEEK)
        .find_map(|(_, seek)| {
            let id = children(seek).find(|(id, _)| *id == SEEK_ID)?.1;
            let position = children(seek).find(|(id, _)| *id == SEEK_POSITION)?.1;
            (uint(id) == u64::from(wanted)).then(|| uint(position))
        })
}

/// The number of the first video track: the first video stream FFmpeg reads.
fn first_video_track(tracks: &[u8]) -> Option<u64> {
    children(tracks)
        .filter(|(id, _)| *id == TRACK_ENTRY)
        .find_map(|(_, entry)| {
            let kind = children(entry).find(|(id, _)| *id == TRACK_TYPE)?.1;
            let number = children(entry).find(|(id, _)| *id == TRACK_NUMBER)?.1;
            (uint(kind) == VIDEO_TRACK).then(|| uint(number))
        })
}

/// The cue points for `track`, ascending by time, one per time.
fn cue_points(cues: &[u8], track: u64) -> Vec<CuePoint> {
    let mut points: Vec<CuePoint> = children(cues)
        .filter(|(id, _)| *id == CUE_POINT)
        .filter_map(|(_, point)| {
            let time = uint(children(point).find(|(id, _)| *id == CUE_TIME)?.1);
            let cluster = children(point)
                .filter(|(id, _)| *id == CUE_TRACK_POSITIONS)
                .find_map(|(_, positions)| {
                    let on = children(positions).find(|(id, _)| *id == CUE_TRACK)?.1;
                    let cluster = children(positions)
                        .find(|(id, _)| *id == CUE_CLUSTER_POSITION)?
                        .1;
                    (uint(on) == track).then(|| uint(cluster))
                })?;
            Some(CuePoint { time, cluster })
        })
        .collect();
    points.sort_by_key(|p| p.time);
    points.dedup_by_key(|p| p.time);
    points
}

/// Whether the keyframes at the start of the file are exactly the indexed
/// ones. Reads the clusters from the first cue point on, up to [`SAMPLE`]
/// bytes; at least one keyframe past the first must turn up.
fn index_is_complete(file: &mut File, index: &Index) -> io::Result<bool> {
    let mut keyframes = Vec::new();
    let mut at = index.segment + index.points[0].cluster;
    let mut budget = SAMPLE;
    let enough = index.points.get(8).map_or(u64::MAX, |p| p.time);
    while budget > 0 && keyframes.last().is_none_or(|&k| k < enough) {
        let Some((id, Some(size), data)) = header_at(file, at)? else {
            break;
        };
        if id == CLUSTER {
            let take = size.min(budget);
            let body = read_body(file, data, take, take)?.unwrap_or_default();
            budget -= take;
            cluster_keyframes(&body, index.track, &mut keyframes);
        }
        at = data + size;
    }
    // Keyframes come in time order, so every one up to the last seen has been seen.
    let Some(&last) = keyframes.last() else {
        return Ok(false);
    };
    let first = index.points[0].time;
    let indexed: Vec<u64> = index
        .points
        .iter()
        .map(|p| p.time)
        .take_while(|&t| t <= last)
        .collect();
    let seen: Vec<u64> = keyframes.into_iter().filter(|&k| k >= first).collect();
    Ok(indexed.len() >= 2 && seen == indexed)
}

/// Appends the times of `track`'s keyframes in a cluster's body, in file order.
fn cluster_keyframes(cluster: &[u8], track: u64, out: &mut Vec<u64>) {
    let mut base = 0;
    for (id, body) in children(cluster) {
        let block = match id {
            CLUSTER_TIMESTAMP => {
                base = uint(body);
                continue;
            }
            SIMPLE_BLOCK => block_header(body).filter(|b| b.flags & 0x80 != 0),
            // A block that refers to no other is a keyframe.
            BLOCK_GROUP if !children(body).any(|(id, _)| id == REFERENCE_BLOCK) => children(body)
                .find(|(id, _)| *id == BLOCK)
                .and_then(|(_, b)| block_header(b)),
            _ => None,
        };
        if let Some(block) = block.filter(|b| b.track == track) {
            out.push(base.saturating_add_signed(i64::from(block.time)));
        }
    }
}

struct BlockHeader {
    track: u64,
    /// Ticks from the cluster's timestamp.
    time: i16,
    flags: u8,
}

fn block_header(block: &[u8]) -> Option<BlockHeader> {
    let (_, track, len) = vint(block)?;
    let rest = block.get(len..len + 3)?;
    Some(BlockHeader {
        track,
        time: i16::from_be_bytes([rest[0], rest[1]]),
        flags: rest[2],
    })
}

/// The element at `at`: its ID, its body's size (`None`: unknown) and where
/// its body starts. `None` when there is no whole header there.
fn header_at(file: &mut File, at: u64) -> io::Result<Option<(u32, Option<u64>, u64)>> {
    let mut buf = [0; 12];
    file.seek(SeekFrom::Start(at))?;
    let mut read = 0;
    while read < buf.len() {
        match file.read(&mut buf[read..])? {
            0 => break,
            n => read += n,
        }
    }
    Ok(header(&buf[..read]).map(|(id, size, len)| (id, size, at + len as u64)))
}

/// `size` bytes from `at`. `None` when that is more than `limit` or past
/// the end of the file.
fn read_body(file: &mut File, at: u64, size: u64, limit: u64) -> io::Result<Option<Vec<u8>>> {
    if size > limit {
        return Ok(None);
    }
    let mut body = vec![0; size as usize];
    file.seek(SeekFrom::Start(at))?;
    match file.read_exact(&mut body) {
        Ok(()) => Ok(Some(body)),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

/// An element header at the start of `data`: ID, body size (`None`:
/// unknown) and the header's length.
fn header(data: &[u8]) -> Option<(u32, Option<u64>, usize)> {
    let (id, _, id_len) = vint(data)?;
    let (_, size, size_len) = vint(data.get(id_len..)?)?;
    let unknown = size == (1 << (7 * size_len)) - 1;
    Some((
        u32::try_from(id).ok()?,
        (!unknown).then_some(size),
        id_len + size_len,
    ))
}

/// An EBML variable-length integer at the start of `data`: as written (an
/// ID keeps its length marker), its value without the marker (a size), and
/// its length.
fn vint(data: &[u8]) -> Option<(u64, u64, usize)> {
    let len = data.first()?.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let raw = data
        .get(..len)?
        .iter()
        .fold(0, |v, &b| (v << 8) | u64::from(b));
    Some((raw, raw & ((1 << (7 * len)) - 1), len))
}

/// The elements in `data`, in order, up to the first that doesn't fit.
/// Unknown sizes run to the end.
fn children(mut data: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    std::iter::from_fn(move || {
        let (id, size, len) = header(data)?;
        let rest = &data[len..];
        let size = match size {
            Some(size) => usize::try_from(size).ok().filter(|&s| s <= rest.len())?,
            None => rest.len(),
        };
        let (body, after) = rest.split_at(size);
        data = after;
        Some((id, body))
    })
}

fn uint(body: &[u8]) -> u64 {
    body.iter().take(8).fold(0, |v, &b| (v << 8) | u64::from(b))
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::MediaTools;

    /// Ten seconds of video with keyframes at uneven times, muxed by FFmpeg.
    fn movie(dir: &Path, container: &str) -> std::path::PathBuf {
        let out = dir.join(format!("movie.{container}"));
        let status = Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=320x180:rate=24:duration=10"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=10"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-bf", "2"])
            .args(["-g", "1000", "-sc_threshold", "0"])
            .args(["-force_key_frames", "0,1.5,2,4.25,7,7.5,9"])
            .args(["-c:a", "aac"])
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        out
    }

    #[tokio::test]
    async fn the_index_gives_the_keyframes_a_full_scan_finds() {
        let dir = tempfile::tempdir().unwrap();
        let path = movie(dir.path(), "mkv");
        let indexed = video_keyframes(&path).unwrap().unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        let scanned = crate::hls::scan_keyframes(&tools, &path, 0).await.unwrap();
        assert_eq!(indexed, scanned);
        assert_eq!(indexed, [0.0, 1.5, 2.0, 4.25, 7.0, 7.5, 9.0]);
    }

    #[test]
    fn an_index_that_leaves_out_a_keyframe_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let path = movie(dir.path(), "mkv");
        let mut file = File::open(&path).unwrap();
        let mut index = read_index(&mut file).unwrap().unwrap();
        assert!(index_is_complete(&mut file, &index).unwrap());
        index.points.remove(2);
        assert!(!index_is_complete(&mut file, &index).unwrap());
    }

    #[test]
    fn other_containers_have_no_index_here() {
        let dir = tempfile::tempdir().unwrap();
        let path = movie(dir.path(), "mp4");
        assert_eq!(video_keyframes(&path).unwrap(), None);
    }
}
