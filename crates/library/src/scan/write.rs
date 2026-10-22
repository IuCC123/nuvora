//! Every catalog write the scan makes, one function per transaction. Each runs
//! inside `Db::write`, so each is atomic; each is safe to repeat.

use nuvora_media::{ArtworkRole, MediaInfo, MovieName, ProbeError, RenderedArtwork};
use rusqlite::{Connection, Transaction};

use super::plan::{KnownFile, Move, ProbeJob, WantedSource};
use super::walk::RootId;
use crate::{ArtworkId, FileId, ItemId};

pub(crate) struct RootRow {
    pub id: RootId,
    pub library_id: i64,
    pub path: std::path::PathBuf,
}

pub(crate) fn load_roots(conn: &Connection) -> rusqlite::Result<Vec<RootRow>> {
    let _ = conn;
    todo!()
}

/// One SELECT over `media_file` for the given roots.
pub(crate) fn load_known_files(conn: &Connection, roots: &[RootId]) -> rusqlite::Result<Vec<KnownFile>> {
    let _ = (conn, roots);
    todo!()
}

/// Insert or update one probed file and everything derived from it:
/// 1. find-or-create the item by `(library_id, name.match_key())`, clearing
///    `missing_since` (a returning file revives its old item and id);
/// 2. upsert the `media_file` row (by `existing` id, else by `(root, rel_path)`
///    so a crash-and-retry of the same insert updates instead of duplicating);
/// 3. replace its `media_stream` rows (or set `probe_error` on failure);
/// 4. rewrite the item's `filename` metadata layer from the item's *oldest*
///    file, so which version was probed last never changes the title.
/// If the file moved to a different item (its name now parses differently),
/// the old item is marked missing when it has no files left.
pub(crate) fn upsert_file(
    tx: &Transaction<'_>,
    job: &ProbeJob,
    fingerprint: [u8; 16],
    name: &MovieName,
    probe: &Result<MediaInfo, ProbeError>,
    now_ms: i64,
) -> rusqlite::Result<FileId> {
    let _ = (tx, job, fingerprint, name, probe, now_ms);
    todo!()
}

/// Apply a rename/move: update `root_id`, `rel_path`, `mtime_ns`; keep the id,
/// streams and artwork. Then re-derive the item from the new name:
/// - same match key => nothing else changes;
/// - new key owned by another item => re-parent the file (becomes a version
///   there); the old item is marked missing if now empty;
/// - new key unowned and the file is its item's only file => re-key the item in
///   place and rewrite its filename layer (the user fixed a typo; the id survives);
/// - otherwise => create the new item and re-parent.
pub(crate) fn move_file(tx: &Transaction<'_>, mv: &Move, name: &MovieName, now_ms: i64) -> rusqlite::Result<()> {
    let _ = (tx, mv, name, now_ms);
    todo!()
}

/// Stored artwork state for planning: `(item, role, source) -> source_sig`.
pub(crate) fn load_artwork_sigs(conn: &Connection) -> rusqlite::Result<Vec<(ItemId, ArtworkRole, String)>> {
    let _ = conn;
    todo!()
}

/// Upsert one `artwork` row. Called only after the store holds `id`'s files.
pub(crate) fn set_artwork(
    tx: &Transaction<'_>,
    item: ItemId,
    source: &WantedSource,
    id: ArtworkId,
    rendered: &RenderedArtwork,
) -> rusqlite::Result<()> {
    let _ = (tx, item, source, id, rendered);
    todo!()
}

/// Delete gone files (streams cascade), mark items with no remaining files
/// missing (`missing_since = now` where NULL), and delete artwork rows whose
/// source is gone. Idempotent: deleting a deleted id is a no-op.
pub(crate) fn remove_gone(
    tx: &Transaction<'_>,
    gone: &[FileId],
    stale_artwork: &[(ItemId, ArtworkRole, &'static str)],
    now_ms: i64,
) -> rusqlite::Result<()> {
    let _ = (tx, gone, stale_artwork, now_ms);
    todo!()
}

pub(crate) fn live_artwork_ids(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<ArtworkId>> {
    let _ = conn;
    todo!()
}
