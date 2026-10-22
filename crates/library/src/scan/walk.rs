//! Phase 1: walk one root. Blocking I/O; runs in `spawn_blocking`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use nuvora_media::{EpisodeName, PathClass, classify_path, is_skipped_dir};

/// A `/`-separated path relative to a library root. Stored as-is in
/// `media_file.rel_path`. Non-UTF-8 names are reported as issues and skipped.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RelPath(pub String);

impl RelPath {
    /// The directory part (`""` at the root) and the file stem, lowercased.
    pub(crate) fn folder_and_stem(&self) -> (&str, String) {
        let (folder, name) = self.0.rsplit_once('/').unwrap_or(("", &self.0));
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        (folder, stem.to_lowercase())
    }

    /// `self` is `path` or lies under it.
    pub(crate) fn is_within(&self, path: &RelPath) -> bool {
        self == path || self.is_under(path)
    }

    pub(crate) fn is_under(&self, folder: &RelPath) -> bool {
        self.0.len() > folder.0.len()
            && self.0.starts_with(&folder.0)
            && self.0.as_bytes()[folder.0.len()] == b'/'
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RootId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Entries (folders, files, symlinks) that failed to read. Stored files at
    /// or under them are unknown.
    pub unreadable: Vec<RelPath>,
    /// `PathClass::Video` entries.
    pub videos: Vec<DiskFile>,
    /// `PathClass::Image` entries, for local artwork.
    pub images: Vec<DiskFile>,
    /// `PathClass::Subtitle` entries, attached to videos by `sidecars::assign`.
    pub subtitles: Vec<DiskFile>,
    /// Non-UTF-8 names and other per-entry errors.
    pub issues: Vec<(String, String)>,
}

impl RootWalk {
    pub(crate) fn empty(root: RootId, library_id: i64, abs: &Path, complete: bool) -> RootWalk {
        RootWalk {
            root,
            library_id,
            abs: abs.to_path_buf(),
            complete,
            unreadable: Vec::new(),
            videos: Vec::new(),
            images: Vec::new(),
            subtitles: Vec::new(),
            issues: Vec::new(),
        }
    }
}

/// `walkdir` with `follow_links(true)` (NAS libraries are often symlink farms)
/// and loop detection; directories classified as extras are pruned, not descended.
pub(crate) fn walk_root(root: RootId, library_id: i64, abs: &Path) -> RootWalk {
    let mut walk = RootWalk::empty(root, library_id, abs, true);
    if let Err(e) = std::fs::read_dir(abs) {
        walk.complete = false;
        walk.issues
            .push((String::new(), format!("cannot read library folder: {e}")));
        return walk;
    }
    let mut entries = walkdir::WalkDir::new(abs).follow_links(true).into_iter();
    while let Some(entry) = entries.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e.path().unwrap_or(abs);
                let rel = rel_path(abs, path).unwrap_or_default();
                if e.loop_ancestor().is_some() {
                    walk.issues.push((rel, "symlink loop".into()));
                    continue;
                }
                walk.issues
                    .push((rel.clone(), format!("cannot read, so it was kept: {e}")));
                if rel.is_empty() || e.io_error().is_some_and(is_share_down) {
                    walk.complete = false;
                    break;
                }
                walk.unreadable.push(RelPath(rel));
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        let rel = entry.path().strip_prefix(abs).unwrap_or(entry.path());
        if entry.file_type().is_dir() {
            if is_skipped_dir(rel) {
                entries.skip_current_dir();
            }
            continue;
        }
        let Some(rel_str) = rel_path(abs, entry.path()) else {
            walk.issues.push((
                rel.to_string_lossy().into_owned(),
                "name is not valid UTF-8".into(),
            ));
            continue;
        };
        let bucket = match classify_path(rel) {
            PathClass::Video => &mut walk.videos,
            PathClass::Image { .. } => &mut walk.images,
            PathClass::Subtitle => &mut walk.subtitles,
            PathClass::Extra | PathClass::Other => continue,
        };
        let meta = match entry.metadata() {
            Ok(meta) if meta.is_file() => meta,
            Ok(_) => continue,
            Err(e) => {
                walk.issues
                    .push((rel_str.clone(), format!("cannot read, so it was kept: {e}")));
                if e.io_error().is_some_and(is_share_down) {
                    walk.complete = false;
                    break;
                }
                walk.unreadable.push(RelPath(rel_str));
                continue;
            }
        };
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as i64);
        bucket.push(DiskFile {
            rel: RelPath(rel_str),
            size: meta.len(),
            mtime_ns,
        });
    }
    walk.videos.sort_by(|a, b| a.rel.cmp(&b.rel));
    walk.images.sort_by(|a, b| a.rel.cmp(&b.rel));
    walk.subtitles.sort_by(|a, b| a.rel.cmp(&b.rel));
    walk.unreadable.sort();
    walk
}

/// In a TV library, a video whose name has no episode number is left out
/// of the walk, as if it were not there, with an issue saying why.
pub(crate) fn keep_episodes(walk: &mut RootWalk, why: &str) {
    let RootWalk { videos, issues, .. } = walk;
    videos.retain(|video| {
        let episode = EpisodeName::parse(Path::new(&video.rel.0)).is_some();
        if !episode {
            issues.push((video.rel.0.clone(), why.to_owned()));
        }
        episode
    });
}

/// Errors that mean the share itself went away mid-walk, not one bad entry.
/// The whole root is then held, as if it had been unreadable from the start.
fn is_share_down(e: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    const EIO: i32 = 5;
    matches!(e.kind(), ErrorKind::NotConnected | ErrorKind::TimedOut)
        || e.raw_os_error() == Some(EIO)
}

/// `path` relative to `root`, `/`-separated; `None` when not valid UTF-8.
fn rel_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts = rel.iter().map(|c| c.to_str()).collect::<Option<Vec<_>>>()?;
    Some(parts.join("/"))
}

/// blake3(size || first 64 KiB || last 64 KiB), truncated to 16 bytes. Survives
/// renames and moves across roots; changes when content changes. Two reads per
/// file, so it is cheap on a NAS too.
pub(crate) fn fingerprint(abs: &Path, size: u64) -> std::io::Result<[u8; 16]> {
    const CHUNK: u64 = 64 * 1024;
    let mut file = File::open(abs)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = Vec::with_capacity(CHUNK as usize);
    (&mut file).take(CHUNK).read_to_end(&mut buf)?;
    hasher.update(&buf);
    if size > CHUNK {
        buf.clear();
        file.seek(SeekFrom::Start(size - CHUNK))?;
        (&mut file).take(CHUNK).read_to_end(&mut buf)?;
        hasher.update(&buf);
    }
    let mut id = [0u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn walks_videos_and_images_and_prunes_extras() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(
            &root.join("The Matrix (1999)/The Matrix (1999).mkv"),
            b"video",
        );
        touch(&root.join("The Matrix (1999)/poster.jpg"), b"image");
        touch(&root.join("The Matrix (1999)/movie.nfo"), b"nfo");
        touch(
            &root.join("The Matrix (1999)/Extras/Making Of.mkv"),
            b"extra",
        );
        touch(
            &root.join("The Matrix (1999)/the.matrix.sample.mkv"),
            b"sample",
        );
        touch(&root.join(".hidden/Secret (2000).mkv"), b"hidden");
        touch(&root.join("Loose.2001.mp4"), b"loose");
        touch(&root.join(".DS_Store"), b"");

        let walk = walk_root(RootId(1), 1, root);
        assert!(
            walk.complete && walk.unreadable.is_empty() && walk.issues.is_empty(),
            "{walk:?}"
        );
        let videos: Vec<&str> = walk.videos.iter().map(|f| f.rel.0.as_str()).collect();
        assert_eq!(
            videos,
            ["Loose.2001.mp4", "The Matrix (1999)/The Matrix (1999).mkv"]
        );
        let images: Vec<&str> = walk.images.iter().map(|f| f.rel.0.as_str()).collect();
        assert_eq!(images, ["The Matrix (1999)/poster.jpg"]);
        assert_eq!(walk.videos[0].size, 5);
        assert!(walk.videos[0].mtime_ns > 0);
    }

    #[test]
    fn missing_root_is_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let walk = walk_root(RootId(1), 1, &dir.path().join("not-mounted"));
        assert!(!walk.complete);
        assert!(walk.videos.is_empty());
        assert_eq!(walk.issues.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_subfolder_is_reported_not_fatal() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("Open (2000)/Open (2000).mkv"), b"v");
        touch(&root.join("Locked (2001)/Locked (2001).mkv"), b"v");
        let locked = root.join("Locked (2001)");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let walk = walk_root(RootId(1), 1, root);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(walk.complete);
        assert_eq!(walk.unreadable, [RelPath("Locked (2001)".into())]);
        assert_eq!(walk.videos.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn files_in_a_listable_but_unsearchable_folder_are_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("Open (2000)/Open (2000).mkv"), b"v");
        touch(&root.join("Locked (2001)/Locked (2001).mkv"), b"v");
        let locked = root.join("Locked (2001)");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
        let walk = walk_root(RootId(1), 1, root);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(walk.complete);
        assert_eq!(
            walk.unreadable,
            [RelPath("Locked (2001)/Locked (2001).mkv".into())]
        );
        assert_eq!(walk.videos.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_folder_whose_target_cannot_be_statted_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (root, nas) = (dir.path().join("Movies"), dir.path().join("nas"));
        touch(&root.join("Open (2000)/Open (2000).mkv"), b"v");
        touch(&nas.join("Linked (2001)/Linked (2001).mkv"), b"v");
        std::os::unix::fs::symlink(nas.join("Linked (2001)"), root.join("Linked (2001)")).unwrap();
        std::fs::set_permissions(&nas, std::fs::Permissions::from_mode(0o000)).unwrap();
        let walk = walk_root(RootId(1), 1, &root);
        std::fs::set_permissions(&nas, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(walk.complete);
        assert_eq!(walk.unreadable, [RelPath("Linked (2001)".into())]);
        assert_eq!(walk.issues.len(), 1, "{:?}", walk.issues);
        assert_eq!(walk.videos.len(), 1);
    }

    #[test]
    fn a_share_dropping_mid_walk_holds_the_root() {
        use std::io::{Error, ErrorKind};
        assert!(is_share_down(&Error::from_raw_os_error(5)), "EIO");
        assert!(is_share_down(&Error::from(ErrorKind::NotConnected)));
        assert!(is_share_down(&Error::from(ErrorKind::TimedOut)));
        assert!(!is_share_down(&Error::from(ErrorKind::PermissionDenied)));
    }

    #[test]
    fn fingerprint_survives_a_rename_and_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&a, &big).unwrap();
        let fp_a = fingerprint(&a, big.len() as u64).unwrap();
        let b = dir.path().join("b.bin");
        std::fs::rename(&a, &b).unwrap();
        assert_eq!(fingerprint(&b, big.len() as u64).unwrap(), fp_a);

        let mut changed = big.clone();
        changed[100_000] ^= 1;
        std::fs::write(&a, &changed).unwrap();
        assert_eq!(
            fingerprint(&a, big.len() as u64).unwrap(),
            fp_a,
            "a middle byte is outside both chunks"
        );
        changed[big.len() - 1] ^= 1;
        std::fs::write(&a, &changed).unwrap();
        assert_ne!(
            fingerprint(&a, big.len() as u64).unwrap(),
            fp_a,
            "the tail is hashed"
        );
        std::fs::write(&a, &big[..10]).unwrap();
        assert_ne!(fingerprint(&a, 10).unwrap(), fp_a, "size is hashed");
    }

    #[test]
    fn rel_path_helpers() {
        let rel = RelPath("Alien (1979)/Alien (1979) - 4K.MKV".into());
        assert_eq!(
            rel.folder_and_stem(),
            ("Alien (1979)", "alien (1979) - 4k".to_owned())
        );
        assert!(rel.is_under(&RelPath("Alien (1979)".into())));
        assert!(!rel.is_under(&RelPath("Alien".into())));
        assert_eq!(
            RelPath("Heat.mkv".into()).folder_and_stem(),
            ("", "heat".to_owned())
        );
    }
}
