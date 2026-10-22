//! Content-addressed artwork files:
//! `<data>/artwork/<first 2 hex>/<32 hex>/{small,large}.webp`.
//!
//! The directory name is the [`ArtworkId`], derived from the source bytes, so:
//! - writing the same artwork twice is a no-op (idempotent rescans),
//! - a URL never changes meaning, so the server sends `immutable` cache headers,
//! - two movies with the same poster share files.
//! The `artwork` table is the only reference to an id; anything on disk not
//! referenced there is garbage and [`ArtworkStore::retain`] removes it.

use std::collections::HashSet;
use std::path::PathBuf;

use nuvora_media::{ArtworkRole, ArtworkSource, ImageSize, PIPELINE_VERSION, RenderedArtwork};

use crate::ArtworkId;

pub(crate) struct ArtworkStore {
    root: PathBuf,
}

impl ArtworkStore {
    pub(crate) fn new(root: PathBuf) -> std::io::Result<ArtworkStore> {
        let _ = root;
        todo!("create_dir_all; remove leftover `*.tmp-*` dirs from a crash")
    }

    /// The id a source will be stored under. Pure; computed before rendering so
    /// an already-stored id skips the (expensive) render entirely.
    pub(crate) fn id_for(role: ArtworkRole, source: &ArtworkSource<'_>) -> ArtworkId {
        let _ = (role, source, PIPELINE_VERSION);
        todo!("blake3(PIPELINE_VERSION.to_le_bytes() || role byte || source.hash_input())[..16]")
    }

    pub(crate) fn contains(&self, id: ArtworkId) -> bool {
        let _ = id;
        todo!("large.webp exists (it is renamed into place last)")
    }

    /// Write every size into `<id>.tmp-<rand>/`, fsync, then rename the
    /// directory to `<id>/`. A crash leaves only a tmp dir (swept on startup),
    /// never a half-written id. If `<id>/` already exists, drop the tmp dir.
    pub(crate) fn put(&self, id: ArtworkId, rendered: &RenderedArtwork) -> std::io::Result<()> {
        let _ = (id, rendered);
        todo!()
    }

    pub(crate) fn existing_path(&self, id: ArtworkId, size: ImageSize) -> Option<PathBuf> {
        let _ = (id, size);
        todo!()
    }

    /// Delete stored ids not in `live`. Run at the end of a completed scan with
    /// every hash in the `artwork` table. Returns how many were removed.
    pub(crate) fn retain(&self, live: &HashSet<ArtworkId>) -> std::io::Result<usize> {
        let _ = live;
        todo!()
    }
}
