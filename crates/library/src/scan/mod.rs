//! The scan: make the catalog match the disk. The only writer of catalog content.
//!
//! One worker task runs scans one at a time; requests coalesce through a
//! capacity-1 channel. Each scan:
//!
//! 1. **Discover** (`walk`): walk every root, stat every entry, classify by name.
//! 2. **Plan** (`plan`, pure): diff the walk against stored files by
//!    `(root, rel_path)` and `(size, mtime_ns)` into unchanged / changed / new /
//!    gone, then pair gone files with new files of equal size and fingerprint
//!    (= moved or renamed; the file id and item id survive).
//! 3. **Read**: ffprobe changed and new files, at most `probe_concurrency` at a
//!    time; each result commits in its own transaction (`write::upsert_file`).
//! 4. **Artwork**: per present item, pick the desired sources (poster: local
//!    file only; backdrop: local file, else an SDR frame grab); render only when
//!    the stored `source_sig` differs.
//! 5. **Clean up**: delete rows of gone files and mark emptied items missing
//!    (only under roots the plan did not hold back), delete artwork rows whose
//!    source vanished, GC the artwork store.
//!
//! Invariants that make it idempotent and crash-safe:
//! - Nothing is deleted until discovery of that root completed. An unreadable
//!   root (NAS offline) deletes nothing under it; an unreadable subfolder
//!   protects every stored file under it; a readable root that walks as empty
//!   while the database holds files for it deletes nothing under it (a share
//!   down behind an automounter). Each root is judged alone.
//! - Every write that changes what clients see bumps `catalog.rev` in the
//!   same transaction.
//! - Every commit leaves a valid catalog. A crash mid-scan loses only work not
//!   yet committed; the next scan finds it as changed/new/gone again and
//!   converges. There is no "scan in progress" state on disk to recover.
//! - An unchanged library does: one walk, one SELECT of stored files, one SELECT
//!   of artwork sigs, zero writes. Target < 2 s for 2,000 files (walk-bound).

mod plan;
mod walk;
pub(crate) mod write;

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use nuvora_media::{
    ArtworkRole, ArtworkSource, MediaInfo, MediaTools, MovieName, ProbeError, RenderedArtwork,
};
use tokio::sync::{mpsc, watch};

use crate::ArtworkId;
use crate::db::{Db, DbError};
use crate::store::ArtworkStore;
use plan::{RenderJob, WantedSource};
use walk::{RootId, RootWalk};

/// Latest scan state plus the summary of the last finished scan.
#[derive(Debug, Clone)]
pub struct ScanStatus {
    pub state: ScanState,
    /// Survives only in memory; after a restart it is `None` until a scan ends.
    pub last: Option<ScanSummary>,
}

#[derive(Debug, Clone)]
pub enum ScanState {
    Idle,
    /// Running, with another scan already queued behind it when `queued`.
    Running {
        progress: ScanProgress,
        queued: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    /// Units finished / total in the current phase (files, or items for artwork).
    pub done: u32,
    pub total: u32,
    pub started_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Discovering,
    Reading,
    Artwork,
    CleaningUp,
}

#[derive(Debug, Clone)]
pub struct ScanSummary {
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub added: u32,
    pub changed: u32,
    pub moved: u32,
    pub removed: u32,
    pub unchanged: u32,
    /// Files skipped because they were modified within the settle window (still copying).
    pub settling: u32,
    /// Capped at 100; `failed` is the true count.
    pub failed: u32,
    pub issues: Vec<ScanIssue>,
}

/// A file the scan could not handle, with a reason a user can act on.
#[derive(Debug, Clone)]
pub struct ScanIssue {
    /// Relative to its library root.
    pub path: String,
    pub message: String,
}

const MAX_ISSUES: usize = 100;

/// Owned by `Library`. Requests go in through `requests`; status comes out of
/// `status`. The worker is the only `watch::Sender`: one owner for scan state.
pub(crate) struct ScanHandle {
    requests: mpsc::Sender<()>,
    status: watch::Receiver<ScanStatus>,
}

/// What the worker needs. All fields are handles; nothing here is mutated.
pub(crate) struct ScanContext {
    pub db: Db,
    pub store: Arc<ArtworkStore>,
    pub tools: MediaTools,
    pub probe_concurrency: NonZeroUsize,
    /// Files modified more recently than this are left for the next scan.
    pub settle_window: Duration,
}

impl ScanHandle {
    pub(crate) fn spawn(ctx: ScanContext) -> ScanHandle {
        let (requests, rx) = mpsc::channel(1);
        let (tx, status) = watch::channel(ScanStatus {
            state: ScanState::Idle,
            last: None,
        });
        tokio::spawn(worker(ctx, rx, requests.downgrade(), tx));
        ScanHandle { requests, status }
    }

    /// `try_send(())`: Ok => queued; Full => one is already queued, nothing to do.
    pub(crate) fn request(&self) -> ScanStatus {
        let _ = self.requests.try_send(());
        self.status()
    }

    /// `queued` is read from the request channel: a request still sitting in
    /// it while a scan runs is the queued follow-up.
    pub(crate) fn status(&self) -> ScanStatus {
        let mut status = self.status.borrow().clone();
        if let ScanState::Running { queued, .. } = &mut status.state {
            *queued = self.requests.capacity() == 0;
        }
        status
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<ScanStatus> {
        self.status.clone()
    }
}

/// Loop: wait for a request, run one scan, publish the summary. A scan that
/// fails outright (database gone) is logged and reported as a summary with one
/// issue; the worker keeps running. A scan that skipped settling files asks for
/// one more scan once the window has passed, so a finished copy shows up
/// without anyone pressing Scan. `resend` is weak so the worker ends with its
/// `Library`.
async fn worker(
    ctx: ScanContext,
    mut requests: mpsc::Receiver<()>,
    resend: mpsc::WeakSender<()>,
    status: watch::Sender<ScanStatus>,
) {
    while requests.recv().await.is_some() {
        let progress = Progress {
            status: &status,
            started_at: SystemTime::now(),
        };
        progress.set(ScanPhase::Discovering, 0, 0);
        let summary = run(&ctx, &progress).await;
        tracing::info!(
            added = summary.added,
            changed = summary.changed,
            moved = summary.moved,
            removed = summary.removed,
            unchanged = summary.unchanged,
            settling = summary.settling,
            failed = summary.failed,
            "scan finished"
        );
        if summary.settling > 0 {
            let resend = resend.clone();
            let delay = ctx.settle_window + Duration::from_secs(1);
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                if let Some(requests) = resend.upgrade() {
                    let _ = requests.try_send(());
                }
            });
        }
        status.send_modify(|s| {
            s.state = ScanState::Idle;
            s.last = Some(summary);
        });
    }
}

struct Progress<'a> {
    status: &'a watch::Sender<ScanStatus>,
    started_at: SystemTime,
}

impl Progress<'_> {
    fn set(&self, phase: ScanPhase, done: usize, total: usize) {
        let progress = ScanProgress {
            phase,
            done: done as u32,
            total: total as u32,
            started_at: self.started_at,
        };
        self.status.send_modify(|s| {
            s.state = ScanState::Running {
                progress,
                queued: false,
            }
        });
    }
}

/// Counters and issues accumulated during one scan.
struct Tally {
    summary: ScanSummary,
}

impl Tally {
    fn new(started_at: SystemTime) -> Tally {
        Tally {
            summary: ScanSummary {
                started_at,
                finished_at: started_at,
                added: 0,
                changed: 0,
                moved: 0,
                removed: 0,
                unchanged: 0,
                settling: 0,
                failed: 0,
                issues: Vec::new(),
            },
        }
    }

    fn issue(&mut self, path: impl Into<String>, message: impl Into<String>) {
        let (path, message) = (path.into(), message.into());
        tracing::warn!(path, "{message}");
        if self.summary.issues.len() < MAX_ISSUES {
            self.summary.issues.push(ScanIssue { path, message });
        }
    }

    fn failed(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.summary.failed += 1;
        self.issue(path, message);
    }
}

/// One full scan, phases 1-5 above. Never fails: an error that stops the scan
/// becomes the summary's one issue.
async fn run(ctx: &ScanContext, progress: &Progress<'_>) -> ScanSummary {
    let mut tally = Tally::new(progress.started_at);
    if let Err(e) = scan(ctx, progress, &mut tally).await {
        tally.failed("", format!("scan aborted: {e}"));
    }
    tally.summary.finished_at = SystemTime::now();
    tally.summary
}

async fn scan(
    ctx: &ScanContext,
    progress: &Progress<'_>,
    tally: &mut Tally,
) -> Result<(), DbError> {
    let now = SystemTime::now();
    let now_ms = unix_ms(now);

    let roots = ctx.db.read(write::load_roots).await?;
    progress.set(ScanPhase::Discovering, 0, roots.len());
    let walks = futures::future::join_all(roots.iter().map(|root| {
        let (id, library_id, path) = (root.id, root.library_id, root.path.clone());
        async move {
            let walked = path.clone();
            tokio::task::spawn_blocking(move || walk::walk_root(id, library_id, &walked))
                .await
                .unwrap_or_else(|_| RootWalk::empty(id, library_id, &path, false))
        }
    }))
    .await;
    progress.set(ScanPhase::Discovering, roots.len(), roots.len());
    for walk in &walks {
        for (path, message) in &walk.issues {
            tally.issue(path.clone(), message.clone());
        }
    }
    let root_abs: HashMap<RootId, PathBuf> =
        walks.iter().map(|w| (w.root, w.abs.clone())).collect();

    let root_ids: Vec<RootId> = roots.iter().map(|r| r.id).collect();
    let known = ctx
        .db
        .read(move |c| write::load_known_files(c, &root_ids))
        .await?;
    let plan = plan::diff(known.clone(), &walks, now, ctx.settle_window);
    let abs_for_pairing = root_abs.clone();
    let plan = tokio::task::spawn_blocking(move || {
        plan::pair_moves(plan, &known, |job| {
            walk::fingerprint(
                &abs_for_pairing[&job.root].join(&job.disk.rel.0),
                job.disk.size,
            )
        })
    })
    .await
    .map_err(|_| DbError::Panicked)?;
    tally.summary.unchanged = plan.unchanged;
    tally.summary.settling = plan.settling;
    tally.summary.moved = plan.moved.len() as u32;
    for rel in &plan.future {
        tally.issue(
            rel.0.clone(),
            "modified time is in the future; check the clock of the device that wrote it",
        );
    }
    for root in &plan.held_roots {
        tally.issue(
            root_abs[root].to_string_lossy(),
            "folder is unreadable or empty; nothing under it was removed",
        );
    }

    for mv in plan.moved {
        let name = MovieName::parse(Path::new(&mv.to.rel.0));
        let path = mv.to.rel.0.clone();
        let moved = ctx
            .db
            .write(move |tx| write::move_file(tx, &mv, &name, now_ms))
            .await;
        if let Err(message) = orphaned(moved)? {
            tally.failed(path, message);
        }
    }

    let total = plan.to_probe.len();
    progress.set(ScanPhase::Reading, 0, total);
    let mut probed = futures::stream::iter(plan.to_probe)
        .map(|job| probe_job(ctx, &root_abs, job))
        .buffer_unordered(ctx.probe_concurrency.get());
    let mut done = 0;
    while let Some((job, fingerprint, probe)) = probed.next().await {
        let path = job.disk.rel.0.clone();
        let existing = job.existing.is_some();
        let probe_error = probe.as_ref().err().map(ToString::to_string);
        let name = MovieName::parse(Path::new(&job.disk.rel.0));
        let written = ctx
            .db
            .write(move |tx| write::upsert_file(tx, &job, fingerprint, &name, &probe, now_ms))
            .await;
        match (orphaned(written)?, probe_error) {
            (Err(message), _) => tally.failed(path, message),
            (Ok(_), probe_error) => {
                if let Some(message) = probe_error {
                    tally.failed(path, message);
                }
                if existing {
                    tally.summary.changed += 1;
                } else {
                    tally.summary.added += 1;
                }
            }
        }
        done += 1;
        progress.set(ScanPhase::Reading, done, total);
    }
    drop(probed);

    let stored = ctx.db.read(write::load_artwork_sigs).await?;
    let items = ctx.db.read(write::load_present_items).await?;
    let wanted = plan::wanted_artwork(&items, &walks, &plan.held_roots);
    let (render, stale) = plan::artwork_diff(&wanted, &stored);
    let total = render.len();
    progress.set(ScanPhase::Artwork, 0, total);
    let render_concurrency = std::thread::available_parallelism()
        .map_or(1, |n| n.get() / 2)
        .max(1);
    let mut rendered = futures::stream::iter(render)
        .map(|job| render_job(ctx, &root_abs, job))
        .buffer_unordered(render_concurrency);
    let mut done = 0;
    while let Some(result) = rendered.next().await {
        match result {
            Ok((job, id, art)) => {
                let path = job_path(&job);
                let set = ctx
                    .db
                    .write(move |tx| {
                        write::set_artwork(tx, job.item, job.role, &job.source, id, &art)
                    })
                    .await;
                if let Err(message) = orphaned(set)? {
                    tally.issue(path, message);
                }
            }
            Err((job, message)) => tally.issue(job_path(&job), message),
        }
        done += 1;
        progress.set(ScanPhase::Artwork, done, total);
    }
    drop(rendered);

    progress.set(ScanPhase::CleaningUp, 0, 1);
    tally.summary.removed = plan.gone.len() as u32;
    if !plan.gone.is_empty() || !stale.is_empty() {
        let gone = plan.gone;
        ctx.db
            .write(move |tx| write::remove_gone(tx, &gone, &stale, now_ms))
            .await?;
    }
    let live = ctx.db.read(write::live_artwork_ids).await?;
    let store = Arc::clone(&ctx.store);
    match tokio::task::spawn_blocking(move || store.retain(&live)).await {
        Ok(Ok(removed)) if removed > 0 => {
            tracing::info!(removed, "artwork store garbage collected")
        }
        Ok(Ok(_)) => {}
        Ok(Err(e)) => tally.issue("", format!("artwork store cleanup failed: {e}")),
        Err(_) => tally.issue("", "artwork store cleanup panicked"),
    }
    progress.set(ScanPhase::CleaningUp, 1, 1);
    Ok(())
}

/// Fingerprint and probe one file. A fingerprint read failure is reported as
/// the probe's failure so the file is stored (with `probe_error`) and not
/// retried until it changes.
async fn probe_job(
    ctx: &ScanContext,
    root_abs: &HashMap<RootId, PathBuf>,
    job: plan::ProbeJob,
) -> (plan::ProbeJob, [u8; 16], Result<MediaInfo, ProbeError>) {
    let abs = root_abs[&job.root].join(&job.disk.rel.0);
    let size = job.disk.size;
    let fp_path = abs.clone();
    let fingerprint = tokio::task::spawn_blocking(move || walk::fingerprint(&fp_path, size)).await;
    match fingerprint {
        Ok(Ok(fingerprint)) => {
            let probe = nuvora_media::probe(&ctx.tools, &abs).await;
            (job, fingerprint, probe)
        }
        Ok(Err(e)) => (job, [0; 16], Err(ProbeError::Io(e))),
        Err(_) => (
            job,
            [0; 16],
            Err(ProbeError::Unparseable("fingerprint task panicked".into())),
        ),
    }
}

/// Render one artwork and put it in the store. The row is written by the caller.
async fn render_job(
    ctx: &ScanContext,
    root_abs: &HashMap<RootId, PathBuf>,
    job: RenderJob,
) -> Result<(RenderJob, ArtworkId, RenderedArtwork), (RenderJob, String)> {
    let store = Arc::clone(&ctx.store);
    let role = job.role;
    let result = match &job.source {
        WantedSource::Local(image) => {
            let path = root_abs[&image.root].join(&image.rel.0);
            match tokio::fs::read(&path).await {
                Ok(bytes) => {
                    tokio::task::spawn_blocking(move || {
                        render_and_store(&store, role, ArtworkSource::File(&bytes))
                    })
                    .await
                }
                Err(e) => Ok(Err(format!("cannot read artwork: {e}"))),
            }
        }
        WantedSource::Frame {
            root, rel, at_ms, ..
        } => {
            let path = root_abs[root].join(&rel.0);
            match ctx
                .tools
                .grab_frame(&path, Duration::from_millis(*at_ms))
                .await
            {
                Ok(frame) => {
                    tokio::task::spawn_blocking(move || {
                        render_and_store(&store, role, ArtworkSource::Frame(&frame))
                    })
                    .await
                }
                Err(e) => Ok(Err(e.to_string())),
            }
        }
    };
    match result {
        Ok(Ok((id, art))) => Ok((job, id, art)),
        Ok(Err(message)) => Err((job, message)),
        Err(_) => Err((job, "render task panicked".into())),
    }
}

fn render_and_store(
    store: &ArtworkStore,
    role: ArtworkRole,
    source: ArtworkSource<'_>,
) -> Result<(ArtworkId, RenderedArtwork), String> {
    let id = ArtworkStore::id_for(role, &source);
    let art = RenderedArtwork::render(role, source).map_err(|e| e.to_string())?;
    if !store.contains(id) {
        store
            .put(id, &art)
            .map_err(|e| format!("cannot store artwork: {e}"))?;
    }
    Ok((id, art))
}

/// A per-file write whose library, folder or item was deleted while the scan
/// ran (the admin panel removed it) fails that file only. Any other database
/// error stops the scan.
fn orphaned<T>(written: Result<T, DbError>) -> Result<Result<T, String>, DbError> {
    use rusqlite::{Error, ffi};
    match written {
        Ok(value) => Ok(Ok(value)),
        Err(DbError::Sqlite(e))
            if matches!(&e, Error::QueryReturnedNoRows)
                || matches!(&e, Error::SqliteFailure(f, _)
                    if f.extended_code == ffi::SQLITE_CONSTRAINT_FOREIGNKEY) =>
        {
            Ok(Err(
                "its library or folder was removed during the scan".into()
            ))
        }
        Err(e) => Err(e),
    }
}

fn job_path(job: &RenderJob) -> String {
    match &job.source {
        WantedSource::Local(image) => image.rel.0.clone(),
        WantedSource::Frame { rel, .. } => rel.0.clone(),
    }
}

pub(crate) fn unix_ms(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
