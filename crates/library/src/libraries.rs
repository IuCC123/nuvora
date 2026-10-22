//! Libraries as the admin panel sees and edits them. Paths arrive validated
//! (absolute, canonical, existing folders); the catalog checks only what needs
//! the database: a folder belongs to at most one library.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::scan::write::{bump_rev, mark_empty_items_missing};
use crate::{Library, LibraryId, LibraryKind, QueryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryInfo {
    pub id: LibraryId,
    pub name: String,
    pub kind: LibraryKind,
    pub roots: Vec<PathBuf>,
    /// Items with at least one file present.
    pub item_count: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{} is already in another library", .0.display())]
    PathInUse(PathBuf),
    #[error("no such library")]
    NotFound,
    #[error(transparent)]
    Query(#[from] QueryError),
}

impl From<crate::db::DbError> for LibraryError {
    fn from(e: crate::db::DbError) -> Self {
        LibraryError::Query(QueryError(e))
    }
}

fn load(c: &Connection, id: i64) -> rusqlite::Result<Option<LibraryInfo>> {
    let Some((name, kind)) = c
        .query_row("SELECT name, kind FROM library WHERE id = ?1", [id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let kind = match kind.as_str() {
        "movies" => LibraryKind::Movies,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                format!("unknown library kind {other:?}").into(),
            ));
        }
    };
    let roots = c
        .prepare_cached("SELECT path FROM library_root WHERE library_id = ?1 ORDER BY id")?
        .query_map([id], |r| Ok(PathBuf::from(r.get::<_, String>(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let item_count: i64 = c.query_row(
        "SELECT count(*) FROM item WHERE library_id = ?1 AND parent_id IS NULL \
         AND missing_since IS NULL",
        [id],
        |r| r.get(0),
    )?;
    Ok(Some(LibraryInfo {
        id: LibraryId(id),
        name,
        kind,
        roots,
        item_count: item_count as u32,
    }))
}

/// The first of `roots` that a library other than `library` already has.
fn root_in_use(
    tx: &Transaction<'_>,
    library: i64,
    roots: &[PathBuf],
) -> rusqlite::Result<Option<PathBuf>> {
    for root in roots {
        let used = tx
            .query_row(
                "SELECT 1 FROM library_root WHERE path = ?1 AND library_id != ?2",
                params![root.to_string_lossy(), library],
                |_| Ok(()),
            )
            .optional()?;
        if used.is_some() {
            return Ok(Some(root.clone()));
        }
    }
    Ok(None)
}

impl Library {
    /// Every library, in creation order.
    pub async fn libraries(&self) -> Result<Vec<LibraryInfo>, QueryError> {
        Ok(self
            .inner
            .db
            .read(|c| {
                let ids: Vec<i64> = c
                    .prepare("SELECT id FROM library ORDER BY id")?
                    .query_map([], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?;
                let mut out = Vec::with_capacity(ids.len());
                for id in ids {
                    out.extend(load(c, id)?);
                }
                Ok(out)
            })
            .await?)
    }

    pub async fn library(&self, id: LibraryId) -> Result<Option<LibraryInfo>, QueryError> {
        Ok(self.inner.db.read(move |c| load(c, id.0)).await?)
    }

    /// Adds a library. Does not scan; the caller calls [`Library::request_scan`].
    pub async fn create_library(
        &self,
        name: String,
        kind: LibraryKind,
        roots: Vec<PathBuf>,
    ) -> Result<LibraryInfo, LibraryError> {
        self.inner
            .db
            .write(move |tx| {
                if let Some(path) = root_in_use(tx, 0, &roots)? {
                    return Ok(Err(LibraryError::PathInUse(path)));
                }
                let kind = match kind {
                    LibraryKind::Movies => "movies",
                };
                // The key only identifies config-file libraries; a panel-made
                // library gets one no config can name by accident.
                tx.execute(
                    "INSERT INTO library (key, name, kind) VALUES ('', ?1, ?2)",
                    params![name, kind],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "UPDATE library SET key = ?1 WHERE id = ?2",
                    params![format!("library-{id}"), id],
                )?;
                for root in &roots {
                    tx.execute(
                        "INSERT INTO library_root (library_id, path) VALUES (?1, ?2)",
                        params![id, root.to_string_lossy()],
                    )?;
                }
                bump_rev(tx)?;
                Ok(load(tx, id)?.ok_or(LibraryError::NotFound))
            })
            .await?
    }

    /// Renames a library and/or replaces its folders. Nothing changes when a
    /// folder is rejected. Swapping exactly one folder for another repoints
    /// it, keeping its files (a share remounted elsewhere). Otherwise a
    /// removed folder takes its files out of the catalog; items it emptied
    /// are marked missing, so they keep their ids if the folder comes back.
    /// Returns whether the folders changed (the caller then requests a scan).
    pub async fn update_library(
        &self,
        id: LibraryId,
        name: Option<String>,
        roots: Option<Vec<PathBuf>>,
        now_ms: i64,
    ) -> Result<(LibraryInfo, bool), LibraryError> {
        self.inner
            .db
            .write(move |tx| {
                let Some(before) = load(tx, id.0)? else {
                    return Ok(Err(LibraryError::NotFound));
                };
                if let Some(roots) = &roots
                    && let Some(path) = root_in_use(tx, id.0, roots)?
                {
                    return Ok(Err(LibraryError::PathInUse(path)));
                }
                if let Some(name) = &name {
                    tx.execute(
                        "UPDATE library SET name = ?1 WHERE id = ?2",
                        params![name, id.0],
                    )?;
                }
                let mut changed = false;
                if let Some(roots) = &roots {
                    let gone: Vec<&PathBuf> =
                        before.roots.iter().filter(|r| !roots.contains(r)).collect();
                    let new: Vec<&PathBuf> =
                        roots.iter().filter(|r| !before.roots.contains(r)).collect();
                    changed = !gone.is_empty() || !new.is_empty();
                    if let ([from], [to]) = (gone.as_slice(), new.as_slice()) {
                        tx.execute(
                            "UPDATE library_root SET path = ?3 WHERE library_id = ?1 AND path = ?2",
                            params![id.0, from.to_string_lossy(), to.to_string_lossy()],
                        )?;
                    } else {
                        for gone in gone {
                            tx.execute(
                                "DELETE FROM library_root WHERE library_id = ?1 AND path = ?2",
                                params![id.0, gone.to_string_lossy()],
                            )?;
                        }
                        for new in new {
                            tx.execute(
                                "INSERT INTO library_root (library_id, path) VALUES (?1, ?2)",
                                params![id.0, new.to_string_lossy()],
                            )?;
                        }
                        if changed {
                            mark_empty_items_missing(tx, now_ms)?;
                        }
                    }
                }
                if changed || name.is_some() {
                    bump_rev(tx)?;
                }
                Ok(load(tx, id.0)?
                    .ok_or(LibraryError::NotFound)
                    .map(|info| (info, changed)))
            })
            .await?
    }

    /// Removes a library with its folders and items from the catalog. Their
    /// artwork files are deleted by the next scan's cleanup.
    pub async fn delete_library(&self, id: LibraryId) -> Result<(), LibraryError> {
        self.inner
            .db
            .write(move |tx| {
                if tx.execute("DELETE FROM library WHERE id = ?1", [id.0])? == 0 {
                    return Ok(Err(LibraryError::NotFound));
                }
                bump_rev(tx)?;
                Ok(Ok(()))
            })
            .await?
    }
}
