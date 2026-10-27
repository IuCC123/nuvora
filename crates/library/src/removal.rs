//! Deleting a movie from disk: its files, their subtitles and, when the
//! movie has a folder of its own, that folder.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nuvora_media::{PathClass, classify_path};
use rusqlite::Connection;

use crate::ids::{FileId, ItemId};
use crate::scan::unix_ms;
use crate::scan::write::remove_gone;
use crate::{Library, QueryError};

/// Files that come with a release and mean nothing without its video.
const RELEASE_LEFTOVERS: &[&str] = &["nfo", "txt", "url", "sfv", "md5", "torrent", "db", "ini"];

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("no such movie")]
    NotFound,
    /// What was deleted before this is out of the catalog; the rest stays.
    #[error("couldn't delete {}: {error}", path.display())]
    Disk {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error(transparent)]
    Query(#[from] QueryError),
}

impl From<crate::db::DbError> for DeleteError {
    fn from(e: crate::db::DbError) -> Self {
        DeleteError::Query(QueryError(e))
    }
}

/// One version of the movie.
struct OnDisk {
    id: FileId,
    root: PathBuf,
    /// Under `root`.
    rel: PathBuf,
    sidecars: Vec<PathBuf>,
}

impl Library {
    /// Deletes a movie's files and subtitles, and its folder when what is
    /// left there is only the movie's own (artwork, extras, release notes).
    /// It leaves the catalog the way a vanished file does: hidden at once,
    /// its watch state kept should the same movie come back.
    pub async fn delete_movie(&self, id: ItemId) -> Result<(), DeleteError> {
        let files = self
            .inner
            .db
            .read(move |c| movie_files(c, id))
            .await?
            .ok_or(DeleteError::NotFound)?;
        let (gone, failed) = tokio::task::spawn_blocking(move || delete_files(&files))
            .await
            .expect("delete task");
        if !gone.is_empty() {
            let now = unix_ms(SystemTime::now());
            self.inner
                .db
                .write(move |tx| remove_gone(tx, &gone, &[], now))
                .await?;
        }
        failed.map_or(Ok(()), Err)
    }
}

/// `None` when `id` is not a movie in the catalog.
fn movie_files(c: &Connection, id: ItemId) -> rusqlite::Result<Option<Vec<OnDisk>>> {
    let movie = c
        .prepare_cached(
            "SELECT 1 FROM item WHERE id = ?1 AND kind = 'movie' AND missing_since IS NULL",
        )?
        .exists([id.0])?;
    if !movie {
        return Ok(None);
    }
    let files = c
        .prepare_cached(
            "SELECT f.id, r.path, f.rel_path FROM media_file f \
             JOIN library_root r ON r.id = f.root_id WHERE f.item_id = ?1 ORDER BY f.id",
        )?
        .query_map([id.0], |r| {
            Ok((
                FileId(r.get(0)?),
                PathBuf::from(r.get::<_, String>(1)?),
                PathBuf::from(r.get::<_, String>(2)?),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    files
        .into_iter()
        .map(|(id, root, rel)| {
            let sidecars = crate::subtitles::sidecars(c, id)?;
            Ok(OnDisk {
                id,
                root,
                rel,
                sidecars: sidecars.into_iter().map(|s| s.path).collect(),
            })
        })
        .collect::<rusqlite::Result<_>>()
        .map(Some)
}

/// The files that are gone, and the error that stopped it, if one did.
fn delete_files(files: &[OnDisk]) -> (Vec<FileId>, Option<DeleteError>) {
    let mut gone = Vec::new();
    for file in files {
        let video = file.root.join(&file.rel);
        if let Err(error) = remove_file(&video) {
            return (gone, Some(DeleteError::Disk { path: video, error }));
        }
        gone.push(file.id);
        for sidecar in &file.sidecars {
            if let Err(error) = remove_file(sidecar) {
                let path = sidecar.clone();
                return (gone, Some(DeleteError::Disk { path, error }));
            }
        }
    }
    for file in files {
        if let Some(folder) = file.rel.parent().filter(|f| !f.as_os_str().is_empty()) {
            remove_folder(&file.root, folder);
        }
    }
    (gone, None)
}

fn remove_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Deletes `root/folder` when all it holds is the movie's own, then each
/// parent folder left empty, up to `root`.
fn remove_folder(root: &Path, folder: &Path) {
    let dir = root.join(folder);
    match only_leftovers(root, &dir) {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => {
            tracing::warn!(folder = %dir.display(), "kept after deleting a movie: {e}");
            return;
        }
    }
    if let Err(e) = std::fs::remove_dir_all(&dir) {
        tracing::warn!(folder = %dir.display(), "kept after deleting a movie: {e}");
        return;
    }
    let mut parent = folder.parent();
    while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
        if std::fs::remove_dir(root.join(p)).is_err() {
            break;
        }
        parent = p.parent();
    }
}

/// True when nothing under `dir` is a video or a file of someone's own: only
/// extras, artwork, subtitles, hidden files and release notes.
fn only_leftovers(root: &Path, dir: &Path) -> std::io::Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !only_leftovers(root, &path)? {
                return Ok(false);
            }
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let leftover = match classify_path(rel) {
            PathClass::Video => false,
            PathClass::Extra | PathClass::Image { .. } | PathClass::Subtitle => true,
            PathClass::Other => {
                let name = entry.file_name().to_string_lossy().into_owned();
                let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
                name.starts_with('.')
                    || ext.is_some_and(|e| RELEASE_LEFTOVERS.contains(&e.as_str()))
            }
        };
        if !leftover {
            return Ok(false);
        }
    }
    Ok(true)
}
