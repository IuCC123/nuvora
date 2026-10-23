//! Live HLS sessions: one ffmpeg run per player, writing fMP4 segments into
//! the session's own folder under `<data>/transcode`.
//!
//! A segment request is answered from disk when the run has made it, waited
//! for when the run is about to, and otherwise restarts the run at that
//! segment (a seek). A run far ahead of the player is paused (SIGSTOP) until
//! the player catches up, segments far behind it are deleted, and a session
//! nobody has asked anything of for [`IDLE`] is stopped and removed. See
//! `nuvora_media::hls` for why segments from different runs line up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use nuvora_library::StreamSource;
use nuvora_media::{
    HlsError, INIT_SEGMENT, MediaInfo, MediaTools, Run, Segments, StreamPlan, VideoEncoder,
    VideoPlan, hardware_candidates,
};
use tokio::process::{Child, Command};

use crate::config::HardwareEncoding;

/// The encoder for converted video: the one the setting names, or with
/// `auto` the first graphics encoder that converts a test clip here, else x264.
pub async fn pick_encoder(tools: &MediaTools, setting: HardwareEncoding) -> VideoEncoder {
    let candidates = match setting {
        HardwareEncoding::Off => vec![],
        HardwareEncoding::Auto => hardware_candidates(),
        HardwareEncoding::Videotoolbox => vec![VideoEncoder::VideoToolbox],
        HardwareEncoding::Nvenc => vec![VideoEncoder::Nvenc],
        HardwareEncoding::Vaapi => vec![VideoEncoder::Vaapi {
            device: nuvora_media::DEFAULT_VAAPI_DEVICE.into(),
        }],
    };
    for candidate in candidates {
        match tools.try_encoder(&candidate).await {
            Ok(()) => {
                tracing::info!(
                    encoder = candidate.label(),
                    "converting video on the graphics chip"
                );
                return candidate;
            }
            // With `auto`, a chip this machine lacks is the usual case, not a problem.
            Err(reason) if setting == HardwareEncoding::Auto => {
                tracing::debug!(encoder = candidate.label(), %reason, "not usable here");
            }
            Err(reason) => {
                tracing::warn!(encoder = candidate.label(), %reason, "the chosen encoder doesn't work here; using the processor");
            }
        }
    }
    tracing::info!("converting video on the processor (x264)");
    VideoEncoder::Software
}

/// A session nobody has requested anything of for this long is removed.
const IDLE: Duration = Duration::from_secs(90);
/// How long a request waits for its segment before giving up.
const WAIT: Duration = Duration::from_secs(60);
/// A request this little ahead of the run (in media seconds) waits for it
/// rather than restarting it.
const CATCH_UP_SECONDS: f64 = 20.0;
/// Pause a run this far ahead of the player; resume below [`RESUME_AHEAD`].
const PAUSE_AHEAD: f64 = 180.0;
const RESUME_AHEAD: f64 = 90.0;
/// Segments this far behind the player are deleted.
const KEEP_BEHIND: f64 = 600.0;
/// Sessions per user; opening one more stops that user's least recent.
const SESSIONS_PER_USER: usize = 3;
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
    encoder: VideoEncoder,
    root: PathBuf,
    sessions: Mutex<HashMap<SessionKey, Arc<Session>>>,
    /// Keyframe times by file fingerprint, for the few most recent files.
    keyframes: Mutex<Vec<(String, Arc<Vec<f64>>)>>,
}

pub struct Session {
    tools: MediaTools,
    encoder: VideoEncoder,
    dir: PathBuf,
    path: PathBuf,
    media: MediaInfo,
    plan: StreamPlan,
    segments: Segments,
    run: tokio::sync::Mutex<RunState>,
    activity: Mutex<Activity>,
}

#[derive(Default)]
struct RunState {
    child: Option<Child>,
    /// The first segment the current run writes.
    first: usize,
    /// The lowest segment of the run not on disk yet.
    made: usize,
    paused: bool,
}

struct Activity {
    at: Instant,
    /// The last segment the player asked for.
    index: usize,
    /// Segments below this have been deleted.
    swept: usize,
}

impl Transcoder {
    /// Clears what a previous process left in `<data>/transcode` and starts
    /// the housekeeping task.
    pub fn new(tools: MediaTools, encoder: VideoEncoder, data_dir: &Path) -> Arc<Transcoder> {
        let root = data_dir.join("transcode");
        let _ = std::fs::remove_dir_all(&root);
        let transcoder = Arc::new(Transcoder {
            tools,
            encoder,
            root,
            sessions: Mutex::new(HashMap::new()),
            keyframes: Mutex::new(Vec::new()),
        });
        tokio::spawn(housekeeping(Arc::downgrade(&transcoder)));
        transcoder
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
                Segments::every(origin, duration)
            }
        };
        let dir = self.root.join(session_dir_name(&key));
        tokio::fs::create_dir_all(&dir).await?;
        let session = Arc::new(Session {
            tools: self.tools.clone(),
            encoder: self.encoder.clone(),
            dir,
            path: source.path,
            media: source.media,
            plan: key.plan,
            segments,
            run: tokio::sync::Mutex::new(RunState::default()),
            activity: Mutex::new(Activity {
                at: Instant::now(),
                index: 0,
                swept: 0,
            }),
        });
        let evicted = {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(existing) = sessions.get(&key) {
                // Another request opened it while this one worked.
                return Ok(existing.clone());
            }
            let mut mine: Vec<(SessionKey, Instant)> = sessions
                .iter()
                .filter(|(k, _)| k.user == key.user)
                .map(|(k, s)| (k.clone(), s.activity.lock().unwrap().at))
                .collect();
            mine.sort_by_key(|(_, at)| *at);
            let excess = (mine.len() + 1).saturating_sub(SESSIONS_PER_USER);
            let evicted: Vec<Arc<Session>> = mine
                .into_iter()
                .take(excess)
                .filter_map(|(k, _)| sessions.remove(&k))
                .collect();
            sessions.insert(key, session.clone());
            evicted
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
                        return Err(TranscodeError::Failed(self.log_tail().await));
                    }
                    _ => false,
                };
                self.advance(&mut run).await;
                let reachable = running
                    && run.first <= index
                    && self.segments.start(index)
                        <= self.segments.start(run.made.min(self.segments.len() - 1))
                            + CATCH_UP_SECONDS;
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
            tokio::time::sleep(Duration::from_millis(80)).await;
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
        }
        .args();
        let child = Command::new(self.tools.ffmpeg())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()?;
        tracing::debug!(dir = %self.dir.display(), first, ?seek, "transcode run started");
        *run = RunState {
            child: Some(child),
            first,
            made: first,
            paused: false,
        };
        Ok(())
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
    async fn keep_pace(&self) {
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
            if !run.paused && ahead > PAUSE_AHEAD {
                signal(&run, true);
                run.paused = true;
            } else if run.paused && ahead < RESUME_AHEAD {
                signal(&run, false);
                run.paused = false;
            }
        }
        drop(run);
        let behind = self.segments.start(index) - KEEP_BEHIND;
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
        let (idle, live): (Vec<_>, Vec<_>) = {
            let mut sessions = transcoder.sessions.lock().unwrap();
            let idle_keys: Vec<SessionKey> = sessions
                .iter()
                .filter(|(_, s)| s.activity.lock().unwrap().at.elapsed() > IDLE)
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
            session.keep_pace().await;
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
