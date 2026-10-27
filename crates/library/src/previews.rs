//! Seek previews (see `nuvora_media::PreviewSheets`), made in the background:
//! after each scan, every video without them, newest first, one at a time so
//! playback and scans keep most of the machine. A video someone opens in the
//! player goes to the front. The admin panel can switch the worker off; what
//! it is doing is published as [`PreviewActivity`].
//!
//! Stored by content fingerprint, like artwork: a renamed or moved file keeps
//! its previews and identical files share them. One folder per video,
//! `<data>/previews/<fingerprint>-v<version>/`, holds `0.jpg`, `1.jpg`, ...
//! and `sheets.json`, written last: a folder without it is unfinished.

use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use nuvora_media::{MediaTools, PREVIEW_VERSION, PreviewError, PreviewSheets};
use rusqlite::{Connection, OptionalExtension};
use tokio::sync::{mpsc, watch};

use crate::db::Db;
use crate::{FileId, ParseIdError, ScanState, ScanStatus};

const SHEETS_FILE: &str = "sheets.json";
const UNFINISHED: &str = ".unfinished-";

/// Names one video's previews: its fingerprint and the preview version,
/// `"<32 hex>-v1"`. Parsed before any path is built from a URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreviewKey(String);

impl PreviewKey {
    fn of(fingerprint: &[u8]) -> PreviewKey {
        let hex: String = fingerprint.iter().map(|b| format!("{b:02x}")).collect();
        PreviewKey(format!("{hex}-v{PREVIEW_VERSION}"))
    }
}

impl FromStr for PreviewKey {
    type Err = ParseIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (hex, version) = s.split_once("-v").ok_or(ParseIdError)?;
        let hex_ok = hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let version_ok = !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit());
        if hex_ok && version_ok {
            Ok(PreviewKey(s.to_owned()))
        } else {
            Err(ParseIdError)
        }
    }
}

impl fmt::Display for PreviewKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A video's finished previews.
#[derive(Debug, Clone)]
pub struct Previews {
    pub key: PreviewKey,
    pub sheets: PreviewSheets,
}

pub(crate) struct PreviewStore {
    root: PathBuf,
}

impl PreviewStore {
    pub(crate) fn new(root: PathBuf) -> std::io::Result<PreviewStore> {
        std::fs::create_dir_all(&root)?;
        Ok(PreviewStore { root })
    }

    pub(crate) fn sheets(&self, key: &PreviewKey) -> Option<PreviewSheets> {
        let text = std::fs::read(self.root.join(&key.0).join(SHEETS_FILE)).ok()?;
        serde_json::from_slice(&text).ok()
    }

    pub(crate) fn sheet(&self, key: &PreviewKey, n: u32) -> Option<PathBuf> {
        let path = self.root.join(&key.0).join(format!("{n}.jpg"));
        path.is_file().then_some(path)
    }

    /// Renders into a scratch folder and moves it into place whole, so a crash
    /// or a failed render never leaves a folder that looks finished.
    async fn render(&self, tools: &MediaTools, job: &Job) -> Result<PreviewSheets, PreviewError> {
        let scratch = self.root.join(format!("{UNFINISHED}{}", job.key));
        let _ = tokio::fs::remove_dir_all(&scratch).await;
        tokio::fs::create_dir(&scratch).await?;
        let rendered = tools
            .render_previews(&job.path, job.duration_ms, &scratch)
            .await;
        let sheets = match rendered {
            Ok(sheets) => sheets,
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&scratch).await;
                return Err(e);
            }
        };
        let json = serde_json::to_vec(&sheets).expect("sheets serialize");
        tokio::fs::write(scratch.join(SHEETS_FILE), json).await?;
        let done = self.root.join(&job.key.0);
        let _ = tokio::fs::remove_dir_all(&done).await;
        tokio::fs::rename(&scratch, &done).await?;
        Ok(sheets)
    }

    /// Deletes previews of videos no longer in the catalog, of older preview
    /// versions, and unfinished ones. Returns how many folders went.
    fn retain(&self, live: &HashSet<PreviewKey>) -> std::io::Result<usize> {
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let keep = name
                .to_str()
                .and_then(|n| n.parse::<PreviewKey>().ok())
                .is_some_and(|k| live.contains(&k));
            if !keep && entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// One video to make previews for.
struct Job {
    key: PreviewKey,
    path: PathBuf,
    duration_ms: u64,
}

/// Videos that can have previews: probed, with a length and a video stream.
const VIDEOS: &str = "SELECT r.path, f.rel_path, f.fingerprint, f.duration_ms \
     FROM media_file f JOIN library_root r ON r.id = f.root_id \
     WHERE f.probe_error IS NULL AND f.duration_ms > 0 \
     AND EXISTS (SELECT 1 FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'video')";

fn job_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let root: String = r.get(0)?;
    let rel: String = r.get(1)?;
    let fingerprint: Vec<u8> = r.get(2)?;
    Ok(Job {
        key: PreviewKey::of(&fingerprint),
        path: PathBuf::from(root).join(rel),
        duration_ms: r.get::<_, i64>(3)? as u64,
    })
}

fn job(conn: &Connection, file: FileId) -> rusqlite::Result<Option<Job>> {
    conn.prepare_cached(&format!("{VIDEOS} AND f.id = ?1"))?
        .query_row([file.0], job_row)
        .optional()
}

fn all_jobs(conn: &Connection) -> rusqlite::Result<Vec<Job>> {
    conn.prepare_cached(&format!("{VIDEOS} ORDER BY f.added_at DESC, f.id DESC"))?
        .query_map([], job_row)?
        .collect()
}

pub(crate) fn key_of(conn: &Connection, file: FileId) -> rusqlite::Result<Option<PreviewKey>> {
    Ok(job(conn, file)?.map(|j| j.key))
}

/// What the seek-preview worker is doing.
#[derive(Debug, Clone, Default)]
pub struct PreviewActivity {
    /// Off in the admin panel: nothing new is made.
    pub enabled: bool,
    /// The video being worked on.
    pub current: Option<PathBuf>,
    /// Videos waiting after it.
    pub queued: usize,
    /// Since the server started.
    pub made: u32,
    pub failed: u32,
}

pub(crate) struct PreviewHandle {
    pub(crate) store: Arc<PreviewStore>,
    urgent: mpsc::UnboundedSender<FileId>,
    enabled: watch::Sender<bool>,
    activity: watch::Receiver<PreviewActivity>,
}

impl PreviewHandle {
    pub(crate) fn spawn(
        db: Db,
        store: Arc<PreviewStore>,
        tools: MediaTools,
        mut scans: watch::Receiver<ScanStatus>,
    ) -> PreviewHandle {
        let (urgent, mut requests) = mpsc::unbounded_channel::<FileId>();
        let (enabled, mut switch) = watch::channel(true);
        let (publish, activity) = watch::channel(PreviewActivity {
            enabled: true,
            ..PreviewActivity::default()
        });
        let worker_store = Arc::clone(&store);
        tokio::spawn(async move {
            let store = worker_store;
            let mut asked = VecDeque::new();
            let mut backlog = VecDeque::new();
            // A video FFmpeg can't read is tried again after a restart, not after every scan.
            let mut failed = HashSet::new();
            let mut last_scan = scans.borrow().last.as_ref().map(|s| s.finished_at);
            loop {
                while let Ok(file) = requests.try_recv() {
                    asked.push_back(file);
                }
                let on = *switch.borrow_and_update();
                let next = match asked.pop_front() {
                    _ if !on => None,
                    Some(file) => db.read(move |c| job(c, file)).await.ok().flatten(),
                    None => backlog.pop_front(),
                };
                if let Some(job) = next {
                    let wanted = store.sheets(&job.key).is_none() && !failed.contains(&job.key);
                    if wanted {
                        let queued = asked.len() + backlog.len();
                        publish.send_modify(|a| {
                            a.current = Some(job.path.clone());
                            a.queued = queued;
                        });
                        match store.render(&tools, &job).await {
                            Ok(_) => publish.send_modify(|a| a.made += 1),
                            Err(e) => {
                                tracing::warn!(path = %job.path.display(), "no seek previews: {e}");
                                failed.insert(job.key);
                                publish.send_modify(|a| a.failed += 1);
                            }
                        }
                    }
                    continue;
                }
                let queued = asked.len() + backlog.len();
                publish.send_modify(|a| {
                    a.enabled = on;
                    a.current = None;
                    a.queued = queued;
                });
                tokio::select! {
                    request = requests.recv() => match request {
                        Some(file) => asked.push_back(file),
                        None => return,
                    },
                    changed = switch.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        if *switch.borrow() {
                            let Ok(jobs) = db.read(all_jobs).await else { continue };
                            backlog = jobs.into_iter().filter(|j| store.sheets(&j.key).is_none()).collect();
                        }
                    },
                    changed = scans.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        let finished = {
                            let status = scans.borrow();
                            match status.state {
                                ScanState::Idle => status.last.as_ref().map(|s| s.finished_at),
                                ScanState::Running { .. } => None,
                            }
                        };
                        if finished.is_none() || finished == last_scan {
                            continue;
                        }
                        last_scan = finished;
                        let Ok(jobs) = db.read(all_jobs).await else { continue };
                        let live: HashSet<_> = jobs.iter().map(|j| j.key.clone()).collect();
                        match store.retain(&live) {
                            Ok(0) => {}
                            Ok(removed) => tracing::info!(removed, "old seek previews deleted"),
                            Err(e) => tracing::warn!("cannot tidy seek previews: {e}"),
                        }
                        backlog = jobs.into_iter().filter(|j| store.sheets(&j.key).is_none()).collect();
                    }
                }
            }
        });
        PreviewHandle {
            store,
            urgent,
            enabled,
            activity,
        }
    }

    pub(crate) fn set_enabled(&self, on: bool) {
        self.enabled
            .send_if_modified(|was| std::mem::replace(was, on) != on);
    }

    pub(crate) fn activity(&self) -> PreviewActivity {
        self.activity.borrow().clone()
    }

    /// Puts `file` at the front of the queue.
    pub(crate) fn hurry(&self, file: FileId) {
        let _ = self.urgent.send(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_only_as_written() {
        let key = PreviewKey::of(&[0xab; 16]);
        assert_eq!(
            key.to_string(),
            format!("{}-v{PREVIEW_VERSION}", "ab".repeat(16))
        );
        assert_eq!(key.to_string().parse::<PreviewKey>().unwrap(), key);
        for bad in [
            "",
            "abc-v1",
            &format!("{}-v", "ab".repeat(16)),
            &format!("{}-v1", "AB".repeat(16)),
            &format!("../{}-v1", "a".repeat(29)),
        ] {
            assert!(bad.parse::<PreviewKey>().is_err(), "{bad:?}");
        }
    }
}
