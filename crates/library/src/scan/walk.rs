//! Phase 1: walk one root. Blocking I/O; runs in `spawn_blocking`.

use std::path::{Path, PathBuf};

use nuvora_media::PathClass;

/// A `/`-separated path relative to a library root. Stored as-is in
/// `media_file.rel_path`. Non-UTF-8 names are reported as issues and skipped.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RelPath(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RootId(pub i64);

#[derive(Debug, Clone)]
pub(crate) struct DiskFile {
    pub rel: RelPath,
    pub size: u64,
    pub mtime_ns: i64,
}

/// Result of walking one root.
#[derive(Debug)]
pub(crate) struct RootWalk {
    pub root: RootId,
    pub library_id: i64,
    pub abs: PathBuf,
    /// False if the root itself could not be read (unmounted share). The planner
    /// then treats every stored file of this root as unknown, never as gone.
    pub complete: bool,
    /// Subfolders that failed to list. Stored files under them are unknown.
    pub unreadable: Vec<RelPath>,
    /// `PathClass::Video` entries.
    pub videos: Vec<DiskFile>,
    /// `PathClass::Image` entries, for local artwork.
    pub images: Vec<DiskFile>,
    /// Non-UTF-8 names and other per-entry errors.
    pub issues: Vec<(String, String)>,
}

/// `walkdir` with `follow_links(true)` (NAS libraries are often symlink farms)
/// and loop detection; directories classified as extras are pruned, not descended.
pub(crate) fn walk_root(root: RootId, library_id: i64, abs: &Path) -> RootWalk {
    let _ = (root, library_id, abs, PathClass::Other);
    todo!()
}

/// blake3(size || first 64 KiB || last 64 KiB), truncated to 16 bytes. Survives
/// renames and moves across roots; changes when content changes. Two reads per
/// file, so it is cheap on a NAS too.
pub(crate) fn fingerprint(abs: &Path, size: u64) -> std::io::Result<[u8; 16]> {
    let _ = (abs, size);
    todo!()
}
