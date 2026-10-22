//! The catalog. One owner for all persistent state: the SQLite database, the
//! content-addressed artwork store, and the scan that writes both.
//!
//! The public surface is [`Library`] plus the domain read models it returns.
//! SQL, row types, the connection pool and the scanner's internals are private;
//! the server crate cannot name them, so it cannot bypass the catalog's rules.

mod accounts;
mod db;
mod ids;
mod libraries;
mod scan;
mod store;
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
    AccountError, Credentials, NewUser, Role, Session, TokenHash, User, UserChanges,
};
pub use ids::{ArtworkId, FileId, ItemId, LibraryId, ParseIdError, UserId};
pub use libraries::{LibraryError, LibraryInfo};
pub use scan::{ScanIssue, ScanPhase, ScanProgress, ScanState, ScanStatus, ScanSummary};
pub use views::{
    Artwork, Card, CatalogRev, Cursor, HOME_ROW_LIMIT, Home, InvalidCursor, Item, ItemKind,
    MAX_ROW_LIMIT, Movie, Row, RowKind, RowPage, StreamSource, Version, VersionMedia,
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
        let db = tokio::task::spawn_blocking(move || db::Db::open(&db_path, READERS))
            .await
            .map_err(|_| OpenError::Db(db::DbError::Panicked))??;
        let specs = config.libraries;
        db.write(move |tx| scan::write::add_libraries(tx, &specs))
            .await?;
        let store =
            Arc::new(store::ArtworkStore::new(config.data_dir.join("artwork")).map_err(io)?);
        let scan = scan::ScanHandle::spawn(scan::ScanContext {
            db: db.clone(),
            store: Arc::clone(&store),
            tools: config.tools,
            probe_concurrency: config.probe_concurrency,
            settle_window: config.settle_window,
        });
        Ok(Library {
            inner: Arc::new(Inner { db, store, scan }),
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
            .unwrap();
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
        async fn data_version(lib: &Library) -> i64 {
            lib.inner
                .db
                .read(|c| c.query_row("PRAGMA data_version", [], |r| r.get::<_, i64>(0)))
                .await
                .unwrap()
        }
        let before = data_version(&lib).await;

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
            data_version(&lib).await,
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
            .expect("id still resolves");
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
            .update_library(movies.id, Some("Films".into()), Some(vec![b.clone()]), 0)
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
        assert!(median < Duration::from_millis(50), "median {median:?}");
    }
}
