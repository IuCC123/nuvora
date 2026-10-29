//! Streaming copies: a 1080p H.264 version of every video too heavy to stream
//! as it is (4K, or a bitrate past [`HEAVY_BITRATE`]), made in the background
//! and kept on the server's own disk.
//!
//! A 4K remux runs at 50 to 80 Mbit/s: more than most home connections carry,
//! and often more than a network share can read, so it stalls however the
//! player buffers. Converting it while it plays needs a graphics chip; on a
//! processor, decoding 4K HEVC alone runs slower than real time. A copy made
//! once, ahead of time, plays smoothly anywhere at a tenth of the bitrate, and
//! the original stays one click away in the player.
//!
//! A copy is the same HLS a converting stream makes (see `nuvora_media::hls`),
//! written once and kept: `<data>/copies/<fingerprint>-v<COPY_VERSION>/` holds
//! `copy.json` (the timeline), `init.mp4` and `{n}.m4s`, and `done` once every
//! segment is in. Segments line up across runs, so a run cut short (a restart,
//! an update) carries on from the first missing segment. One copy is made at a
//! time, at the lowest priority, and paused while anyone streams through the
//! server (a conversion needs the processor, a remux the disk). A video someone opens goes to the front.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nuvora_library::{FileId, Library, StreamSource};
use nuvora_media::{
    AudioOutput, AudioPlan, EncodeOptions, HdrFormat, INIT_SEGMENT, MediaTools, Run, RunProgress,
    Segments, StreamPlan, VideoPlan,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::transcode::Transcoder;

/// Bump when copies change shape: older ones are deleted and made again.
const COPY_VERSION: u32 = 1;
/// Copies are never taller than this.
pub const COPY_HEIGHT: u32 = 1080;
/// Videos above this many bit/s get a copy, whatever their size.
pub const HEAVY_BITRATE: u64 = 40_000_000;
const TIMELINE_FILE: &str = "copy.json";
const DONE_FILE: &str = "done";
/// What a copy is expected to take per second of video, to check the disk
/// before starting: 6 Mbit/s, a busy film's 1080p copy.
const BYTES_PER_SECOND: u64 = 750_000;
/// Disk left free for everything else.
const DISK_RESERVE: u64 = 3_000_000_000;

/// Where a video's copy stands.
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CopyState {
    /// Light enough to stream as it is: no copy is made.
    NotNeeded,
    /// Copies are switched off in the admin panel.
    Off,
    /// Waiting for the videos ahead of it.
    Queued {
        position: u32,
    },
    Making {
        /// 0 to 1.
        progress: f64,
        /// Seconds of video made per second; `None` until measured.
        speed: Option<f64>,
        /// Seconds until done at the current speed.
        eta_s: Option<u64>,
        /// Held while someone streams through the server.
        paused: bool,
    },
    Ready {
        height: u32,
        /// Average bit/s over the whole copy.
        bitrate: u64,
    },
    Failed {
        reason: String,
    },
}

/// Why making a copy stopped short.
enum Stop {
    Failed(String),
    /// Not enough disk: tried again a minute later, as space may free up.
    NoSpace(String),
    /// Someone opened another heavy video, or copies were switched off: the
    /// run ends and carries on later from where it got to.
    Yield,
}

impl From<String> for Stop {
    fn from(reason: String) -> Self {
        Stop::Failed(reason)
    }
}

impl From<&str> for Stop {
    fn from(reason: &str) -> Self {
        Stop::Failed(reason.to_owned())
    }
}

/// The timeline a copy was cut on, kept so every run cuts it the same way.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Timeline {
    origin: f64,
    duration: f64,
    segment_seconds: f64,
    height: u32,
}

impl Timeline {
    fn segments(&self) -> Segments {
        Segments::every(self.origin, self.duration, self.segment_seconds)
    }
}

pub struct Copies {
    root: PathBuf,
    tools: MediaTools,
    library: Library,
    transcoder: Arc<Transcoder>,
    /// FFmpeg has `zscale`, so HDR can be tone mapped.
    tonemap: bool,
    states: Mutex<HashMap<FileId, CopyState>>,
    /// Files someone opened, most recent first: made before the others.
    urgent: Mutex<Vec<FileId>>,
    wake: Notify,
}

impl Copies {
    pub async fn start(
        tools: MediaTools,
        library: Library,
        transcoder: Arc<Transcoder>,
        data_dir: &Path,
    ) -> Arc<Copies> {
        let root = data_dir.join("copies");
        if let Err(e) = tokio::fs::create_dir_all(&root).await {
            tracing::warn!("can't create {}: {e}", root.display());
        }
        let tonemap = has_zscale(&tools).await;
        if !tonemap {
            tracing::info!("FFmpeg has no zscale: HDR videos get no streaming copy");
        }
        let copies = Arc::new(Copies {
            root,
            tools,
            library,
            transcoder,
            tonemap,
            states: Mutex::new(HashMap::new()),
            urgent: Mutex::new(Vec::new()),
            wake: Notify::new(),
        });
        tokio::spawn(work(copies.clone()));
        copies
    }

    /// Where `file`'s copy stands. Asking moves it to the front of the queue.
    pub fn state(&self, file: FileId) -> CopyState {
        let state = self.states.lock().unwrap().get(&file).cloned();
        match state {
            None => CopyState::NotNeeded,
            Some(state) => {
                if matches!(state, CopyState::Queued { .. }) {
                    let mut urgent = self.urgent.lock().unwrap();
                    urgent.retain(|f| *f != file);
                    urgent.insert(0, file);
                    self.wake.notify_one();
                }
                state
            }
        }
    }

    /// Every copy in the works or waiting, for the Activity page.
    pub fn activity(&self) -> Vec<(FileId, CopyState)> {
        let mut out: Vec<(FileId, CopyState)> = self
            .states
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| !matches!(s, CopyState::Ready { .. } | CopyState::Off))
            .map(|(f, s)| (*f, s.clone()))
            .collect();
        out.sort_by_key(|(f, s)| match s {
            CopyState::Making { .. } => (0, 0, *f),
            CopyState::Queued { position } => (1, *position, *f),
            _ => (2, 0, *f),
        });
        out
    }

    /// The folder of `source`'s finished copy.
    pub fn ready_dir(&self, source: &StreamSource) -> Option<PathBuf> {
        let dir = self.dir(source);
        dir.join(DONE_FILE).is_file().then_some(dir)
    }

    /// The playlist of a finished copy in `dir`.
    pub fn playlist(dir: &Path, start: Option<f64>) -> Option<String> {
        let timeline = read_timeline(dir)?;
        Some(timeline.segments().playlist(
            INIT_SEGMENT,
            |i| format!("{i}.m4s"),
            start.filter(|s| s.is_finite() && *s > 0.0),
        ))
    }

    fn dir(&self, source: &StreamSource) -> PathBuf {
        self.root
            .join(format!("{}-v{COPY_VERSION}", source.fingerprint))
    }

    /// The settings changed: copies may have been switched on or off.
    pub fn retune(&self) {
        self.wake.notify_one();
    }

    fn set(&self, file: FileId, state: CopyState) {
        self.states.lock().unwrap().insert(file, state);
    }

    /// The heavy videos and their copies' folders; drops folders of videos
    /// no longer in the catalog. `None` when the catalog can't be read.
    async fn survey(&self) -> Option<Vec<(FileId, StreamSource)>> {
        let files = self
            .library
            .heavy_files(COPY_HEIGHT, HEAVY_BITRATE)
            .await
            .map_err(|e| tracing::warn!("can't list videos for streaming copies: {e}"))
            .ok()?;
        let mut heavy = Vec::new();
        for file in files {
            if let Ok(Some(source)) = self.library.stream_source(file).await {
                heavy.push((file, source));
            }
        }
        let keep: Vec<PathBuf> = heavy.iter().map(|(_, s)| self.dir(s)).collect();
        if let Ok(mut entries) = tokio::fs::read_dir(&self.root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if !keep.contains(&entry.path()) {
                    tracing::info!(dir = %entry.path().display(), "removing a streaming copy no video needs");
                    let _ = tokio::fs::remove_dir_all(entry.path()).await;
                }
            }
        }
        Some(heavy)
    }

    /// Brings `file`'s copy up to date: made in full, or as far as it gets
    /// before failing or being switched off.
    async fn make(&self, file: FileId, source: &StreamSource) -> Result<(), Stop> {
        let dir = self.dir(source);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
        let video = source
            .media
            .video
            .first()
            .ok_or(Stop::from("the file has no video"))?;
        if video.hdr != HdrFormat::Sdr && !self.tonemap {
            return Err("HDR video needs FFmpeg with zscale to make a copy".into());
        }
        let timeline = match read_timeline(&dir) {
            Some(t) => t,
            None => {
                let origin = nuvora_media::landing(&self.tools, &source.path, video.index, 0.0)
                    .await
                    .map_err(|e| e.to_string())?
                    .unwrap_or(0.0);
                let t = Timeline {
                    origin,
                    duration: source.media.duration_ms.unwrap_or(0) as f64 / 1000.0,
                    segment_seconds: f64::from(self.transcoder.tuning().segment_seconds),
                    height: video.height.min(COPY_HEIGHT),
                };
                let json = serde_json::to_vec(&t).expect("timeline serializes");
                tokio::fs::write(dir.join(TIMELINE_FILE), json)
                    .await
                    .map_err(|e| e.to_string())?;
                t
            }
        };
        let segments = timeline.segments();
        let first = first_missing(&dir, segments.len()).await;
        if first < segments.len() {
            let left = segments.end_of(segments.len() - 1) - segments.start(first);
            let needed = (left as u64) * BYTES_PER_SECOND + DISK_RESERVE;
            if let Some(free) = free_space(&self.root)
                && free < needed
            {
                return Err(Stop::NoSpace(format!(
                    "Not enough disk space: needs about {} GB, {} GB free",
                    needed / 1_000_000_000,
                    free / 1_000_000_000
                )));
            }
            self.run(file, source, &dir, &segments, first).await?;
            if first_missing(&dir, segments.len()).await < segments.len() {
                return Err("FFmpeg stopped before the end of the video".into());
            }
        }
        let bytes = folder_size(&dir).await;
        tokio::fs::write(dir.join(DONE_FILE), bytes.to_string())
            .await
            .map_err(|e| e.to_string())?;
        self.set(
            file,
            CopyState::Ready {
                height: timeline.height,
                bitrate: (bytes as f64 * 8.0 / timeline.duration.max(1.0)) as u64,
            },
        );
        tracing::info!(path = %source.path.display(), "streaming copy ready");
        Ok(())
    }

    /// One FFmpeg run from segment `first` to the end, held while anyone
    /// streams through the server.
    async fn run(
        &self,
        file: FileId,
        source: &StreamSource,
        dir: &Path,
        segments: &Segments,
        first: usize,
    ) -> Result<(), Stop> {
        let audio = source
            .media
            .audio
            .iter()
            .find(|a| a.is_default)
            .or(source.media.audio.first())
            .map(|a| AudioPlan {
                stream: a.index,
                output: AudioOutput::Aac {
                    channels: a.channels.clamp(1, 6),
                },
            });
        let plan = StreamPlan {
            video: VideoPlan::H264 {
                max_height: COPY_HEIGHT,
            },
            audio,
        };
        let options = EncodeOptions {
            tonemap: self.tonemap,
            ..self.transcoder.tuning().encode_options()
        };
        let encoder = self.transcoder.encoder();
        let args = Run {
            path: &source.path,
            media: &source.media,
            plan: &plan,
            segments,
            first,
            seek: (first > 0).then(|| segments.start(first)),
            dir,
            encoder: &encoder,
            options: &options,
        }
        .args();
        let log = std::fs::File::create(dir.join("ffmpeg.log")).map_err(|e| e.to_string())?;
        let mut command = if cfg!(unix) {
            // The lowest priority: anyone's stream comes first.
            let mut c = Command::new("nice");
            c.args(["-n", "19"]).arg(self.tools.ffmpeg());
            c
        } else {
            Command::new(self.tools.ffmpeg())
        };
        let mut child = command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("can't start FFmpeg: {e}"))?;
        let yield_now = || {
            !self.transcoder.tuning().streaming_copies
                || self
                    .urgent
                    .lock()
                    .unwrap()
                    .first()
                    .is_some_and(|u| *u != file)
        };
        tracing::info!(path = %source.path.display(), first, "making a streaming copy");
        let report = Arc::new(Mutex::new(RunProgress::default()));
        if let Some(stdout) = child.stdout.take() {
            let report = report.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                let mut progress = RunProgress::default();
                while let Ok(Some(line)) = lines.next_line().await {
                    if progress.read_line(&line) {
                        *report.lock().unwrap() = progress.clone();
                    }
                }
            });
        }
        let start = segments.origin();
        let end = segments.end_of(segments.len() - 1);
        let total = (end - start).max(1.0);
        let mut paused = false;
        // FFmpeg's own speed averages over the whole run, pauses included:
        // measure it here instead, over running time only.
        let mut speed: Option<f64> = None;
        let mut last: Option<(Instant, f64)> = None;
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if status.success() {
                    return Ok(());
                }
                let log = tokio::fs::read_to_string(dir.join("ffmpeg.log"))
                    .await
                    .unwrap_or_default();
                let tail: Vec<&str> = log.lines().rev().take(3).collect();
                let tail: Vec<&str> = tail.into_iter().rev().collect();
                return Err(format!("FFmpeg stopped: {}", tail.join(" / ")).into());
            }
            if yield_now() {
                signal(&child, false);
                let _ = child.kill().await;
                return Err(Stop::Yield);
            }
            let hold = self.transcoder.streaming();
            if hold != paused {
                signal(&child, hold);
                paused = hold;
                last = None;
            }
            let reported = report.lock().unwrap().out_time;
            let at = reported.unwrap_or(segments.start(first)).clamp(start, end);
            if !paused && reported.is_some() {
                let now = Instant::now();
                if let Some((then, was)) = last {
                    let wall = now.duration_since(then).as_secs_f64();
                    // Over half a minute or more, so one slow scene doesn't swing it.
                    if wall >= 30.0 {
                        let rate = (at - was).max(0.0) / wall;
                        speed = Some(speed.map_or(rate, |s| s + (rate - s) * 0.3));
                        last = Some((now, at));
                    }
                } else {
                    last = Some((now, at));
                }
            }
            let speed = speed.filter(|s| *s > 0.0 && !paused);
            self.set(
                file,
                CopyState::Making {
                    progress: ((at - start) / total).clamp(0.0, 1.0),
                    speed,
                    eta_s: speed.map(|s| ((end - at) / s) as u64),
                    paused,
                },
            );
        }
    }
}

/// Every minute and whenever someone opens a heavy video: list what
/// needs a copy, and make the most wanted one.
async fn work(copies: Arc<Copies>) {
    // Let the startup scan read the catalog first.
    tokio::time::sleep(Duration::from_secs(20)).await;
    let mut failed: HashMap<FileId, String> = HashMap::new();
    loop {
        let on = copies.transcoder.tuning().streaming_copies;
        let Some(heavy) = copies.survey().await else {
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        };
        let mut todo = Vec::new();
        {
            let mut states = copies.states.lock().unwrap();
            states.clear();
            for (file, source) in &heavy {
                let dir = copies.dir(source);
                let state = if dir.join(DONE_FILE).is_file() {
                    ready_state(&dir)
                } else if let Some(reason) = failed.get(file) {
                    CopyState::Failed {
                        reason: reason.clone(),
                    }
                } else if !on {
                    CopyState::Off
                } else {
                    todo.push(*file);
                    CopyState::Queued { position: 0 }
                };
                states.insert(*file, state);
            }
            let urgent = copies.urgent.lock().unwrap().clone();
            todo.sort_by_key(|f| urgent.iter().position(|u| u == f).unwrap_or(usize::MAX));
            for (i, file) in todo.iter().enumerate() {
                states.insert(
                    *file,
                    CopyState::Queued {
                        position: i as u32 + 1,
                    },
                );
            }
        }
        let Some(&file) = todo.first() else {
            tokio::select! {
                () = copies.wake.notified() => {}
                () = tokio::time::sleep(Duration::from_secs(60)) => {}
            }
            continue;
        };
        let (_, source) = heavy.iter().find(|(f, _)| *f == file).expect("listed");
        copies.urgent.lock().unwrap().retain(|f| *f != file);
        match copies.make(file, source).await {
            Err(Stop::Failed(reason)) => {
                tracing::warn!(path = %source.path.display(), %reason, "no streaming copy");
                failed.insert(file, reason);
            }
            Err(Stop::NoSpace(reason)) => {
                tracing::warn!(path = %source.path.display(), %reason, "streaming copy waits for disk space");
                copies.set(file, CopyState::Failed { reason });
                tokio::select! {
                    () = copies.wake.notified() => {}
                    () = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
            }
            Ok(()) | Err(Stop::Yield) => {}
        }
    }
}

fn ready_state(dir: &Path) -> CopyState {
    let bytes: u64 = std::fs::read_to_string(dir.join(DONE_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    match read_timeline(dir) {
        Some(t) => CopyState::Ready {
            height: t.height,
            bitrate: (bytes as f64 * 8.0 / t.duration.max(1.0)) as u64,
        },
        None => CopyState::Failed {
            reason: "The copy's timeline is missing".into(),
        },
    }
}

fn read_timeline(dir: &Path) -> Option<Timeline> {
    serde_json::from_slice(&std::fs::read(dir.join(TIMELINE_FILE)).ok()?).ok()
}

/// The first segment not on disk; `count` when all are.
async fn first_missing(dir: &Path, count: usize) -> usize {
    for i in 0..count {
        if !tokio::fs::try_exists(dir.join(format!("{i}.m4s")))
            .await
            .unwrap_or(false)
        {
            return i;
        }
    }
    count
}

async fn folder_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            total += entry.metadata().await.map_or(0, |m| m.len());
        }
    }
    total
}

#[cfg(unix)]
fn free_space(path: &Path) -> Option<u64> {
    let stat = nix::sys::statvfs::statvfs(path).ok()?;
    Some(stat.blocks_available() as u64 * stat.fragment_size() as u64)
}

#[cfg(not(unix))]
fn free_space(_: &Path) -> Option<u64> {
    None
}

async fn has_zscale(tools: &MediaTools) -> bool {
    Command::new(tools.ffmpeg())
        .args(["-hide_banner", "-h", "filter=zscale"])
        .stdin(Stdio::null())
        .output()
        .await
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("Filter zscale"))
}

/// SIGSTOP (`pause`) or SIGCONT. A no-op off Unix.
fn signal(child: &Child, pause: bool) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        use nix::sys::signal::{Signal, kill};
        let sig = if pause {
            Signal::SIGSTOP
        } else {
            Signal::SIGCONT
        };
        let _ = kill(nix::unistd::Pid::from_raw(pid as i32), sig);
    }
    #[cfg(not(unix))]
    let _ = (child, pause);
}
