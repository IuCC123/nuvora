//! The catalog. One owner for all persistent state: the SQLite database, the
//! content-addressed artwork store, and the scan that writes both.
//!
//! The public surface is [`Library`] plus the domain read models it returns.
//! SQL, row types, the connection pool and the scanner's internals are private;
//! the server crate cannot name them, so it cannot bypass the catalog's rules.

mod accounts;
mod browse;
mod db;
mod home_layout;
mod ids;
mod libraries;
mod metadata;
mod previews;
mod scan;
mod search;
mod shows;
#[cfg(test)]
mod shows_tests;
mod store;
mod subtitles;
#[cfg(test)]
mod test_support;
mod views;
mod watch_state;

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nuvora_media::{DeviceProfile, ImageSize, MediaTools};
use tokio::sync::watch;

pub use accounts::{
    AccountError, Credentials, NewUser, Role, Session, SubtitleMode, TokenHash, User, UserChanges,
};
pub use browse::{
    GroupKey, IndexGroup, LibraryCursor, LibraryPage, LibraryQuery, Order, Resolution, Sort,
    WatchFilter,
};
pub use home_layout::HomeRowSpec;
pub use ids::{ArtworkId, FileId, ItemId, LibraryId, ParseIdError, UserId};
pub use libraries::{LibraryError, LibraryInfo};
pub use metadata::read::{Credit, Extras};
pub use previews::{PreviewKey, Previews};
pub use scan::{ScanIssue, ScanPhase, ScanProgress, ScanState, ScanStatus, ScanSummary};
pub use search::{EpisodeHit, PersonHit, PersonRole, SearchResults};
pub use shows::{Episode, EpisodeRef, Season, SeasonSummary, Show};
pub use subtitles::{SIDECAR_INDEX_BASE, Sidecar, SubtitleSource};
pub use views::{
    Artwork, Card, CardNote, CatalogRev, Cursor, EpisodeNumber, HOME_ROW_LIMIT, Home,
    InvalidCursor, Item, ItemKind, MAX_ROW_LIMIT, Movie, Row, RowKind, RowPage, StreamSource,
    Version, VersionMedia,
};
pub use watch_state::{Resume, WatchError, WatchEvent, WatchRev, WatchState};

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
    /// Files modified more recently than this are skipped by a scan as still
    /// being copied. The server uses 60 s; tests use zero.
    pub settle_window: Duration,
    /// Look up descriptions, artwork and cast online after each scan. The
    /// server does; tests don't, so they never depend on the network.
    pub online_metadata: bool,
}

/// One library from the config file or `--movies`. `key` is the stable identity
/// (a slug). [`Library::open`] only adds a library or root that does not exist
/// yet; after that the admin panel owns it, and changing or removing it in the
/// config changes nothing.
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
    /// TV Shows: every file is an episode, grouped into seasons and shows.
    Shows,
}

impl LibraryKind {
    /// The `library.kind` column.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LibraryKind::Movies => "movies",
            LibraryKind::Shows => "shows",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<LibraryKind> {
        match name {
            "movies" => Some(LibraryKind::Movies),
            "shows" => Some(LibraryKind::Shows),
            _ => None,
        }
    }
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

/// Upper bound on a library page; larger requests are clamped.
pub const MAX_LIBRARY_LIMIT: u32 = 200;

/// Read-only connections in the pool; home and item reads are short, so a
/// handful covers a houseful of clients.
const READERS: usize = 4;

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
    metadata: metadata::MetadataHandle,
    previews: previews::PreviewHandle,
}

impl Library {
    /// Open (creating if needed) the database, apply migrations, add the
    /// configured libraries and roots that do not exist yet (by `key`), and
    /// start the scan worker.
    /// Does not scan; the caller calls [`Library::request_scan`] at startup so
    /// the HTTP server is up immediately, serving whatever is already cataloged.
    pub async fn open(config: LibraryConfig) -> Result<Library, OpenError> {
        let data_dir = config.data_dir.clone();
        let io = |source| OpenError::DataDir {
            path: data_dir.clone(),
            source,
        };
        std::fs::create_dir_all(&config.data_dir).map_err(io)?;
        let db_path = config.data_dir.join("nuvora.db");
        #[cfg(unix)]
        owner_only(&config.data_dir, &db_path).map_err(io)?;
        let db = tokio::task::spawn_blocking(move || db::Db::open(&db_path, READERS))
            .await
            .map_err(|_| OpenError::Db(db::DbError::Panicked))??;
        let specs = config.libraries;
        db.write(move |tx| scan::write::add_libraries(tx, &specs))
            .await?;
        let store =
            Arc::new(store::ArtworkStore::new(config.data_dir.join("artwork")).map_err(io)?);
        let preview_store =
            Arc::new(previews::PreviewStore::new(config.data_dir.join("previews")).map_err(io)?);
        let preview_tools = config.tools.clone();
        let scan = scan::ScanHandle::spawn(scan::ScanContext {
            db: db.clone(),
            store: Arc::clone(&store),
            tools: config.tools,
            probe_concurrency: config.probe_concurrency,
            settle_window: config.settle_window,
        });
        let metadata = metadata::MetadataHandle::spawn(
            db.clone(),
            Arc::clone(&store),
            scan.subscribe(),
            config.online_metadata,
        );
        let previews = previews::PreviewHandle::spawn(
            db.clone(),
            preview_store,
            preview_tools,
            scan.subscribe(),
        );
        Ok(Library {
            inner: Arc::new(Inner {
                db,
                store,
                scan,
                metadata,
                previews,
            }),
        })
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

    /// `user`'s home screen in one read transaction: the first page (at most
    /// [`HOME_ROW_LIMIT`] cards) of each [`RowKind::ALL`] row, with its total and
    /// a cursor to the rest. Continue Watching only when it has cards. Items
    /// whose files are all missing are excluded. Indexed keyset queries, so the
    /// cost does not grow with the library.
    pub async fn home(&self, user: UserId) -> Result<Home, QueryError> {
        Ok(self.inner.db.read(move |c| views::home(c, user)).await?)
    }

    /// The page of `kind` after `after` (`None` = from the start), `limit` cards
    /// clamped to `1..=MAX_ROW_LIMIT`. `after` comes from [`Cursor::parse`] with
    /// the same `kind`, or from a previous page of this row.
    pub async fn row(
        &self,
        kind: RowKind,
        after: Option<Cursor>,
        limit: u32,
        user: UserId,
    ) -> Result<RowPage, QueryError> {
        debug_assert!(after.as_ref().is_none_or(|c| c.kind() == kind));
        let limit = limit.clamp(1, MAX_ROW_LIMIT);
        Ok(self
            .inner
            .db
            .read(move |c| {
                let rev = views::rev(c)?;
                let (_, cards, next) = views::row_page(c, kind, after.as_ref(), limit, user)?;
                Ok(RowPage { rev, cards, next })
            })
            .await?)
    }

    /// One item with every version (file) and every stream, plus a playback
    /// decision per version for `device` and `user`'s watch state. `None` if
    /// the id does not exist. Missing items still resolve (the id is stable),
    /// with an empty `versions`.
    pub async fn item(
        &self,
        id: ItemId,
        device: DeviceProfile,
        user: UserId,
    ) -> Result<Option<Item>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| views::item(c, id, &device, user))
            .await?)
    }

    /// The cards of library `id` matching `query` after `after` (`None` =
    /// from the start), `limit` clamped to `1..=MAX_LIBRARY_LIMIT`. `None`
    /// when the library does not exist. `after` comes from
    /// [`LibraryCursor::parse`] with the same query.
    pub async fn library_items(
        &self,
        id: LibraryId,
        query: LibraryQuery,
        after: Option<LibraryCursor>,
        limit: u32,
        user: UserId,
    ) -> Result<Option<LibraryPage>, QueryError> {
        let limit = limit.clamp(1, MAX_LIBRARY_LIMIT);
        Ok(self
            .inner
            .db
            .read(move |c| {
                let rev = views::rev(c)?;
                let page = browse::page(c, id, &query, after.as_ref(), limit, user)?;
                Ok(page.map(|p| LibraryPage {
                    rev,
                    total: p.total,
                    cards: p.cards,
                    next: p.next,
                }))
            })
            .await?)
    }

    /// The group headers of library `id` for `query`, with the catalog rev
    /// they were read at. `None` when the library does not exist.
    pub async fn library_index(
        &self,
        id: LibraryId,
        query: LibraryQuery,
        user: UserId,
    ) -> Result<Option<(CatalogRev, Vec<IndexGroup>)>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| {
                let rev = views::rev(c)?;
                Ok(browse::index(c, id, &query, user)?.map(|groups| (rev, groups)))
            })
            .await?)
    }

    /// A season's present episodes in order, each with its versions'
    /// playback decisions for `device` and `user`'s watch state. `None`
    /// unless `season` is a season.
    pub async fn episodes(
        &self,
        season: ItemId,
        device: DeviceProfile,
        user: UserId,
    ) -> Result<Option<Vec<Episode>>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| shows::episodes(c, season, &device, user))
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

    /// The seek previews of `file`, or `None` while they are still to be made
    /// (or it can't have any). Asking moves the file to the front of the queue.
    pub async fn previews(&self, file: FileId) -> Result<Option<Previews>, QueryError> {
        let key = self
            .inner
            .db
            .read(move |c| previews::key_of(c, file))
            .await?;
        let Some(key) = key else { return Ok(None) };
        match self.inner.previews.store.sheets(&key) {
            Some(sheets) => Ok(Some(Previews { key, sheets })),
            None => {
                self.inner.previews.hurry(file);
                Ok(None)
            }
        }
    }

    /// Path of sheet `n` of the previews named `key`, if it exists. No database
    /// access: the key names the folder.
    pub fn preview_sheet(&self, key: &PreviewKey, n: u32) -> Option<PathBuf> {
        self.inner.previews.store.sheet(key, n)
    }

    /// Path of one stored artwork size, if it exists. No database access: the
    /// store is content-addressed, so the id alone names the file.
    pub fn artwork_path(&self, id: ArtworkId, size: ImageSize) -> Option<PathBuf> {
        self.inner.store.existing_path(id, size)
    }
}

/// The database holds password and session hashes, so the data directory is
/// 0700 and the database 0600. SQLite creates `-wal` and `-shm` with the
/// database's mode, so creating it 0600 before the first open covers them.
/// A path this user cannot chmod (a mount owned by someone else) is left as
/// it is, with a warning.
#[cfg(unix)]
fn owner_only(data_dir: &std::path::Path, db_path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let restrict = |path: &std::path::Path, mode: u32| {
        let Ok(meta) = std::fs::metadata(path) else {
            return;
        };
        if meta.permissions().mode() & 0o777 == mode {
            return;
        }
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
            tracing::warn!(path = %path.display(), "cannot restrict permissions to {mode:o}: {e}");
        }
    };
    restrict(data_dir, 0o700);
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(db_path)?;
    for suffix in ["", "-wal", "-shm"] {
        let mut path = db_path.as_os_str().to_owned();
        path.push(suffix);
        restrict(std::path::Path::new(&path), 0o600);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{movie_library, sdr_movie, seed_movies};
    use std::path::Path;
    use std::time::{Instant, SystemTime};

    /// A user with no watch state.
    const NOBODY: UserId = UserId(0);

    async fn open(root: &Path, data: &Path) -> Library {
        let tools = MediaTools::locate(None, None).await.unwrap();
        Library::open(LibraryConfig {
            data_dir: data.to_path_buf(),
            libraries: vec![LibrarySpec {
                key: "movies".into(),
                name: "Movies".into(),
                kind: LibraryKind::Movies,
                roots: vec![root.to_path_buf()],
            }],
            tools,
            probe_concurrency: NonZeroUsize::new(4).unwrap(),
            settle_window: Duration::ZERO,
            online_metadata: false,
        })
        .await
        .unwrap()
    }

    /// Request a scan and wait for the summary of a scan started after the request.
    async fn scan(lib: &Library) -> ScanSummary {
        let before = SystemTime::now();
        let mut status = lib.watch_scan();
        lib.request_scan();
        let done = status
            .wait_for(|s| {
                matches!(s.state, ScanState::Idle)
                    && s.last.as_ref().is_some_and(|l| l.started_at >= before)
            })
            .await
            .unwrap();
        done.last.clone().unwrap()
    }

    async fn home_titles(lib: &Library) -> Vec<String> {
        let home = lib.home(NOBODY).await.unwrap();
        home.rows[1].cards.iter().map(|c| c.title.clone()).collect()
    }

    async fn find(lib: &Library, title: &str) -> Movie {
        let home = lib.home(NOBODY).await.unwrap();
        let card = home.rows[1]
            .cards
            .iter()
            .find(|c| c.title == title)
            .unwrap_or_else(|| panic!("{title} on home"));
        let Item::Movie(movie) = lib
            .item(card.id, DeviceProfile::baseline(), NOBODY)
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("a movie")
        };
        movie
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_scan_catalogs_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        movie_library(&root);
        let lib = open(&root, &data).await;
        assert_eq!(lib.rev().get(), 0);

        let summary = scan(&lib).await;
        assert_eq!(
            (
                summary.added,
                summary.changed,
                summary.moved,
                summary.removed,
                summary.failed
            ),
            (5, 0, 0, 0, 0),
            "{summary:?}"
        );
        assert!(summary.issues.is_empty(), "{:?}", summary.issues);
        assert!(lib.rev().get() > 0);

        let home = lib.home(NOBODY).await.unwrap();
        assert_eq!(
            home.rows.iter().map(|r| r.total).collect::<Vec<_>>(),
            [5, 5]
        );
        assert_eq!(
            home_titles(&lib).await,
            ["Big Buck", "Bright", "Elephants", "Sintel", "The Spring"]
        );

        let big_buck = find(&lib, "Big Buck").await;
        assert_eq!(big_buck.year, Some(2008));
        let poster = big_buck.poster.as_ref().expect("local poster");
        assert_eq!((poster.width, poster.height), (600, 900));
        let backdrop = big_buck.backdrop.as_ref().expect("local fanart");
        assert_eq!((backdrop.width, backdrop.height), (1280, 720));
        for size in ImageSize::ALL {
            assert!(
                lib.artwork_path(poster.id, size)
                    .is_some_and(|p| p.is_file()),
                "{size:?}"
            );
        }
        let VersionMedia::Probed { info, playback } = &big_buck.versions[0].media else {
            panic!("probed")
        };
        assert_eq!(playback, &nuvora_media::PlaybackDecision::DirectPlay);
        assert_eq!(info.video[0].height, 180);
        let source = lib
            .stream_source(big_buck.versions[0].file_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            source.path,
            root.join("Big Buck (2008)/Big Buck (2008).mp4")
        );

        let sintel = find(&lib, "Sintel").await;
        assert_eq!(sintel.year, Some(2010));
        assert!(sintel.poster.is_none(), "no local poster, no frame poster");
        let frame = sintel
            .backdrop
            .as_ref()
            .expect("frame backdrop from the SDR file");
        assert_eq!((frame.width, frame.height), (320, 180));

        let bright = find(&lib, "Bright").await;
        assert!(bright.backdrop.is_none(), "HDR gets no frame grab");
        let elephants = find(&lib, "Elephants").await;
        assert_eq!(elephants.versions.len(), 1, "the sample is not a version");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rescan_without_changes_commits_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        movie_library(&root);
        let lib = open(&root, &data).await;
        scan(&lib).await;
        let rev = lib.rev();
        // data_version counts other connections' commits as one connection
        // sees them, so both readings come from the same connection.
        let watcher = rusqlite::Connection::open(data.join("nuvora.db")).unwrap();
        let data_version = || -> i64 {
            watcher
                .query_row("PRAGMA data_version", [], |r| r.get(0))
                .unwrap()
        };
        let before = data_version();

        let summary = scan(&lib).await;
        assert_eq!(
            (
                summary.unchanged,
                summary.added,
                summary.changed,
                summary.moved,
                summary.removed
            ),
            (5, 0, 0, 0, 0)
        );
        assert_eq!(lib.rev(), rev, "catalog rev unchanged");
        assert_eq!(
            data_version(),
            before,
            "no transaction committed during the rescan"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_keeps_the_item_id_and_deletion_marks_it_missing() {
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        movie_library(&root);
        let lib = open(&root, &data).await;
        scan(&lib).await;
        let sintel = find(&lib, "Sintel").await;

        std::fs::rename(
            root.join("Sintel (2010)/Sintel.2010.1080p.BluRay.x264-GRP.mkv"),
            root.join("Sintel (2010)/Sintel (2010).mkv"),
        )
        .unwrap();
        let summary = scan(&lib).await;
        assert_eq!(
            (
                summary.moved,
                summary.added,
                summary.removed,
                summary.changed
            ),
            (1, 0, 0, 0),
            "{summary:?}"
        );
        let renamed = find(&lib, "Sintel").await;
        assert_eq!(renamed.id, sintel.id, "item id survives a rename");
        assert_eq!(
            renamed.versions[0].file_id, sintel.versions[0].file_id,
            "file id survives a rename"
        );
        assert_eq!(renamed.versions[0].file_name, "Sintel (2010).mkv");
        assert_eq!(
            renamed.backdrop.as_ref().map(|b| b.id),
            sintel.backdrop.as_ref().map(|b| b.id),
            "artwork kept"
        );

        std::fs::rename(root.join("Sintel (2010)"), root.join("Sintel Fixed (2010)")).unwrap();
        let summary = scan(&lib).await;
        assert_eq!(
            (summary.moved, summary.added, summary.removed),
            (1, 0, 0),
            "folder rename is a move: {summary:?}"
        );
        let rekeyed = find(&lib, "Sintel Fixed").await;
        assert_eq!(
            rekeyed.id, sintel.id,
            "a rename to an unused title re-keys the item in place"
        );
        assert_eq!(rekeyed.versions[0].file_id, sintel.versions[0].file_id);

        std::fs::remove_dir_all(root.join("Sintel Fixed (2010)")).unwrap();
        let summary = scan(&lib).await;
        assert_eq!(summary.removed, 1);
        assert_eq!(
            home_titles(&lib).await,
            ["Big Buck", "Bright", "Elephants", "The Spring"]
        );
        let Item::Movie(missing) = lib
            .item(sintel.id, DeviceProfile::baseline(), NOBODY)
            .await
            .unwrap()
            .expect("id still resolves")
        else {
            panic!("a movie")
        };
        assert!(missing.versions.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn offline_or_empty_root_marks_nothing_missing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        movie_library(&root);
        let lib = open(&root, &data).await;
        scan(&lib).await;
        let rev = lib.rev();

        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).unwrap();
        let summary = scan(&lib).await;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(summary.removed, 0);
        assert!(
            summary
                .issues
                .iter()
                .any(|i| i.message.contains("cannot read library folder")),
            "{:?}",
            summary.issues
        );
        assert_eq!(
            lib.home(NOBODY).await.unwrap().rows[0].total,
            5,
            "unreadable root removes nothing"
        );
        assert_eq!(lib.rev(), rev);

        let parked = dir.path().join("parked");
        std::fs::rename(&root, &parked).unwrap();
        std::fs::create_dir(&root).unwrap();
        let summary = scan(&lib).await;
        assert_eq!(summary.removed, 0);
        assert_eq!(
            lib.home(NOBODY).await.unwrap().rows[0].total,
            5,
            "empty root with stored files removes nothing"
        );
        assert_eq!(lib.rev(), rev);

        std::fs::remove_dir(&root).unwrap();
        std::fs::rename(&parked, &root).unwrap();
        let summary = scan(&lib).await;
        assert_eq!((summary.unchanged, summary.removed), (5, 0));
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn an_offline_symlinked_folder_removes_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (root, data, nas) = (
            dir.path().join("Movies"),
            dir.path().join("data"),
            dir.path().join("nas"),
        );
        sdr_movie(&root.join("Open (2000)/Open (2000).mp4"));
        sdr_movie(&nas.join("Linked (2001)/Linked (2001).mp4"));
        std::os::unix::fs::symlink(nas.join("Linked (2001)"), root.join("Linked (2001)")).unwrap();
        let lib = open(&root, &data).await;
        assert_eq!(scan(&lib).await.added, 2);

        std::fs::set_permissions(&nas, std::fs::Permissions::from_mode(0o000)).unwrap();
        let summary = scan(&lib).await;
        std::fs::set_permissions(&nas, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(summary.removed, 0, "{:?}", summary.issues);
        assert_eq!(home_titles(&lib).await, ["Linked", "Open"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_future_mtime_does_not_rescan_forever() {
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        let future = root.join("Future (2001)/Future (2001).mp4");
        sdr_movie(&future);
        std::fs::File::options()
            .write(true)
            .open(&future)
            .unwrap()
            .set_modified(SystemTime::now() + Duration::from_secs(86_400 * 365))
            .unwrap();
        let lib = open(&root, &data).await;
        let summary = scan(&lib).await;
        assert_eq!((summary.added, summary.settling), (1, 0), "{summary:?}");
        assert!(
            summary
                .issues
                .iter()
                .any(|i| i.path == "Future (2001)/Future (2001).mp4"),
            "{:?}",
            summary.issues
        );

        let mut status = lib.watch_scan();
        status.borrow_and_update();
        let quiet = tokio::time::timeout(Duration::from_secs(3), status.changed()).await;
        assert!(quiet.is_err(), "no scan runs without a request");
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_that_could_not_be_read_is_retried_next_scan() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        let file = root.join("Heat (1995)/Heat (1995).mp4");
        sdr_movie(&file);
        let mode = |m| std::fs::set_permissions(&file, std::fs::Permissions::from_mode(m)).unwrap();
        let lib = open(&root, &data).await;

        mode(0o000);
        let first = scan(&lib).await;
        let second = scan(&lib).await;
        mode(0o644);
        assert_eq!((first.added, first.failed), (1, 1), "{first:?}");
        assert_eq!(
            (second.changed, second.unchanged, second.failed),
            (1, 0, 1),
            "{second:?}"
        );

        let third = scan(&lib).await;
        assert_eq!((third.changed, third.failed), (1, 0), "{third:?}");
        let heat = find(&lib, "Heat").await;
        assert!(matches!(
            heat.versions[0].media,
            VersionMedia::Probed { .. }
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn config_only_adds_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, data) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("data"),
        );
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let lib = open(&a, &data).await;
        let movies = lib.libraries().await.unwrap().remove(0);
        let (renamed, _) = lib
            .update_library(
                movies.id,
                Some("Films".into()),
                Some(vec![b.clone()]),
                None,
                0,
            )
            .await
            .unwrap();
        assert_eq!(renamed.roots, std::slice::from_ref(&b));
        drop(lib);

        let lib = open(&a, &data).await;
        let libraries = lib.libraries().await.unwrap();
        assert_eq!(libraries.len(), 1);
        assert_eq!(
            libraries[0].name, "Films",
            "the panel's name survives a restart"
        );
        assert_eq!(
            libraries[0].roots,
            [b.clone(), a.clone()],
            "the config folder is added back, the panel's folder kept"
        );

        let other = lib
            .create_library("Other".into(), LibraryKind::Movies, vec![a.clone()])
            .await;
        assert!(matches!(other, Err(LibraryError::PathInUse(p)) if p == a));

        let films = libraries[0].id;
        lib.delete_library(films).await.unwrap();
        let panel = lib
            .create_library("Panel".into(), LibraryKind::Movies, vec![a.clone()])
            .await
            .unwrap();
        drop(lib);
        let lib = open(&a, &data).await;
        assert_eq!(
            lib.libraries().await.unwrap(),
            [panel],
            "a config folder the panel already uses creates no empty library"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_rejected_folder_change_keeps_the_old_name() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, data) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("data"),
        );
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let lib = open(&a, &data).await;
        let movies = lib.libraries().await.unwrap().remove(0);
        lib.create_library("Other".into(), LibraryKind::Movies, vec![b.clone()])
            .await
            .unwrap();
        let rejected = lib
            .update_library(
                movies.id,
                Some("Films".into()),
                Some(vec![b.clone()]),
                None,
                0,
            )
            .await;
        assert!(matches!(rejected, Err(LibraryError::PathInUse(p)) if p == b));
        let after = lib.library(movies.id).await.unwrap().unwrap();
        assert_eq!((after.name.as_str(), after.roots), ("Movies", vec![a]));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn swapping_one_folder_for_another_keeps_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, data) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("data"),
        );
        sdr_movie(&a.join("Heat (1995)/Heat (1995).mp4"));
        let lib = open(&a, &data).await;
        scan(&lib).await;
        let before = find(&lib, "Heat").await;

        std::fs::rename(&a, &b).unwrap();
        let movies = lib.libraries().await.unwrap().remove(0);
        let (info, changed) = lib
            .update_library(movies.id, None, Some(vec![b.clone()]), None, 0)
            .await
            .unwrap();
        assert!(changed);
        assert_eq!(info.roots, [b]);
        let summary = scan(&lib).await;
        assert_eq!(
            (summary.added, summary.removed, summary.unchanged),
            (0, 0, 1),
            "{summary:?}"
        );
        let after = find(&lib, "Heat").await;
        assert_eq!(
            (after.id, after.versions[0].file_id),
            (before.id, before.versions[0].file_id)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn swapping_one_folder_for_an_empty_one_takes_its_files_out() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, data) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("data"),
        );
        sdr_movie(&a.join("Heat (1995)/Heat (1995).mp4"));
        std::fs::create_dir_all(&b).unwrap();
        let lib = open(&a, &data).await;
        scan(&lib).await;

        let movies = lib.libraries().await.unwrap().remove(0);
        assert_eq!(movies.item_count, 1);
        let (info, changed) = lib
            .update_library(movies.id, None, Some(vec![b.clone()]), None, 0)
            .await
            .unwrap();
        assert!(changed);
        assert_eq!((info.roots, info.item_count), (vec![b], 0));
        scan(&lib).await;
        assert_eq!(lib.library(movies.id).await.unwrap().unwrap().item_count, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deleting_a_library_mid_scan_fails_its_files_not_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        let (root, other, data) = (
            dir.path().join("Movies"),
            dir.path().join("Other"),
            dir.path().join("data"),
        );
        for i in 0..8 {
            sdr_movie(&root.join(format!("Movie {i} (200{i})/Movie {i} (200{i}).mp4")));
        }
        for i in 0..3 {
            sdr_movie(&other.join(format!("Keep {i} (199{i})/Keep {i} (199{i}).mp4")));
        }
        let lib = open(&root, &data).await;
        let movies = lib.libraries().await.unwrap().remove(0);
        lib.create_library("Other".into(), LibraryKind::Movies, vec![other])
            .await
            .unwrap();

        let mut status = lib.watch_scan();
        lib.request_scan();
        status
            .wait_for(|s| {
                matches!(&s.state, ScanState::Running { progress, .. }
                    if progress.phase == ScanPhase::Reading && progress.done >= 1)
            })
            .await
            .unwrap();
        lib.delete_library(movies.id).await.unwrap();
        let summary = status
            .wait_for(|s| matches!(s.state, ScanState::Idle) && s.last.is_some())
            .await
            .unwrap()
            .last
            .clone()
            .unwrap();
        assert!(
            summary
                .issues
                .iter()
                .all(|i| !i.message.contains("scan aborted")),
            "{:?}",
            summary.issues
        );
        assert!(summary.failed >= 1, "{summary:?}");
        assert_eq!(summary.added + summary.failed, 11, "{summary:?}");
        assert_eq!(home_titles(&lib).await, ["Keep 0", "Keep 1", "Keep 2"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scan_requests_coalesce_into_one_follow_up() {
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        for i in 0..6 {
            sdr_movie(&root.join(format!("Movie {i} (200{i})/Movie {i} (200{i}).mp4")));
        }
        let lib = open(&root, &data).await;
        let mut status = lib.watch_scan();
        lib.request_scan();
        status
            .wait_for(|s| matches!(s.state, ScanState::Running { .. }))
            .await
            .unwrap();
        for _ in 0..10 {
            lib.request_scan();
        }
        assert!(matches!(
            lib.scan_status().state,
            ScanState::Running { queued: true, .. }
        ));

        // A watch receiver sees only the current value, so the first summary may
        // already be replaced by the follow-up's when we look. The follow-up is
        // recognisable: it found everything already cataloged.
        let follow_up = status
            .wait_for(|s| {
                matches!(s.state, ScanState::Idle)
                    && s.last.as_ref().is_some_and(|l| l.unchanged == 6)
            })
            .await
            .unwrap()
            .last
            .clone()
            .unwrap();
        assert_eq!((follow_up.added, follow_up.unchanged), (0, 6));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let settled = lib.scan_status();
        assert!(matches!(settled.state, ScanState::Idle));
        assert_eq!(
            settled.last.unwrap().started_at,
            follow_up.started_at,
            "ten requests during a scan ran exactly one follow-up"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn home_with_2000_movies_is_fast() {
        let dir = tempfile::tempdir().unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        let lib = Library::open(LibraryConfig {
            data_dir: dir.path().to_path_buf(),
            libraries: vec![],
            tools,
            probe_concurrency: NonZeroUsize::new(1).unwrap(),
            settle_window: Duration::ZERO,
            online_metadata: false,
        })
        .await
        .unwrap();
        lib.inner
            .db
            .write(|tx| seed_movies(tx, 2000))
            .await
            .unwrap();

        for _ in 0..5 {
            lib.home(NOBODY).await.unwrap();
        }
        let mut samples = Vec::new();
        for _ in 0..50 {
            let started = Instant::now();
            let home = lib.home(NOBODY).await.unwrap();
            samples.push(started.elapsed());
            assert_eq!(
                home.rows
                    .iter()
                    .map(|r| (r.total, r.cards.len()))
                    .collect::<Vec<_>>(),
                [(2000, 40), (2000, 40)]
            );
            assert_eq!(home.rows[0].cards[0].title, "Movie 1999");
            assert_eq!(home.rows[1].cards[0].title, "A Movie 0000");
            assert_eq!(
                home.rows[1]
                    .cards
                    .iter()
                    .filter(|c| c.poster.is_some())
                    .count(),
                20
            );
        }
        samples.sort();
        let (min, median, max) = (samples[0], samples[25], samples[49]);
        let plans: Vec<String> = lib
            .inner
            .db
            .read(|c| {
                let mut out = Vec::new();
                for sql in [views::CARDS_PLAN_RECENT, views::CARDS_PLAN_TITLE] {
                    let mut stmt = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
                    let rows = stmt.query_map([], |r| r.get::<_, String>(3))?;
                    out.push(rows.collect::<rusqlite::Result<Vec<_>>>()?.join(" | "));
                }
                Ok(out)
            })
            .await
            .unwrap();
        eprintln!("home() with 2000 movies: min {min:?} median {median:?} max {max:?}");
        eprintln!("recently_added plan: {}", plans[0]);
        eprintln!("all_movies plan: {}", plans[1]);
        // Shared CI runners are slower; they raise the budget.
        let budget = std::env::var("NUVORA_PERF_BUDGET_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(50);
        assert!(median < Duration::from_millis(budget), "median {median:?}");
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn the_data_dir_and_database_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
        std::fs::create_dir_all(&root).unwrap();
        let mode = |name: &str| {
            std::fs::metadata(data.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };

        let lib = open(&root, &data).await;
        assert_eq!(mode(""), 0o700, "created 0700");
        for name in ["nuvora.db", "nuvora.db-wal", "nuvora.db-shm"] {
            assert_eq!(mode(name), 0o600, "{name} created 0600");
        }
        drop(lib);

        std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
        let db = data.join("nuvora.db");
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _lib = open(&root, &data).await;
        assert_eq!(mode(""), 0o700, "tightened to 0700");
        for name in ["nuvora.db", "nuvora.db-wal", "nuvora.db-shm"] {
            assert_eq!(mode(name), 0o600, "{name} tightened to 0600");
        }
    }
}
