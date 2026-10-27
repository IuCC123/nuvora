//! Streaming a file a device can't play as-is: an HLS playlist of fMP4
//! segments, cut by ffmpeg on demand.
//!
//! The playlist is written up front for the whole file, so a player can seek
//! anywhere. ffmpeg then produces segments from wherever the player asks,
//! and is restarted at a segment when a seek jumps past what it has made.
//! That only works when every run cuts at the same times and stamps them the
//! same way, so:
//!
//! - A copied video is cut at every keyframe of the source (the only places
//!   a copy can start), found once with ffprobe. A converted one gets a
//!   keyframe forced every few seconds ([`SEGMENT_SECONDS`] by default) and
//!   none elsewhere.
//! - Every run keeps the source's timestamps (`-copyts`, `frag_discont`), so
//!   a segment made by any run sits at the same place on the timeline.
//! - A run that copies video starts on a keyframe the demuxer can seek to;
//!   [`landing`] asks ffmpeg which one before the run starts, and the run's
//!   first segment is numbered after it.
//!
//! Times here are the source's presentation timestamps in seconds. The
//! playlist's timeline starts at the first keyframe, so player time is
//! source time minus [`Segments::origin`].

use std::ffi::OsString;
use std::fmt::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::{
    AudioOutput, EncodeOptions, MediaInfo, MediaTools, StreamPlan, VideoCodec, VideoEncoder,
    VideoPlan,
};

/// Default length of a converted segment.
pub const SEGMENT_SECONDS: f64 = 6.0;
/// The init segment every run writes, before its first media segment.
pub const INIT_SEGMENT: &str = "init.mp4";

/// Where each segment starts, in source time, plus where the last one ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Segments {
    starts: Vec<f64>,
    end: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum HlsError {
    #[error("{tool} timed out after {after:?}")]
    Timeout { tool: &'static str, after: Duration },
    #[error("{tool} failed: {stderr}")]
    Failed { tool: &'static str, stderr: String },
    #[error("the file has no keyframes ffprobe could read")]
    NoKeyframes,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Segments {
    /// One segment per keyframe interval: what a copied video can be cut at.
    /// `keyframes` sorted ascending; `None` when empty.
    pub fn at_keyframes(keyframes: &[f64], duration: f64) -> Option<Segments> {
        let first = *keyframes.first()?;
        let mut starts: Vec<f64> = Vec::with_capacity(keyframes.len());
        for &k in keyframes {
            // Two keyframes on the same millisecond would make an empty segment.
            if starts.last().is_none_or(|&last| k - last >= 0.001) {
                starts.push(k);
            }
        }
        let end = (first + duration).max(starts[starts.len() - 1] + 0.001);
        Some(Segments { starts, end })
    }

    /// A segment every `seconds` from `origin`: what a converted video is
    /// cut at.
    pub fn every(origin: f64, duration: f64, seconds: f64) -> Segments {
        let count = ((duration / seconds).ceil() as usize).max(1);
        Segments {
            starts: (0..count).map(|i| origin + i as f64 * seconds).collect(),
            end: origin + duration.max(0.001),
        }
    }

    pub fn len(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    /// Source time of the first frame: player time 0.
    pub fn origin(&self) -> f64 {
        self.starts[0]
    }

    /// Source time segment `index` starts at.
    pub fn start(&self, index: usize) -> f64 {
        self.starts[index]
    }

    /// Source time segment `index` ends at.
    pub fn end_of(&self, index: usize) -> f64 {
        self.starts.get(index + 1).copied().unwrap_or(self.end)
    }

    /// The segment that holds player time `seconds`.
    pub fn index_at(&self, seconds: f64) -> usize {
        let at = self.origin() + seconds.max(0.0);
        self.starts.partition_point(|&s| s <= at).saturating_sub(1)
    }

    /// The segment starting at source time `at` (within a millisecond), else
    /// the last one starting before it.
    pub fn index_of(&self, at: f64) -> usize {
        self.starts
            .partition_point(|&s| s <= at + 0.001)
            .saturating_sub(1)
    }

    /// A VOD media playlist over every segment. `init` and `segment(i)` are
    /// the URIs to write; `start` (player seconds) adds `EXT-X-START`, which
    /// players without a start position of their own honour.
    pub fn playlist(
        &self,
        init: &str,
        segment: impl Fn(usize) -> String,
        start: Option<f64>,
    ) -> String {
        let longest = (0..self.len())
            .map(|i| self.end_of(i) - self.start(i))
            .fold(0.0_f64, f64::max);
        let mut out = String::with_capacity(64 + self.len() * 40);
        out.push_str(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-INDEPENDENT-SEGMENTS\n",
        );
        let _ = writeln!(
            out,
            "#EXT-X-TARGETDURATION:{}",
            longest.ceil().max(1.0) as u64
        );
        out.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        if let Some(start) = start.filter(|s| *s > 0.0) {
            let _ = writeln!(out, "#EXT-X-START:TIME-OFFSET={start:.3},PRECISE=YES");
        }
        let _ = writeln!(out, "#EXT-X-MAP:URI=\"{init}\"");
        for i in 0..self.len() {
            let _ = writeln!(
                out,
                "#EXTINF:{:.6},\n{}",
                self.end_of(i) - self.start(i),
                segment(i)
            );
        }
        out.push_str("#EXT-X-ENDLIST\n");
        out
    }
}

/// Source times of every keyframe of stream `video`, ascending. Reads the
/// whole file's packet headers (no decoding): a fraction of a second for an
/// episode on a local disk.
pub async fn keyframes(tools: &MediaTools, path: &Path, video: u32) -> Result<Vec<f64>, HlsError> {
    let mut command = Command::new(&tools.ffprobe);
    command
        .args(["-v", "error", "-select_streams"])
        .arg(video.to_string())
        .args(["-show_entries", "packet=pts_time,flags", "-of", "csv=p=0"])
        .arg(path);
    let output = run("ffprobe", command, tools.probe_timeout * 4).await?;
    let mut times: Vec<f64> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (pts, flags) = line.split_once(',')?;
            flags
                .contains('K')
                .then(|| pts.parse::<f64>().ok())
                .flatten()
        })
        .collect();
    times.sort_by(f64::total_cmp);
    if times.is_empty() {
        return Err(HlsError::NoKeyframes);
    }
    Ok(times)
}

/// The first video packet's time when ffmpeg seeks to `at` the way a run
/// does: the keyframe a copying run will start on. `at` must be past the
/// wanted keyframe by more than ffmpeg's 3/23 s seek backoff. At 0 there is
/// no seek, as a run from the start has none: seeking an AVI to 0 lands past
/// its first keyframe.
pub async fn landing(
    tools: &MediaTools,
    path: &Path,
    video: u32,
    at: f64,
) -> Result<Option<f64>, HlsError> {
    let mut command = Command::new(&tools.ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(["-copyts", "-seek_timestamp", "1", "-noaccurate_seek"]);
    if at > 0.0 {
        command.arg("-ss").arg(format!("{at:.3}"));
    }
    command
        .arg("-i")
        .arg(path)
        .arg("-map")
        .arg(format!("0:{video}"))
        .args(["-c", "copy", "-frames:v", "1", "-f", "framecrc", "-"]);
    let output = run("ffmpeg", command, tools.probe_timeout).await?;
    Ok(parse_framecrc(&String::from_utf8_lossy(&output.stdout)))
}

/// `#tb 0: 1/1000` then `0, dts, pts, duration, size, crc`: the pts in
/// seconds, or the dts for a packet without a pts (XviD in AVI has none).
fn parse_framecrc(text: &str) -> Option<f64> {
    let (num, den) = text.lines().find_map(|l| {
        let (n, d) = l.strip_prefix("#tb 0:")?.trim().split_once('/')?;
        Some((n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?))
    })?;
    let line = text
        .lines()
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())?;
    // FFmpeg's AV_NOPTS_VALUE: no time at all, not a time.
    let time = |field: &str| field.trim().parse::<i64>().ok().filter(|&t| t != i64::MIN);
    let mut fields = line.split(',').skip(1);
    let dts = fields.next().and_then(time);
    let pts = fields.next().and_then(time);
    Some(pts.or(dts)? as f64 * num / den)
}

/// One ffmpeg run of a stream.
pub struct Run<'a> {
    pub path: &'a Path,
    pub media: &'a MediaInfo,
    pub plan: &'a StreamPlan,
    pub segments: &'a Segments,
    /// The number of the first segment this run writes.
    pub first: usize,
    /// Where to seek, in source time (`None`: from the beginning). For a
    /// copying run, a time [`landing`] confirmed lands on segment `first`.
    pub seek: Option<f64>,
    /// Segments and the init segment are written here.
    pub dir: &'a Path,
    /// For converted video.
    pub encoder: &'a VideoEncoder,
    /// For converted video and audio, and the log level.
    pub options: &'a EncodeOptions,
}

impl Run<'_> {
    /// The arguments after the ffmpeg binary. Every segment is written to a
    /// temporary name and renamed when complete, so a file named
    /// `{n}.m4s` is always whole. FFmpeg reports progress on stdout
    /// (`key=value` lines, see [`RunProgress`]).
    pub fn args(&self) -> Vec<OsString> {
        let mut a: Vec<OsString> = Vec::new();
        let mut push = |items: &[&str]| a.extend(items.iter().map(OsString::from));
        push(&[
            "-hide_banner",
            "-loglevel",
            self.options.log_level,
            "-nostats",
            "-progress",
            "pipe:1",
            "-stats_period",
            "1",
            "-nostdin",
            "-y",
            "-copyts",
            "-seek_timestamp",
            "1",
        ]);
        let copy = self.plan.video == VideoPlan::Copy;
        if let Some(seek) = self.seek {
            if copy {
                push(&["-noaccurate_seek"]);
            }
            push(&["-ss", &format!("{seek:.3}")]);
        }
        if self.plan.video != VideoPlan::Copy {
            a.extend(self.encoder.input_args(self.options));
        }
        a.push("-i".into());
        a.push(self.path.as_os_str().to_owned());
        let mut push = |items: &[&str]| a.extend(items.iter().map(OsString::from));
        let video = self.media.video.first();
        push(&["-map", &format!("0:{}", video.map_or(0, |v| v.index))]);
        if let Some(audio) = self.plan.audio {
            push(&["-map", &format!("0:{}", audio.stream)]);
        }
        push(&["-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1"]);
        let mut converted = Vec::new();
        match self.plan.video {
            VideoPlan::Copy => {
                push(&["-c:v", "copy"]);
                if video.is_some_and(|v| v.codec == VideoCodec::Hevc) {
                    // Apple players only take HEVC tagged `hvc1`.
                    push(&["-tag:v", "hvc1"]);
                }
            }
            VideoPlan::H264 { max_height } => {
                // Keyframes only where segments start: the muxer cuts at every one.
                let forced: Vec<String> = (self.first..self.segments.len())
                    .map(|i| format!("{:.3}", self.segments.start(i)))
                    .collect();
                converted = self
                    .encoder
                    .output_args(max_height, &forced.join(","), self.options);
            }
        }
        a.extend(converted);
        let mut push = |items: &[&str]| a.extend(items.iter().map(OsString::from));
        match self.plan.audio.map(|a| a.output) {
            None => {}
            Some(AudioOutput::Copy) => push(&["-c:a", "copy"]),
            Some(AudioOutput::Aac { channels }) => {
                let options = self.options;
                let (channels, bitrate) = options.aac(channels);
                // Nothing may start before zero: ExoPlayer rejects the
                // stream. Sources often have audio a few ms early (an
                // encoder's delay, rounded to MKV's milliseconds), so that
                // goes; and the AAC encoder's 1024-sample lead-in would
                // stamp the first frame early, so audio moves 21 ms later,
                // inaudible and the same in every run.
                let mut filters = "atrim=start=0,asetpts=PTS+1024/SR/TB".to_owned();
                if options.night_mode {
                    // About -18 dB, 4:1, with +6 dB back: loud scenes come
                    // down, speech comes up.
                    filters.push_str(
                        ",acompressor=threshold=0.125:ratio=4:attack=5:release=250:makeup=2",
                    );
                }
                push(&[
                    "-c:a",
                    "aac",
                    "-ac",
                    &channels.to_string(),
                    "-b:a",
                    &format!("{bitrate}k"),
                    "-af",
                    &filters,
                ]);
            }
        }
        push(&[
            "-avoid_negative_ts",
            "disabled",
            "-max_muxing_queue_size",
            "4096",
            "-f",
            "hls",
            "-hls_time",
            "0.01",
            "-hls_segment_type",
            "fmp4",
            "-hls_fmp4_init_filename",
            INIT_SEGMENT,
            // Each fragment keeps its source timestamps instead of starting at 0.
            "-hls_segment_options",
            "movflags=+frag_discont",
            "-hls_flags",
            "temp_file",
            "-hls_list_size",
            "0",
            "-start_number",
            &self.first.to_string(),
            "-hls_segment_filename",
        ]);
        a.push(self.dir.join("%d.m4s").into_os_string());
        a.push(self.dir.join("ffmpeg.m3u8").into_os_string());
        a
    }
}

/// What a run's FFmpeg says about itself on stdout (`-progress`), once a
/// second: a block of `key=value` lines ending in `progress=continue` (or
/// `end`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunProgress {
    /// Video frames written.
    pub frame: u64,
    pub fps: f64,
    /// The output's bitrate so far, kbit/s.
    pub bitrate_kbps: Option<f64>,
    /// Where the output has got to, in source seconds (runs keep source time).
    pub out_time: Option<f64>,
    /// Media seconds made per second of wall time.
    pub speed: Option<f64>,
    /// FFmpeg reached the end of the file.
    pub ended: bool,
}

impl RunProgress {
    /// Reads one line into `self`. True when it ends a block: `self` then
    /// holds a whole report.
    pub fn read_line(&mut self, line: &str) -> bool {
        let Some((key, value)) = line.trim().split_once('=') else {
            return false;
        };
        let number = |v: &str| v.trim().parse::<f64>().ok().filter(|n| n.is_finite());
        match key {
            "frame" => self.frame = value.trim().parse().unwrap_or(self.frame),
            "fps" => self.fps = number(value).unwrap_or(0.0),
            "bitrate" => self.bitrate_kbps = number(value.trim_end_matches("kbits/s")),
            "out_time_us" => {
                self.out_time = number(value).filter(|t| *t >= 0.0).map(|us| us / 1e6);
            }
            "speed" => self.speed = number(value.trim_end_matches('x')),
            "progress" => {
                self.ended = value.trim() == "end";
                return true;
            }
            _ => {}
        }
        false
    }
}

async fn run(
    tool: &'static str,
    mut command: Command,
    timeout: Duration,
) -> Result<std::process::Output, HlsError> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| HlsError::Timeout {
            tool,
            after: timeout,
        })??;
    if !output.status.success() {
        return Err(HlsError::Failed {
            tool,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyframe_segments_cover_the_file() {
        let s = Segments::at_keyframes(&[0.0, 1.001, 1.001, 4.004, 10.5], 24.0).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s.index_at(0.0), 0);
        assert_eq!(s.index_at(4.0), 1);
        assert_eq!(s.index_at(4.004), 2);
        assert_eq!(s.index_at(99.0), 3);
        assert_eq!(s.index_of(4.0045), 2);
        let playlist = s.playlist("init.mp4", |i| format!("{i}.m4s"), None);
        assert!(
            playlist.contains("#EXT-X-TARGETDURATION:14\n"),
            "{playlist}"
        );
        assert!(
            playlist.contains("#EXTINF:13.500000,\n3.m4s\n#EXT-X-ENDLIST"),
            "{playlist}"
        );
        assert!(!playlist.contains("EXT-X-START"));
    }

    #[test]
    fn converted_segments_are_evenly_spaced_from_the_first_frame() {
        let s = Segments::every(1.4, 20.0, SEGMENT_SECONDS);
        assert_eq!(s.len(), 4);
        assert_eq!(s.start(3), 1.4 + 18.0);
        assert_eq!(s.index_at(6.0), 1);
        let playlist = s.playlist("init.mp4?x=1", |i| format!("{i}.m4s?x=1"), Some(61.5));
        assert!(playlist.contains("#EXT-X-START:TIME-OFFSET=61.500,PRECISE=YES\n"));
        assert!(playlist.contains("#EXT-X-MAP:URI=\"init.mp4?x=1\"\n"));
        assert!(playlist.contains("#EXTINF:2.000000,\n3.m4s?x=1\n"));
    }

    #[test]
    fn progress_blocks_read_into_a_report() {
        let mut progress = RunProgress::default();
        let block = "frame=240\nfps=48.10\nstream_0_0_q=-1.0\nbitrate=N/A\ntotal_size=1024\nout_time_us=10010000\nout_time=00:00:10.010000\nspeed=2.05x\n";
        for line in block.lines() {
            assert!(!progress.read_line(line));
        }
        assert!(progress.read_line("progress=continue"));
        assert_eq!(
            progress,
            RunProgress {
                frame: 240,
                fps: 48.1,
                bitrate_kbps: None,
                out_time: Some(10.01),
                speed: Some(2.05),
                ended: false,
            }
        );
        progress.read_line("bitrate=5120.3kbits/s");
        progress.read_line("speed=N/A");
        progress.read_line("out_time_us=-9223372036854775807");
        assert!(progress.read_line("progress=end"));
        assert_eq!(progress.bitrate_kbps, Some(5120.3));
        assert_eq!((progress.speed, progress.out_time), (None, None));
        assert!(progress.ended);
    }

    #[test]
    fn framecrc_gives_the_first_packet_time() {
        let text = "#software: Lavf\n#tb 0: 1/1000\n#media_type 0: video\n0,      293419,      293502,       41,   282486, 0x41c0a477\n";
        assert_eq!(parse_framecrc(text), Some(293.502));
        assert_eq!(
            parse_framecrc("#tb 0: 1/90000\n0, 0, 126000, 3003, 5, 0x0\n"),
            Some(1.4)
        );
        assert_eq!(parse_framecrc(""), None);
    }

    #[test]
    fn framecrc_without_a_pts_gives_the_dts() {
        // Oppenheimer.2023.D.BDRip.avi (XviD), seeked to 600.2 s.
        let text = "#tb 0: 104271/2500000\n#media_type 0: video\n0,      14373, -9223372036854775808,        1,    51247, 0x9ecdf930\n";
        let at = parse_framecrc(text).unwrap();
        assert!((at - 599.475).abs() < 0.001, "{at}");
        assert_eq!(
            parse_framecrc(
                "#tb 0: 1/25\n0, -9223372036854775808, -9223372036854775808, 1, 5, 0x0\n"
            ),
            None
        );
    }
}
