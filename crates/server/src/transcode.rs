//! Live HLS sessions: one ffmpeg run per player, writing fMP4 segments into
//! the session's own folder under `<data>/transcode`.
//!
//! A segment request is answered from disk when the run has made it, waited
//! for when the run is about to, and otherwise restarts the run at that
//! segment (a seek). A run far ahead of the player is paused (SIGSTOP) until
//! the player catches up, segments far behind it are deleted, and a session
//! nobody has asked anything of for a while is stopped and removed. How far,
//! how long and how many come from the admin panel's [`Tuning`]: a session
//! keeps the encoder, encode options and segment length it opened with (every
//! run must cut and encode the same way), while pacing and limits follow the
//! current settings. See `nuvora_media::hls` for why segments from different
//! runs line up.
//!
//! Each run reports its progress on stdout; [`Transcoder::activity`] reads
//! it, with what is on disk, for the admin panel's Activity page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, Instant, SystemTime};

use nuvora_library::StreamSource;
use nuvora_media::{
    EncodeOptions, HlsError, INIT_SEGMENT, MediaInfo, MediaTools, Run, RunProgress, Segments,
    StreamPlan, VideoEncoder, VideoPlan, hardware_candidates,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::config::HardwareEncoding;
use crate::tuning::Tuning;

/// The encoder for converted video: the one `choice` names, or with `auto`
/// the first graphics encoder that converts a test clip here (with
/// `options`), else x264. When a named one fails, also why.
pub async fn pick_encoder(
    tools: &MediaTools,
    choice: HardwareEncoding,
    vaapi_device: &Path,
    options: &EncodeOptions,
) -> (VideoEncoder, Option<String>) {
    let vaapi = || VideoEncoder::Vaapi {
        device: vaapi_device.to_path_buf(),
    };
    let candidates = match choice {
        HardwareEncoding::Off => vec![],
        HardwareEncoding::Auto => hardware_candidates()
            .into_iter()
            .map(|c| match c {
                VideoEncoder::Vaapi { .. } => vaapi(),
                other => other,
            })
            .collect(),
        HardwareEncoding::Videotoolbox => vec![VideoEncoder::VideoToolbox],
        HardwareEncoding::Nvenc => vec![VideoEncoder::Nvenc],
        HardwareEncoding::Vaapi => vec![vaapi()],
    };
    let mut problem = None;
    for candidate in candidates {
        match tools.try_encoder(&candidate, options).await {
            Ok(()) => {
                tracing::info!(
                    encoder = candidate.label(),
                    "converting video on the graphics chip"
                );
                return (candidate, None);
            }
            // With `auto`, a chip this machine lacks is the usual case, not a problem.
            Err(reason) if choice == HardwareEncoding::Auto => {
                tracing::debug!(encoder = candidate.label(), %reason, "not usable here");
            }
            Err(reason) => {
                tracing::warn!(encoder = candidate.label(), %reason, "the chosen encoder doesn't work here; using the processor");
                problem = Some(format!("{} didn't work here: {reason}", candidate.label()));
            }
        }
    }
    tracing::info!("converting video on the processor (x264)");
    (VideoEncoder::Software, problem)
}

/// How long a request waits for its segment before giving up.
const WAIT: Duration = Duration::from_secs(60);
/// A seek target past a keyframe by more than ffmpeg's 3/23 s backoff, so a
/// copying run starts on that keyframe and not the one before.
const SEEK_PAST_KEYFRAME: f64 = 0.2;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub user: i64,
    /// Chosen by the player, one per playback.
    pub session: String,
    pub file: i64,
    pub plan: StreamPlan,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("no such segment")]
    NotFound,
    /// The server converts as many videos at once as it is allowed to.
    #[error("already converting {0} videos, the most this server is set to")]
    Busy(u32),
    #[error("the file has no video stream")]
    NoVideo,
    #[error("converting timed out")]
    Timeout,
    #[error("ffmpeg stopped: {0}")]
    Failed(String),
    #[error(transparent)]
    Media(#[from] HlsError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct Transcoder {
    tools: MediaTools,
    root: PathBuf,
    /// The encoder `nuvora.toml` or a flag names; the panel's choice otherwise.
    pinned: Option<HardwareEncoding>,
    setup: RwLock<Setup>,
    /// One [`Transcoder::retune`] at a time.
    retuning: tokio::sync::Mutex<()>,
    sessions: Mutex<HashMap<SessionKey, Arc<Session>>>,
    /// Keyframe times by file fingerprint, for the few most recent files.
    keyframes: Mutex<Vec<(String, Arc<Vec<f64>>)>>,
}

/// The settings and the encoder they picked.
#[derive(Clone)]
struct Setup {
    tuning: Arc<Tuning>,
    encoder: VideoEncoder,
    /// Why the chosen graphics encoder isn't the one in use.
    problem: Option<String>,
}

pub struct Session {
    /// The session folder's name: opaque, unique.
    id: String,
    tools: MediaTools,
    encoder: VideoEncoder,
    options: EncodeOptions,
    /// Waiting for a run versus restarting it.
    catch_up: f64,
    dir: PathBuf,
    path: PathBuf,
    media: MediaInfo,
    plan: StreamPlan,
    segments: Segments,
    opened: SystemTime,
    run: tokio::sync::Mutex<RunState>,
    activity: Mutex<Activity>,
    progress: Arc<Mutex<LiveProgress>>,
}

#[derive(Default)]
struct RunState {
    child: Option<Child>,
    /// The first segment the current run writes.
    first: usize,
    /// The lowest segment of the run not on disk yet.
    made: usize,
    paused: bool,
    started: Option<Instant>,
    /// The last run failed with this (FFmpeg's last lines).
    failed: Option<String>,
}

/// The current run's latest progress report.
#[derive(Default)]
struct LiveProgress {
    /// Counts runs, so a killed run's last words never land on its successor.
    run: u64,
    report: Option<RunProgress>,
}

/// One stream, as the admin panel's Activity page shows it. Times are in
/// player seconds (0 is the first frame).
pub struct StreamActivity {
    pub id: String,
    pub user: i64,
    pub file: i64,
    pub plan: StreamPlan,
    pub media: MediaInfo,
    /// What converts the video; `None` when it is copied.
    pub encoder: Option<&'static str>,
    pub options: EncodeOptions,
    pub duration: f64,
    /// The start of the segment the player asked for last.
    pub position: f64,
    /// What is on disk, as `(start, end)` spans.
    pub made: Vec<(f64, f64)>,
    pub segments: usize,
    /// The current run: where it started and how far it has got.
    pub run_span: Option<(f64, f64)>,
    pub run_for: Option<Duration>,
    pub state: RunPhase,
    pub progress: Option<RunProgress>,
    /// The session folder's size: segments, the init segment and the log.
    pub disk_bytes: u64,
    pub idle: Duration,
    pub opened: SystemTime,
}

pub enum RunPhase {
    /// No run: none asked for yet, or between one and the next.
    Waiting,
    Converting,
    /// Far enough ahead of the player; resumes as it catches up.
    Paused,
    /// The run reached the end of the file.
    Finished,
    Failed(String),
}

struct Activity {
    at: Instant,
    /// The last segment the player asked for.
    index: usize,
    /// Segments below this have been deleted.
    swept: usize,
}

impl Transcoder {
    /// Picks the encoder, clears what a previous process left in
    /// `<data>/transcode` and starts the housekeeping task.
    pub async fn start(
        tools: MediaTools,
        data_dir: &Path,
        tuning: Tuning,
        pinned: Option<HardwareEncoding>,
    ) -> Arc<Transcoder> {
        let (encoder, problem) = pick_encoder(
            &tools,
            pinned.unwrap_or(tuning.hardware_encoding),
            Path::new(&tuning.vaapi_device),
            &tuning.encode_options(),
        )
        .await;
        let root = data_dir.join("transcode");
        let _ = std::fs::remove_dir_all(&root);
        let transcoder = Arc::new(Transcoder {
            tools,
            root,
            pinned,
            setup: RwLock::new(Setup {
                tuning: Arc::new(tuning),
                encoder,
                problem,
            }),
            retuning: tokio::sync::Mutex::new(()),
            sessions: Mutex::new(HashMap::new()),
            keyframes: Mutex::new(Vec::new()),
        });
        tokio::spawn(housekeeping(Arc::downgrade(&transcoder)));
        transcoder
    }

    fn setup(&self) -> Setup {
        self.setup.read().unwrap().clone()
    }

    pub fn tuning(&self) -> Arc<Tuning> {
        self.setup().tuning
    }

    /// What converts video for streams opened now.
    pub fn encoder(&self) -> VideoEncoder {
        self.setup().encoder
    }

    /// Why the chosen graphics encoder isn't the one in use, if it isn't.
    pub fn encoder_problem(&self) -> Option<String> {
        self.setup().problem
    }

    /// The encoder `nuvora.toml` or a flag names, over the panel's.
    pub fn pinned(&self) -> Option<HardwareEncoding> {
        self.pinned
    }

    /// New settings for streams opened from now on, and for every stream's
    /// pacing. The encoder is picked again (a test clip) when anything it
    /// depends on changed.
    pub async fn retune(&self, tuning: Tuning) {
        let _one_at_a_time = self.retuning.lock().await;
        let old = self.setup();
        let same_encoder = old.tuning.hardware_encoding == tuning.hardware_encoding
            && old.tuning.vaapi_device == tuning.vaapi_device
            && old.tuning.encode_options() == tuning.encode_options();
        let (encoder, problem) = if same_encoder {
            (old.encoder, old.problem)
        } else {
            pick_encoder(
                &self.tools,
                self.pinned.unwrap_or(tuning.hardware_encoding),
                Path::new(&tuning.vaapi_device),
                &tuning.encode_options(),
            )
            .await
        };
        *self.setup.write().unwrap() = Setup {
            tuning: Arc::new(tuning),
            encoder,
            problem,
        };
    }

    /// Every open stream, oldest first.
    pub async fn activity(&self) -> Vec<StreamActivity> {
        let sessions: Vec<(SessionKey, Arc<Session>)> = self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|(k, s)| (k.clone(), s.clone()))
            .collect();
        let mut out = Vec::with_capacity(sessions.len());
        for (key, session) in sessions {
            out.push(session.snapshot(&key).await);
        }
        out.sort_by_key(|a| a.opened);
        out
    }

    /// The session for `key`, opened on first use: the file's segments are
    /// worked out (keyframes for a copied video) and its folder made.
    pub async fn session(
        &self,
        key: SessionKey,
        source: StreamSource,
    ) -> Result<Arc<Session>, TranscodeError> {
        if let Some(session) = self.sessions.lock().unwrap().get(&key) {
            return Ok(session.clone());
        }
        let setup = self.setup();
        let video = source
            .media
            .video
            .first()
            .ok_or(TranscodeError::NoVideo)?
            .index;
        let duration = source.media.duration_ms.unwrap_or(0) as f64 / 1000.0;
        let segments = match key.plan.video {
            VideoPlan::Copy => {
                let keyframes = self.keyframes(&source, video).await?;
                Segments::at_keyframes(&keyframes, duration).ok_or(HlsError::NoKeyframes)?
            }
            VideoPlan::H264 { .. } => {
                let origin = nuvora_media::landing(&self.tools, &source.path, video, 0.0)
                    .await?
                    .unwrap_or(0.0);
                Segments::every(origin, duration, f64::from(setup.tuning.segment_seconds))
            }
        };
        let id = session_dir_name(&key);
        let dir = self.root.join(&id);
        let tuning = &setup.tuning;
        let session = Arc::new(Session {
            id,
            tools: self.tools.clone(),
            encoder: setup.encoder.clone(),
            options: tuning.encode_options(),
            catch_up: f64::from(tuning.catch_up_s),
            dir,
            path: source.path,
            media: source.media,
            plan: key.plan,
            segments,
            opened: SystemTime::now(),
            run: tokio::sync::Mutex::new(RunState::default()),
            activity: Mutex::new(Activity {
                at: Instant::now(),
                index: 0,
                swept: 0,
            }),
            progress: Arc::default(),
        });
        tokio::fs::create_dir_all(&session.dir).await?;
        // `Err` holds what to answer instead: the session another request
        // opened meanwhile, or a refusal.
        let evicted = (|| {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(existing) = sessions.get(&key) {
                // Another request opened it while this one worked.
                return Err(Ok(existing.clone()));
            }
            let least_recent_first =
                |sessions: &HashMap<SessionKey, Arc<Session>>, also: &dyn Fn(&Session) -> bool| {
                    let mut mine: Vec<(SessionKey, Instant)> = sessions
                        .iter()
                        .filter(|(k, s)| k.user == key.user && also(s))
                        .map(|(k, s)| (k.clone(), s.activity.lock().unwrap().at))
                        .collect();
                    mine.sort_by_key(|(_, at)| *at);
                    mine.into_iter().map(|(k, _)| k).collect::<Vec<_>>()
                };
            let mut evicted = Vec::new();
            // At the server's limit, a person starting a video gives up their
            // own oldest conversions (likely abandoned) before anyone is refused.
            let limit = tuning.max_conversions as usize;
            if limit > 0 && session.converts() {
                let others = sessions
                    .iter()
                    .filter(|(k, s)| k.user != key.user && s.converts())
                    .count();
                if others >= limit {
                    return Err(Err(TranscodeError::Busy(tuning.max_conversions)));
                }
                let mine = least_recent_first(&sessions, &Session::converts);
                let excess = (others + mine.len() + 1).saturating_sub(limit);
                evicted.extend(mine.iter().take(excess).filter_map(|k| sessions.remove(k)));
            }
            let mine = least_recent_first(&sessions, &|_| true);
            let excess = (mine.len() + 1).saturating_sub(tuning.streams_per_user as usize);
            evicted.extend(mine.iter().take(excess).filter_map(|k| sessions.remove(k)));
            sessions.insert(key, session.clone());
            Ok(evicted)
        })();
        let evicted = match evicted {
            Ok(evicted) => evicted,
            Err(not_opened) => {
                let _ = tokio::fs::remove_dir_all(&session.dir).await;
                return not_opened;
            }
        };
        for old in evicted {
            old.stop().await;
        }
        Ok(session)
    }

    async fn keyframes(
        &self,
        source: &StreamSource,
        video: u32,
    ) -> Result<Arc<Vec<f64>>, HlsError> {
        let id = format!("{}:{video}", source.fingerprint);
        if let Some((_, k)) = self
            .keyframes
            .lock()
            .unwrap()
            .iter()
            .find(|(f, _)| *f == id)
        {
            return Ok(k.clone());
        }
        let found = Arc::new(nuvora_media::keyframes(&self.tools, &source.path, video).await?);
        let mut cache = self.keyframes.lock().unwrap();
        cache.retain(|(f, _)| *f != id);
        cache.push((id, found.clone()));
        if cache.len() > 16 {
            cache.remove(0);
        }
        Ok(found)
    }
}

impl Session {
    fn converts(&self) -> bool {
        matches!(self.plan.video, VideoPlan::H264 { .. })
    }

    /// The playlist. `query` is appended to every URI (the session id), and
    /// the init segment's URI names the segment playback starts at, so the
    /// first run starts there.
    pub fn playlist(&self, query: &str, start: Option<f64>) -> String {
        let from = self.segments.index_at(start.unwrap_or(0.0));
        self.touch(None);
        self.segments.playlist(
            &format!("{INIT_SEGMENT}?{query}&from={from}"),
            |i| format!("{i}.m4s?{query}"),
            start,
        )
    }

    /// The init segment. Every run writes the same one, so the first run's
    /// copy serves the whole session; with none yet, a run starts at `from`.
    pub async fn init(&self, from: usize) -> Result<PathBuf, TranscodeError> {
        let ready = self.dir.join("init.ready.mp4");
        if tokio::fs::try_exists(&ready).await.unwrap_or(false) {
            return Ok(ready);
        }
        self.segment(from.min(self.segments.len() - 1)).await?;
        Ok(ready)
    }

    /// Segment `index`, made first if needed.
    pub async fn segment(&self, index: usize) -> Result<PathBuf, TranscodeError> {
        if index >= self.segments.len() {
            return Err(TranscodeError::NotFound);
        }
        self.touch(Some(index));
        let file = self.dir.join(format!("{index}.m4s"));
        let deadline = Instant::now() + WAIT;
        loop {
            if tokio::fs::try_exists(&file).await.unwrap_or(false) {
                self.keep_init().await?;
                return Ok(file);
            }
            {
                let mut run = self.run.lock().await;
                let running = match run.child.as_mut().map(Child::try_wait) {
                    Some(Ok(None)) => true,
                    Some(Ok(Some(status))) if !status.success() => {
                        run.child = None;
                        let why = self.log_tail().await;
                        run.failed = Some(why.clone());
                        return Err(TranscodeError::Failed(why));
                    }
                    _ => false,
                };
                self.advance(&mut run).await;
                let reachable = running
                    && run.first <= index
                    && self.segments.start(index)
                        <= self.segments.start(run.made.min(self.segments.len() - 1))
                            + self.catch_up;
                if !reachable {
                    self.start(&mut run, index).await?;
                } else if run.paused {
                    signal(&run, false);
                    run.paused = false;
                }
            }
            if Instant::now() >= deadline {
                return Err(TranscodeError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Kills the run and deletes the session's folder.
    async fn stop(&self) {
        let mut run = self.run.lock().await;
        if let Some(mut child) = run.child.take() {
            signal(&run, false);
            let _ = child.kill().await;
        }
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
    }

    fn touch(&self, index: Option<usize>) {
        let mut activity = self.activity.lock().unwrap();
        activity.at = Instant::now();
        if let Some(index) = index {
            activity.index = index;
        }
    }

    /// Moves `made` past every segment the run has written.
    async fn advance(&self, run: &mut RunState) {
        while run.made < self.segments.len()
            && tokio::fs::try_exists(self.dir.join(format!("{}.m4s", run.made)))
                .await
                .unwrap_or(false)
        {
            run.made += 1;
        }
    }

    /// Replaces the run with one whose first segment is `index` or, for a
    /// copied video, the keyframe at or before it that ffmpeg can seek to.
    async fn start(&self, run: &mut RunState, index: usize) -> Result<(), TranscodeError> {
        if let Some(mut child) = run.child.take() {
            signal(run, false);
            let _ = child.kill().await;
        }
        let video = self
            .media
            .video
            .first()
            .ok_or(TranscodeError::NoVideo)?
            .index;
        let (first, seek) = match self.plan.video {
            _ if index == 0 => (0, None),
            VideoPlan::H264 { .. } => (index, Some(self.segments.start(index))),
            VideoPlan::Copy => {
                let at = self.segments.start(index) + SEEK_PAST_KEYFRAME;
                let landed = nuvora_media::landing(&self.tools, &self.path, video, at)
                    .await?
                    .unwrap_or(self.segments.start(index));
                let first = self.segments.index_of(landed);
                if first > index {
                    // The next keyframe is closer than the backoff: start a
                    // segment early instead.
                    let at = self.segments.start(index.saturating_sub(1)) + SEEK_PAST_KEYFRAME;
                    (index.saturating_sub(1), Some(at))
                } else {
                    (first, Some(at))
                }
            }
        };
        let log = std::fs::File::create(self.dir.join("ffmpeg.log"))?;
        let args = Run {
            path: &self.path,
            media: &self.media,
            plan: &self.plan,
            segments: &self.segments,
            first,
            seek,
            dir: &self.dir,
            encoder: &self.encoder,
            options: &self.options,
        }
        .args();
        let mut child = Command::new(self.tools.ffmpeg())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()?;
        tracing::debug!(dir = %self.dir.display(), first, ?seek, "transcode run started");
        if let Some(stdout) = child.stdout.take() {
            let this_run = {
                let mut live = self.progress.lock().unwrap();
                live.run += 1;
                live.report = None;
                live.run
            };
            let live = Arc::clone(&self.progress);
            // Reads to the end whatever happens, so FFmpeg never blocks on a full pipe.
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                let mut report = RunProgress::default();
                while let Ok(Some(line)) = lines.next_line().await {
                    if report.read_line(&line) {
                        let mut live = live.lock().unwrap();
                        if live.run == this_run {
                            live.report = Some(report.clone());
                        }
                    }
                }
            });
        }
        *run = RunState {
            child: Some(child),
            first,
            made: first,
            paused: false,
            started: Some(Instant::now()),
            failed: None,
        };
        Ok(())
    }

    /// See [`StreamActivity`].
    async fn snapshot(&self, key: &SessionKey) -> StreamActivity {
        let (index, at) = {
            let a = self.activity.lock().unwrap();
            (a.index, a.at)
        };
        let mut on_disk = Vec::new();
        let mut disk_bytes = 0;
        if let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                disk_bytes += entry.metadata().await.map_or(0, |m| m.len());
                let name = entry.file_name();
                if let Some(n) = name
                    .to_str()
                    .and_then(|n| n.strip_suffix(".m4s"))
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| *n < self.segments.len())
                {
                    on_disk.push(n);
                }
            }
        }
        on_disk.sort_unstable();
        let origin = self.segments.origin();
        let player = |source: f64| (source - origin).max(0.0);
        let mut made: Vec<(f64, f64)> = Vec::new();
        let mut previous = None;
        for n in on_disk {
            let (start, end) = (
                player(self.segments.start(n)),
                player(self.segments.end_of(n)),
            );
            match made.last_mut() {
                Some(span) if previous == Some(n.wrapping_sub(1)) => span.1 = end,
                _ => made.push((start, end)),
            }
            previous = Some(n);
        }
        let last = self.segments.len() - 1;
        let duration = player(self.segments.end_of(last));
        let (state, run_span, run_for) = match self.run.try_lock() {
            // A run is starting or a request is checking on it.
            Err(_) => (RunPhase::Converting, None, None),
            Ok(mut run) => {
                self.advance(&mut run).await;
                let state = match run.child.as_mut().map(Child::try_wait) {
                    None => match &run.failed {
                        Some(why) => RunPhase::Failed(why.clone()),
                        None => RunPhase::Waiting,
                    },
                    Some(Ok(None)) if run.paused => RunPhase::Paused,
                    Some(Ok(None)) => RunPhase::Converting,
                    Some(Ok(Some(status))) if status.success() => RunPhase::Finished,
                    Some(_) => RunPhase::Failed(self.log_tail().await),
                };
                let span = run.child.is_some().then(|| {
                    let reached = if run.made > last {
                        duration
                    } else {
                        player(self.segments.start(run.made))
                    };
                    (player(self.segments.start(run.first.min(last))), reached)
                });
                (state, span, run.started.map(|s| s.elapsed()))
            }
        };
        let progress = self.progress.lock().unwrap().report.clone();
        StreamActivity {
            id: self.id.clone(),
            user: key.user,
            file: key.file,
            plan: self.plan,
            media: self.media.clone(),
            encoder: self.converts().then(|| self.encoder.label()),
            options: self.options.clone(),
            duration,
            position: player(self.segments.start(index.min(last))),
            made,
            segments: self.segments.len(),
            run_span,
            run_for,
            state,
            progress,
            disk_bytes,
            idle: at.elapsed(),
            opened: self.opened,
        }
    }

    /// Copies the run's init segment to a name no later run overwrites, once
    /// a media segment proves it complete.
    async fn keep_init(&self) -> Result<(), TranscodeError> {
        let ready = self.dir.join("init.ready.mp4");
        if !tokio::fs::try_exists(&ready).await.unwrap_or(false) {
            // Concurrent requests each copy to their own name; the renames
            // race harmlessly, every copy being the same bytes.
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let partial = self.dir.join(format!(
                "init.{}.copy",
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            tokio::fs::copy(self.dir.join(INIT_SEGMENT), &partial).await?;
            tokio::fs::rename(&partial, &ready).await?;
        }
        Ok(())
    }

    async fn log_tail(&self) -> String {
        let log = tokio::fs::read_to_string(self.dir.join("ffmpeg.log"))
            .await
            .unwrap_or_default();
        let lines: Vec<&str> = log.lines().rev().take(3).collect();
        lines.into_iter().rev().collect::<Vec<_>>().join(" / ")
    }

    /// Pauses a run far ahead of the player, resumes one it is catching up
    /// to, and deletes segments far behind it.
    async fn keep_pace(&self, tuning: &Tuning) {
        let pause_ahead = f64::from(tuning.buffer_ahead_s);
        let (index, swept) = {
            let a = self.activity.lock().unwrap();
            (a.index, a.swept)
        };
        let Ok(mut run) = self.run.try_lock() else {
            return;
        };
        self.advance(&mut run).await;
        if run.child.is_some() && run.made > index {
            let ahead = self.segments.start(run.made.min(self.segments.len() - 1))
                - self.segments.start(index);
            if tuning.throttle && !run.paused && ahead > pause_ahead {
                signal(&run, true);
                run.paused = true;
            } else if run.paused && (!tuning.throttle || ahead < pause_ahead / 2.0) {
                signal(&run, false);
                run.paused = false;
            }
        }
        drop(run);
        let behind = self.segments.start(index) - f64::from(tuning.keep_behind_s);
        let mut i = swept;
        while i < index && self.segments.start(i) < behind {
            let _ = tokio::fs::remove_file(self.dir.join(format!("{i}.m4s"))).await;
            i += 1;
        }
        self.activity.lock().unwrap().swept = i;
    }
}

/// Every two seconds: drop idle sessions, keep the others' runs in step.
async fn housekeeping(transcoder: Weak<Transcoder>) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tick.tick().await;
        let Some(transcoder) = transcoder.upgrade() else {
            return;
        };
        let tuning = transcoder.tuning();
        let idle_after = Duration::from_secs(tuning.idle_timeout_s.into());
        let (idle, live): (Vec<_>, Vec<_>) = {
            let mut sessions = transcoder.sessions.lock().unwrap();
            let idle_keys: Vec<SessionKey> = sessions
                .iter()
                .filter(|(_, s)| s.activity.lock().unwrap().at.elapsed() > idle_after)
                .map(|(k, _)| k.clone())
                .collect();
            let idle = idle_keys
                .iter()
                .filter_map(|k| sessions.remove(k))
                .collect();
            (idle, sessions.values().cloned().collect())
        };
        for session in idle {
            session.stop().await;
        }
        for session in live {
            session.keep_pace(&tuning).await;
        }
    }
}

/// SIGSTOP (`pause`) or SIGCONT the run. A no-op off Unix.
fn signal(run: &RunState, pause: bool) {
    #[cfg(unix)]
    if let Some(pid) = run.child.as_ref().and_then(Child::id) {
        use nix::sys::signal::{Signal, kill};
        let sig = if pause {
            Signal::SIGSTOP
        } else {
            Signal::SIGCONT
        };
        let _ = kill(nix::unistd::Pid::from_raw(pid as i32), sig);
    }
    #[cfg(not(unix))]
    let _ = (run, pause);
}

/// Unique per session and opaque: a hash of the key and the time.
fn session_dir_name(key: &SessionKey) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(format!("{key:?}").as_bytes());
    hasher.update(
        &std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hasher.finalize().to_hex()[..20].to_owned()
}
