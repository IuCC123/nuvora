//! Phase 2: pure planning. Given what is stored and what is on disk, decide what
//! to do. No I/O except through the fingerprint closure, so the whole
//! rename/move/removal policy is unit-tested with in-memory tables.

use std::time::SystemTime;

use super::walk::{DiskFile, RelPath, RootId, RootWalk};
use crate::{FileId, ItemId};

/// A stored `media_file` row, as much as planning needs.
#[derive(Debug, Clone)]
pub(crate) struct KnownFile {
    pub id: FileId,
    pub root: RootId,
    pub rel: RelPath,
    pub size: u64,
    pub mtime_ns: i64,
    pub fingerprint: [u8; 16],
    pub probe_version: u32,
}

/// A file to (re)read with ffprobe and upsert.
#[derive(Debug, Clone)]
pub(crate) struct ProbeJob {
    pub root: RootId,
    pub library_id: i64,
    pub disk: DiskFile,
    /// `Some` when the path is already stored (content changed, or
    /// `probe_version` is stale): the row is updated in place, id kept.
    pub existing: Option<FileId>,
}

#[derive(Debug, Clone)]
pub(crate) struct Move {
    pub id: FileId,
    pub to_root: RootId,
    pub to_library_id: i64,
    pub to: DiskFile,
}

#[derive(Debug, Default)]
pub(crate) struct ScanPlan {
    pub unchanged: u32,
    /// Modified within the last 60 s; probably still copying. Left for the next
    /// scan, and neither probed nor treated as gone.
    pub settling: u32,
    pub to_probe: Vec<ProbeJob>,
    pub moved: Vec<Move>,
    /// Stored files absent from a *completely* walked part of a root.
    pub gone: Vec<FileId>,
    /// Whether Clean up may delete `gone` files and mark emptied items missing.
    /// False when any root was unreadable, or when a readable root walked as
    /// empty while the database holds files for it: that is what an automounter
    /// mount point looks like when the share is down. Moves still apply.
    pub may_mark_missing: bool,
}

/// Diff stored files against walks. Rules:
/// - same path, same (size, mtime) and current probe_version => unchanged;
/// - same path, different (size, mtime) or stale probe_version => to_probe(existing);
/// - path only on disk => to_probe(new), candidate for move pairing;
/// - path only in store, under a complete root and not under an unreadable
///   subfolder => gone, candidate for move pairing; otherwise left untouched;
/// - `may_mark_missing` = every root complete, and no root with stored files
///   walked as empty.
/// Deterministic: inputs are sorted by (root, rel) first.
pub(crate) fn diff(known: Vec<KnownFile>, walks: &[RootWalk], now: SystemTime) -> ScanPlan {
    let _ = (known, walks, now);
    todo!()
}

/// Pair gone files with new files: only pairs with equal size are
/// fingerprinted (so a 2,000-file rescan with no moves reads nothing), and a
/// pair needs equal fingerprints. Each paired file moves from `gone` and
/// `to_probe` into `moved`. Ambiguous matches (two new files with one gone
/// file's fingerprint, i.e. a copy) pair the lexicographically first and treat
/// the rest as new.
pub(crate) fn pair_moves(
    plan: ScanPlan,
    known: &[KnownFile],
    fingerprint: impl FnMut(&ProbeJob) -> std::io::Result<[u8; 16]>,
) -> ScanPlan {
    let _ = (known, fingerprint);
    let _ = plan;
    todo!()
}

/// A local image file. Folder rule: generic names (`poster`, `folder`,
/// `cover` / `fanart`, `backdrop`, `background`) count only when the folder
/// holds exactly one item's videos; `<video stem>-poster` / `-fanart` always count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalImage {
    pub root: RootId,
    pub rel: RelPath,
    pub size: u64,
    pub mtime_ns: i64,
}

/// The artwork one present item should have. A poster is local or nothing (the
/// web app then draws a typographic card), so a frame poster is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WantedArtwork {
    pub item: ItemId,
    pub poster: Option<LocalImage>,
    pub backdrop: Option<WantedSource>,
}

/// Where a backdrop comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WantedSource {
    Local(LocalImage),
    /// No local backdrop: a frame at 10% of the runtime of the item's best SDR
    /// version. An item whose versions are all HDR gets no backdrop.
    Frame { file: FileId, fingerprint: [u8; 16], at_ms: u64 },
}

impl WantedSource {
    /// The `artwork.source_sig` string: equal sig => the stored render is current.
    pub(crate) fn sig(&self) -> String {
        todo!("'local:<root>:<rel>:<size>:<mtime>' | 'frame:<file>:<fp hex>:<at_ms>'")
    }
}

#[cfg(test)]
mod tests {
    //! Table tests over (stored rows, walk) -> plan:
    //! - unchanged library => only `unchanged`, no fingerprint calls;
    //! - file renamed in place => one Move, nothing gone, nothing probed;
    //! - folder renamed (`Alien/` -> `Alien (1979)/`) with poster => Move;
    //! - file replaced by a new encode at the same path => to_probe(existing);
    //! - root unreadable => nothing gone; subfolder unreadable => its files kept;
    //! - file mtime 10 s ago => settling, not probed, not gone;
    //! - running diff+pair twice on its own output state yields an empty plan.

    #[test]
    #[ignore = "sketch: enable with the implementation"]
    fn unreadable_root_blocks_missing_marks() {
        todo!("one root with complete = false and stored files => may_mark_missing == false")
    }

    #[test]
    #[ignore = "sketch: enable with the implementation"]
    fn empty_walk_of_populated_root_blocks_missing_marks() {
        todo!("stored files, root complete with no videos => may_mark_missing == false")
    }

    #[test]
    #[ignore = "sketch: enable with the implementation"]
    fn complete_walks_allow_missing_marks() {
        todo!("one stored file deleted from a non-empty complete root => gone == [it], may_mark_missing")
    }
}
