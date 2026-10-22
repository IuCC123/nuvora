//! SQLite access. One writer connection, a small pool of read-only connections,
//! all driven from `spawn_blocking`. WAL mode lets readers run during a scan
//! without ever seeing a half-written file (each file's rows commit atomically).
//!
//! Why one explicit writer instead of letting connections race: SQLite has one
//! writer at a time anyway; racing writers turn into SQLITE_BUSY retries. A
//! single `Mutex<Connection>` makes the serialization structural and visible.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Transaction};
use tokio::sync::Semaphore;

use crate::CatalogRev;

/// Migrations in order. `PRAGMA user_version` = number applied.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("database is newer than this build (schema {found}, supported {supported})")]
    TooNew { found: usize, supported: usize },
    #[error("database task panicked")]
    Panicked,
}

#[derive(Clone)]
pub(crate) struct Db {
    writer: Arc<Mutex<Connection>>,
    readers: Arc<ReadPool>,
    /// `catalog.rev` as of the last commit. Only `write` stores it.
    rev: Arc<AtomicU64>,
}

struct ReadPool {
    idle: Mutex<Vec<Connection>>,
    permits: Semaphore,
}

impl Db {
    /// Open `path`, set pragmas, migrate, then open `readers` read-only
    /// connections, and load `catalog.rev`. Pragmas on every connection: `journal_mode=WAL`,
    /// `synchronous=NORMAL` (durable at checkpoint; a power cut can lose the
    /// last transactions, which the next scan redoes), `foreign_keys=ON`,
    /// `busy_timeout=5000`, `mmap_size=268435456`, `cache_size=-16000`.
    pub(crate) fn open(path: &Path, readers: usize) -> Result<Db, DbError> {
        let _ = (path, readers);
        todo!()
    }

    /// Run `f` on a pooled read-only connection, inside one deferred read
    /// transaction so multi-query reads (home = two queries) see one snapshot.
    pub(crate) async fn read<T, F>(&self, f: F) -> Result<T, DbError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let _ = f;
        todo!("acquire permit; pop conn; spawn_blocking; BEGIN; f; COMMIT; push conn back")
    }

    /// The revision published by the last `write`.
    pub(crate) fn rev(&self) -> CatalogRev {
        CatalogRev(self.rev.load(Ordering::Acquire))
    }

    /// Run `f` in an IMMEDIATE transaction on the single writer connection.
    /// Commits if `f` returns Ok, rolls back otherwise. Keep `f` short (one
    /// file's rows); never hold the writer across an await or a subprocess.
    /// After the commit, still holding the writer, read `catalog.rev` and store
    /// it in `rev`, so readers never see a rev newer than the committed data.
    pub(crate) async fn write<T, F>(&self, f: F) -> Result<T, DbError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> rusqlite::Result<T> + Send + 'static,
    {
        let _ = f;
        todo!("spawn_blocking; lock writer; transaction_with_behavior(Immediate); f; commit")
    }
}

/// Apply pending migrations in one transaction each, bumping `user_version`.
/// Idempotent: a second call is a no-op. Refuses a database from a newer build.
fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let _ = (conn, MIGRATIONS);
    todo!()
}
