//! Matroska's own index. A Matroska or WebM file ends (or starts) with Cues:
//! where each keyframe of the video is, and, from mkvmerge and FFmpeg, every
//! subtitle block. Reading them takes a few megabytes; finding the keyframes
//! with ffprobe, or a subtitle track's lines with ffmpeg, reads every packet
//! of the file, which for an 80 GB remux on a network share is up to an hour.
//!
//! Muxers index every video keyframe (mkvmerge and FFmpeg both do), but
//! nothing requires it, and segments cut at a sample of them would not line
//! up with what FFmpeg cuts. So the index is trusted only once the frames
//! at the start of the file agree with it.

use std::collections::HashMap;
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
const BLOCK_DURATION: u32 = 0x9B;
const CODEC_ID: u32 = 0x86;
const SUBTITLE_TRACK: u64 = 0x11;
const CUE_RELATIVE_POSITION: u32 = 0xF0;
const CUE_DURATION: u32 = 0xB2;
/// A subtitle block holds a line or two of text.
const MAX_SUBTITLE_BLOCK: u64 = 64 << 10;

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
    let Some(head) = read_head(file)? else {
        return Ok(None);
    };
    let Some(track) = head.tracks.as_deref().and_then(first_video_track) else {
        return Ok(None);
    };
    let points = cue_points(&head.cues, track);
    if points.is_empty() {
        return Ok(None);
    }
    Ok(Some(Index {
        scale: head.scale,
        track,
        segment: head.segment,
        points,
    }))
}

/// What the start of a Matroska file says about the rest.
struct Head {
    /// Nanoseconds per tick; never 0.
    scale: u64,
    /// Where the segment's data starts: positions in the file are relative to it.
    segment: u64,
    tracks: Option<Vec<u8>>,
    cues: Vec<u8>,
}

/// The segment's settings, track list and index. `None` when the file isn't
/// Matroska or has no index.
fn read_head(file: &mut File) -> io::Result<Option<Head>> {
    let Some((EBML, Some(size), data)) = header_at(file, 0)? else {
        return Ok(None);
    };
    let Some((SEGMENT, segment_size, segment)) = header_at(file, data + size)? else {
        return Ok(None);
    };
    let file_end = file.metadata()?.len();
    let end = segment_size.map_or(file_end, |s| (segment + s).min(file_end));

    let mut scale = 1_000_000;
    let mut tracks = None;
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
                    TRACKS => tracks = Some(body),
                    _ => cues = Some(body),
                }
            }
            _ => {}
        }
        at = data + size;
    }
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
    if scale == 0 {
        return Ok(None);
    }
    Ok(Some(Head {
        scale,
        segment,
        tracks,
        cues,
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

/// One line of a text subtitle track. Times in seconds; the text as muxed
/// (SubRip markup and all).
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleLine {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Every line of SubRip stream `stream` (FFmpeg's stream index), read
/// through the index alone: a few kilobytes per line, where ffmpeg reads the
/// whole file to find them (an hour for an 80 GB remux on a network share).
/// `None` when the file isn't Matroska, the stream isn't `S_TEXT/UTF8`, or
/// the index doesn't say exactly where each of its blocks is.
pub fn text_subtitle_lines(path: &Path, stream: u32) -> io::Result<Option<Vec<SubtitleLine>>> {
    let mut file = File::open(path)?;
    let Some(head) = read_head(&mut file)? else {
        return Ok(None);
    };
    // FFmpeg numbers a Matroska file's streams in track-list order.
    let Some(entry) = head.tracks.as_deref().and_then(|tracks| {
        children(tracks)
            .filter(|(id, _)| *id == TRACK_ENTRY)
            .nth(stream as usize)
            .map(|(_, entry)| entry)
    }) else {
        return Ok(None);
    };
    let field = |wanted: u32| {
        children(entry)
            .find(|(id, _)| *id == wanted)
            .map(|(_, b)| b)
    };
    let srt = field(TRACK_TYPE).map(uint) == Some(SUBTITLE_TRACK)
        && field(CODEC_ID) == Some(b"S_TEXT/UTF8".as_slice());
    let Some(track) = field(TRACK_NUMBER).map(uint).filter(|_| srt) else {
        return Ok(None);
    };
    let Some(blocks) = subtitle_blocks(&head.cues, track) else {
        return Ok(None);
    };
    // Each line is a few small reads somewhere in the file; on a network
    // share the wait for each is what takes the time, so several are read at
    // once (Interstellar's 1,915 lines: 152 s one at a time).
    let per_reader = blocks.len().div_ceil(SUBTITLE_READERS);
    let parts: Vec<io::Result<Option<Vec<SubtitleLine>>>> = std::thread::scope(|scope| {
        let readers: Vec<_> = blocks
            .chunks(per_reader)
            .map(|part| scope.spawn(|| read_subtitle_lines(path, &head, track, part)))
            .collect();
        readers
            .into_iter()
            .map(|reader| {
                reader
                    .join()
                    .unwrap_or_else(|_| Err(io::Error::other("a subtitle reader panicked")))
            })
            .collect()
    });
    let mut lines = Vec::with_capacity(blocks.len());
    for part in parts {
        let Some(part) = part? else {
            return Ok(None);
        };
        lines.extend(part);
    }
    Ok(Some(lines))
}

/// Reads running at once for [`text_subtitle_lines`].
const SUBTITLE_READERS: usize = 16;
/// Enough for a cluster's header and its timestamp, the element after it.
const CLUSTER_HEAD: usize = 32;
/// Enough for a subtitle block in one read, almost always.
const BLOCK_READ: usize = 4096;

/// The lines at `blocks`, from a file of its own: two reads each, the
/// cluster's head (once per cluster) and the block. `None` as for
/// [`text_subtitle_lines`].
fn read_subtitle_lines(
    path: &Path,
    head: &Head,
    track: u64,
    blocks: &[SubtitleBlock],
) -> io::Result<Option<Vec<SubtitleLine>>> {
    let mut file = File::open(path)?;
    let seconds = |ticks: u64| ticks as f64 * head.scale as f64 / 1e9;
    // Each cluster's body start and timestamp.
    let mut clusters: HashMap<u64, (u64, Option<u64>)> = HashMap::new();
    let mut lines = Vec::with_capacity(blocks.len());
    for block in blocks {
        let (body, cluster_time) = match clusters.get(&block.cluster) {
            Some(&known) => known,
            None => {
                let at = head.segment + block.cluster;
                let bytes = read_at(&mut file, at, CLUSTER_HEAD)?;
                let Some((CLUSTER, _, len)) = header(&bytes) else {
                    return Ok(None);
                };
                let time = match header(&bytes[len..]) {
                    Some((CLUSTER_TIMESTAMP, Some(size), inner)) => bytes
                        .get(len + inner..len + inner + size as usize)
                        .filter(|_| size <= 8)
                        .map(uint),
                    _ => None,
                };
                let known = (at + len as u64, time);
                clusters.insert(block.cluster, known);
                known
            }
        };
        let at = body + block.relative;
        let mut bytes = read_at(&mut file, at, BLOCK_READ)?;
        let Some((id, Some(size), len)) = header(&bytes) else {
            return Ok(None);
        };
        if size > MAX_SUBTITLE_BLOCK {
            return Ok(None);
        }
        let end = len + size as usize;
        if bytes.len() < end {
            bytes = read_at(&mut file, at, end)?;
            if bytes.len() < end {
                return Ok(None);
            }
        }
        let element = &bytes[len..end];
        let (payload, duration) = match id {
            SIMPLE_BLOCK => (element, None),
            BLOCK_GROUP => {
                let Some((_, payload)) = children(element).find(|(id, _)| *id == BLOCK) else {
                    return Ok(None);
                };
                let duration = children(element)
                    .find(|(id, _)| *id == BLOCK_DURATION)
                    .map(|(_, d)| uint(d));
                (payload, duration)
            }
            _ => return Ok(None),
        };
        // Laced blocks hold several frames; subtitles never are.
        let Some(header) =
            block_header(payload).filter(|h| h.track == track && h.flags & 0x06 == 0)
        else {
            return Ok(None);
        };
        let Some(duration) = duration.or(block.duration) else {
            return Ok(None);
        };
        let start = match cluster_time {
            Some(base) => base.saturating_add_signed(i64::from(header.time)),
            None => block.time,
        };
        let (_, _, number_len) = vint(payload).expect("block_header read it");
        let text = String::from_utf8_lossy(&payload[number_len + 3..]);
        lines.push(SubtitleLine {
            start: seconds(start),
            end: seconds(start + duration),
            text: text.trim_end_matches('\0').to_owned(),
        });
    }
    Ok(Some(lines))
}

/// Up to `len` bytes from `at`; fewer at the end of the file.
fn read_at(file: &mut File, at: u64, len: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; len];
    file.seek(SeekFrom::Start(at))?;
    let mut read = 0;
    while read < len {
        match file.read(&mut bytes[read..])? {
            0 => break,
            n => read += n,
        }
    }
    bytes.truncate(read);
    Ok(bytes)
}

/// Where one subtitle block is, from the index.
struct SubtitleBlock {
    /// Ticks.
    time: u64,
    /// The block's cluster, from the segment's data.
    cluster: u64,
    /// The block, from the cluster's body.
    relative: u64,
    /// Ticks, when the index says.
    duration: Option<u64>,
}

/// Every indexed block of `track`, in time order. `None` when there are none,
/// or when one has no position within its cluster.
fn subtitle_blocks(cues: &[u8], track: u64) -> Option<Vec<SubtitleBlock>> {
    let mut blocks = Vec::new();
    for (_, point) in children(cues).filter(|(id, _)| *id == CUE_POINT) {
        let Some((_, time)) = children(point).find(|(id, _)| *id == CUE_TIME) else {
            continue;
        };
        for (_, positions) in children(point).filter(|(id, _)| *id == CUE_TRACK_POSITIONS) {
            let field = |wanted: u32| {
                children(positions)
                    .find(|(id, _)| *id == wanted)
                    .map(|(_, b)| uint(b))
            };
            if field(CUE_TRACK) != Some(track) {
                continue;
            }
            blocks.push(SubtitleBlock {
                time: uint(time),
                cluster: field(CUE_CLUSTER_POSITION)?,
                relative: field(CUE_RELATIVE_POSITION)?,
                duration: field(CUE_DURATION),
            });
        }
    }
    blocks.sort_by_key(|b| (b.time, b.cluster, b.relative));
    blocks.dedup_by_key(|b| (b.cluster, b.relative));
    (!blocks.is_empty()).then_some(blocks)
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
        let scanned = crate::hls::scan_keyframes(&tools, &path, 0, None)
            .await
            .unwrap();
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
