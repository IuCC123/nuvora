//! Content-addressed artwork files:
//! `<data>/artwork/<first 2 hex>/<32 hex>/{small,large}.webp`.
//!
//! The directory name is the [`ArtworkId`], derived from the source bytes, so:
//! - writing the same artwork twice is a no-op (idempotent rescans),
//! - a URL never changes meaning, so the server sends `immutable` cache headers,
//! - two movies with the same poster share files.
//!
//! The `artwork` table is the only reference to an id; anything on disk not
//! referenced there is garbage and [`ArtworkStore::retain`] removes it.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nuvora_media::{ArtworkRole, ArtworkSource, ImageSize, PIPELINE_VERSION, RenderedArtwork};

use crate::ArtworkId;

pub(crate) struct ArtworkStore {
    root: PathBuf,
}

const TMP_MARKER: &str = ".tmp-";

impl ArtworkStore {
    pub(crate) fn new(root: PathBuf) -> std::io::Result<ArtworkStore> {
        fs::create_dir_all(&root)?;
        for entry in shard_entries(&root)? {
            if entry.file_name().to_string_lossy().contains(TMP_MARKER) {
                fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(ArtworkStore { root })
    }

    /// The id a source will be stored under. Pure; computed before rendering so
    /// an already-stored id skips the (expensive) render entirely.
    pub(crate) fn id_for(role: ArtworkRole, source: &ArtworkSource<'_>) -> ArtworkId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&PIPELINE_VERSION.to_le_bytes());
        hasher.update(&[match role {
            ArtworkRole::Poster => 0,
            ArtworkRole::Backdrop => 1,
        }]);
        hasher.update(source.hash_input());
        let mut id = [0u8; 16];
        id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
        ArtworkId(id)
    }

    pub(crate) fn contains(&self, id: ArtworkId) -> bool {
        self.existing_path(id, ImageSize::Large).is_some()
    }

    /// Write every size into `<id>.tmp-<rand>/`, fsync, then rename the
    /// directory to `<id>/`. A crash leaves only a tmp dir (swept on startup),
    /// never a half-written id. If `<id>/` already exists, drop the tmp dir.
    pub(crate) fn put(&self, id: ArtworkId, rendered: &RenderedArtwork) -> std::io::Result<()> {
        if self.contains(id) {
            return Ok(());
        }
        let dir = self.dir(id);
        let shard = dir.parent().expect("dir is under a shard");
        fs::create_dir_all(shard)?;
        let tmp = shard.join(format!("{id}{TMP_MARKER}{}", unique_suffix()));
        fs::create_dir_all(&tmp)?;
        let written = rendered.files.iter().try_for_each(|(size, bytes)| {
            let mut file = fs::File::create(tmp.join(file_name(*size)))?;
            file.write_all(bytes)?;
            file.sync_all()
        });
        let moved = written.and_then(|()| fs::rename(&tmp, &dir));
        match moved {
            Ok(()) => Ok(()),
            Err(_) if self.contains(id) => fs::remove_dir_all(&tmp),
            Err(e) => {
                let _ = fs::remove_dir_all(&tmp);
                Err(e)
            }
        }
    }

    pub(crate) fn existing_path(&self, id: ArtworkId, size: ImageSize) -> Option<PathBuf> {
        let path = self.dir(id).join(file_name(size));
        path.is_file().then_some(path)
    }

    /// Delete stored ids not in `live`. Run at the end of a completed scan with
    /// every hash in the `artwork` table. Returns how many were removed.
    pub(crate) fn retain(&self, live: &HashSet<ArtworkId>) -> std::io::Result<usize> {
        let mut removed = 0;
        for entry in shard_entries(&self.root)? {
            let keep = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<ArtworkId>().ok())
                .is_some_and(|id| live.contains(&id));
            if !keep {
                fs::remove_dir_all(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn dir(&self, id: ArtworkId) -> PathBuf {
        let hex = id.to_string();
        self.root.join(&hex[..2]).join(hex)
    }
}

fn file_name(size: ImageSize) -> String {
    format!("{}.webp", size.as_str())
}

/// Every directory two levels down (`<shard>/<entry>`), ids and leftovers alike.
fn shard_entries(root: &Path) -> std::io::Result<Vec<fs::DirEntry>> {
    let mut entries = Vec::new();
    for shard in fs::read_dir(root)? {
        let shard = shard?;
        if !shard.file_type()?.is_dir() {
            continue;
        }
        for entry in fs::read_dir(shard.path())? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

fn unique_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nuvora_media::Placeholder;

    fn rendered(role: ArtworkRole) -> RenderedArtwork {
        RenderedArtwork {
            role,
            width: 640,
            height: 960,
            placeholder: Placeholder(vec![1, 2, 3]),
            files: vec![
                (ImageSize::Small, b"small".to_vec()),
                (ImageSize::Large, b"large".to_vec()),
            ],
        }
    }

    #[test]
    fn ids_depend_on_role_and_bytes_only() {
        let a = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"img"));
        let b = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"img"));
        let c = ArtworkStore::id_for(ArtworkRole::Backdrop, &ArtworkSource::File(b"img"));
        let d = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"other"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn put_is_atomic_idempotent_and_addressable() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtworkStore::new(dir.path().join("artwork")).unwrap();
        let id = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"img"));
        assert!(!store.contains(id));
        assert_eq!(store.existing_path(id, ImageSize::Small), None);

        store.put(id, &rendered(ArtworkRole::Poster)).unwrap();
        assert!(store.contains(id));
        let small = store.existing_path(id, ImageSize::Small).unwrap();
        assert_eq!(fs::read(&small).unwrap(), b"small");
        assert!(small.ends_with(format!("{}/{id}/small.webp", &id.to_string()[..2])));
        store.put(id, &rendered(ArtworkRole::Poster)).unwrap();
        assert_eq!(
            shard_entries(dir.path().join("artwork").as_path())
                .unwrap()
                .len(),
            1,
            "no tmp dirs left"
        );
    }

    #[test]
    fn startup_sweeps_tmp_dirs_and_retain_removes_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("artwork");
        let store = ArtworkStore::new(root.clone()).unwrap();
        let keep = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"keep"));
        let drop_me = ArtworkStore::id_for(ArtworkRole::Poster, &ArtworkSource::File(b"drop"));
        store.put(keep, &rendered(ArtworkRole::Poster)).unwrap();
        store.put(drop_me, &rendered(ArtworkRole::Poster)).unwrap();
        let crashed = root.join("ab").join(format!("{keep}.tmp-123"));
        fs::create_dir_all(&crashed).unwrap();

        let store = ArtworkStore::new(root.clone()).unwrap();
        assert!(!crashed.exists(), "tmp dir swept on open");
        assert!(store.contains(keep) && store.contains(drop_me));

        let removed = store.retain(&HashSet::from([keep])).unwrap();
        assert_eq!(removed, 1);
        assert!(store.contains(keep));
        assert!(!store.contains(drop_me));
    }
}
