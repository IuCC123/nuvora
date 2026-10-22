//! The catalog. One owner for all persistent state: the SQLite database, the
//! content-addressed artwork store, and the scan that writes both.
//!
//! The public surface is [`Library`] plus the domain read models it returns.
//! SQL, row types, the connection pool and the scanner's internals are private;
//! the server crate cannot name them, so it cannot bypass the catalog's rules.

mod db;
mod ids;
mod scan;
mod store;
mod views;

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use nuvora_media::{DeviceProfile, ImageSize, MediaTools};
use tokio::sync::watch;

pub use ids::{ArtworkId, FileId, ItemId, ParseIdError};
pub use scan::{ScanIssue, ScanPhase, ScanProgress, ScanState, ScanStatus, ScanSummary};
pub use views::{
    Artwork, Card, CatalogRev, Cursor, HOME_ROW_LIMIT, Home, InvalidCursor, Item, ItemKind,
    MAX_ROW_LIMIT, Movie, Row, RowKind, RowPage, StreamSource, Version, VersionMedia,
};

/// Everything the catalog needs, already validated by the caller (the server's
/// config loader). The catalog does not read config files or environment.
#[derive(Debug, Clone)]
pub struct LibraryConfig {
    /// Holds `nuvora.db` (+ `-wal`, `-shm`) and `artwork/`.
    pub data_dir: PathBuf,
    pub libraries: Vec<LibrarySpec>,
    pub tools: MediaTools,
    /// Concurrent ffprobe processes. Default: min(available_parallelism, 8).
    pub probe_concurrency: NonZeroUsize,
}

/// One configured library. `key` is the stable identity (a slug); renaming
/// `name` keeps the library's id and everything hanging off it.
#[derive(Debug, Clone)]
pub struct LibrarySpec {
    pub key: String,
    pub name: String,
    pub kind: LibraryKind,
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    Movies,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("cannot use data directory {path}: {source}")]
    DataDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("database: {0}")]
    Db(#[from] db::DbError),
}

#[derive(Debug, thiserror::Error)]
#[error("catalog query failed: {0}")]
pub struct QueryError(#[from] db::DbError);

/// Handle to the catalog. Cheap to clone; all clones share one database, one
/// artwork store and one scan worker.
#[derive(Clone)]
pub struct Library {
    inner: Arc<Inner>,
}

struct Inner {
    db: db::Db,
    store: Arc<store::ArtworkStore>,
    scan: scan::ScanHandle,
}

impl Library {
    /// Open (creating if needed) the database, apply migrations, upsert the
    /// configured libraries and roots by `key`, and start the scan worker.
    /// Does not scan; the caller calls [`Library::request_scan`] at startup so
    /// the HTTP server is up immediately, serving whatever is already cataloged.
    pub async fn open(config: LibraryConfig) -> Result<Library, OpenError> {
        let _ = config;
        todo!(
            "create dirs; db::Db::open; db.write(sync_libraries); store::ArtworkStore::new; scan::ScanHandle::spawn"
        )
    }

    /// Ask for a scan of every library. Idempotent and coalescing: while a scan
    /// runs, at most one more is queued; calling this ten times during a scan
    /// yields exactly one follow-up scan. Returns the status right after queuing.
    pub fn request_scan(&self) -> ScanStatus {
        self.inner.scan.request()
    }

    /// Latest scan status (cheap; reads a `watch` channel).
    pub fn scan_status(&self) -> ScanStatus {
        self.inner.scan.status()
    }

    /// Subscribe to scan status changes (for a future SSE endpoint or tests).
    pub fn watch_scan(&self) -> watch::Receiver<ScanStatus> {
        self.inner.scan.subscribe()
    }

    /// The catalog revision as of the last committed write. No query: the writer
    /// publishes it after each commit, so a matching `If-None-Match` costs nothing.
    pub fn rev(&self) -> CatalogRev {
        self.inner.db.rev()
    }

    /// The home screen in one read transaction: the first page (at most
    /// [`HOME_ROW_LIMIT`] cards) of each [`RowKind::ALL`] row, with its total and
    /// a cursor to the rest. Items whose files are all missing are excluded. Two
    /// indexed keyset queries, so the cost does not grow with the library.
    pub async fn home(&self) -> Result<Home, QueryError> {
        Ok(self.inner.db.read(views::home).await?)
    }

    /// The page of `kind` after `after` (`None` = from the start), `limit` cards
    /// clamped to `1..=MAX_ROW_LIMIT`. `after` comes from [`Cursor::parse`] with
    /// the same `kind`, or from a previous page of this row.
    pub async fn row(
        &self,
        kind: RowKind,
        after: Option<Cursor>,
        limit: u32,
    ) -> Result<RowPage, QueryError> {
        debug_assert!(after.as_ref().is_none_or(|c| c.kind() == kind));
        let limit = limit.clamp(1, MAX_ROW_LIMIT);
        Ok(self
            .inner
            .db
            .read(move |c| {
                let rev = views::rev(c)?;
                let (_, cards, next) = views::row_page(c, kind, after.as_ref(), limit)?;
                Ok(RowPage { rev, cards, next })
            })
            .await?)
    }

    /// One item with every version (file) and every stream, plus a playback
    /// decision per version for `device`. `None` if the id does not exist.
    /// Missing items still resolve (the id is stable), with an empty `versions`.
    pub async fn item(
        &self,
        id: ItemId,
        device: DeviceProfile,
    ) -> Result<Option<Item>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| views::item(c, id, &device))
            .await?)
    }

    /// Where to read a file for direct play. The absolute path is built here from
    /// root + relative path and is never sent to clients.
    pub async fn stream_source(&self, id: FileId) -> Result<Option<StreamSource>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| views::stream_source(c, id))
            .await?)
    }

    /// Path of one stored artwork size, if it exists. No database access: the
    /// store is content-addressed, so the id alone names the file.
    pub fn artwork_path(&self, id: ArtworkId, size: ImageSize) -> Option<PathBuf> {
        self.inner.store.existing_path(id, size)
    }
}
