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
//!    (only when `plan.may_mark_missing`), delete artwork rows whose source
//!    vanished, GC the artwork store.
//!
//! Invariants that make it idempotent and crash-safe:
//! - Nothing is deleted until discovery of that root completed. An unreadable
//!   root (NAS offline) deletes nothing; an unreadable subfolder protects every
//!   stored file under it; a readable root that walks as empty while the
//!   database holds files for it deletes nothing (a share down behind an
//!   automounter).
//! - Every write that changes what clients see bumps `catalog.rev` in the
//!   same transaction.
//! - Every commit leaves a valid catalog. A crash mid-scan loses only work not
//!   yet committed; the next scan finds it as changed/new/gone again and
//!   converges. There is no "scan in progress" state on disk to recover.
//! - An unchanged library does: one walk, one SELECT of stored files, one SELECT
//!   of artwork sigs, zero writes. Target < 2 s for 2,000 files (walk-bound).

mod plan;
mod walk;
mod write;

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::SystemTime;

use nuvora_media::MediaTools;
use tokio::sync::{mpsc, watch};

use crate::db::Db;
use crate::store::ArtworkStore;

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
    Running { progress: ScanProgress, queued: bool },
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
    /// Files skipped because they were modified in the last 60 s (still copying).
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
}

impl ScanHandle {
    pub(crate) fn spawn(ctx: ScanContext) -> ScanHandle {
        let _ = ctx;
        todo!("mpsc::channel(1); watch::channel(Idle); tokio::spawn(worker(ctx, rx, tx))")
    }

    /// `try_send(())`: Ok => queued; Full => one is already queued, nothing to do.
    pub(crate) fn request(&self) -> ScanStatus {
        let _ = self.requests.try_send(());
        self.status()
    }

    pub(crate) fn status(&self) -> ScanStatus {
        self.status.borrow().clone()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<ScanStatus> {
        self.status.clone()
    }
}

/// Loop: wait for a request, run one scan, publish the summary. A scan that
/// fails outright (database gone) is logged and reported as a summary with one
/// issue; the worker keeps running.
async fn worker(ctx: ScanContext, mut requests: mpsc::Receiver<()>, status: watch::Sender<ScanStatus>) {
    while requests.recv().await.is_some() {
        let _ = (&ctx, &status);
        todo!("run(&ctx, &progress).await; status.send_modify(|s| s.last = Some(summary))")
    }
}

/// One full scan, phases 1-5 above. Pseudocode:
///
/// ```text
/// roots   = db.read(write::load_roots)
/// walks   = for each root: spawn_blocking(walk::walk_root(root))         -- phase Discovering
/// known   = db.read(write::load_known_files(roots))
/// plan    = plan::diff(known, &walks, now)                                -- pure
/// plan    = plan::pair_moves(plan, |f| walk::fingerprint(f))              -- reads 128 KiB per candidate
/// for m in plan.moved: db.write(write::move_file(m))
/// stream::iter(plan.to_probe)                                             -- phase Reading
///     .map(|f| async { (f, fingerprint(f), tools.probe(path)) })
///     .buffer_unordered(probe_concurrency)
///     .for_each(|r| db.write(write::upsert_file(r)); progress.tick())
/// wanted  = plan::wanted_artwork(&walks, db.read(write::load_artwork_sigs))  -- phase Artwork
/// for job in wanted.render (buffer_unordered(cpu/2)):           -- frame: grab_frame(at_ms), SDR only
///     id = ArtworkStore::id_for(role, source)
///     if !store.contains(id) { spawn_blocking(render); store.put(id, ..) }  -- files before row
///     db.write(write::set_artwork(item, role, source, sig, id, ..))
/// gone    = if plan.may_mark_missing { plan.gone } else { [] }
/// db.write(write::remove_gone(gone, wanted.stale_artwork, now))            -- phase CleaningUp
/// store.retain(db.read(write::live_artwork_ids))
/// ```
async fn run(ctx: &ScanContext, status: &watch::Sender<ScanStatus>) -> ScanSummary {
    let _ = (ctx, status);
    todo!()
}
