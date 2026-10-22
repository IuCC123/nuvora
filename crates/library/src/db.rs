//! SQLite access. One writer connection, a small pool of read-only connections,
//! all driven from `spawn_blocking`. WAL mode lets readers run during a scan
//! without ever seeing a half-written file (each file's rows commit atomically).
//!
//! Why one explicit writer instead of letting connections race: SQLite has one
//! writer at a time anyway; racing writers turn into SQLITE_BUSY retries. A
//! single `Mutex<Connection>` makes the serialization structural and visible.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use tokio::sync::Semaphore;

use crate::CatalogRev;

/// Migrations in order. `PRAGMA user_version` = number applied.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_accounts.sql"),
    include_str!("../migrations/0003_watch_state.sql"),
    include_str!("../migrations/0004_dolby_vision_compatibility.sql"),
    include_str!("../migrations/0006_subtitle_prefs.sql"),
];

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
    permits: Arc<Semaphore>,
}

impl Db {
    /// Open `path`, set pragmas, migrate, then open `readers` read-only
    /// connections, and load `catalog.rev`. Pragmas on every connection: `journal_mode=WAL`,
    /// `synchronous=NORMAL` (durable at checkpoint; a power cut can lose the
    /// last transactions, which the next scan redoes), `foreign_keys=ON`,
    /// `busy_timeout=5000`, `mmap_size=268435456`, `cache_size=-16000`.
    pub(crate) fn open(path: &Path, readers: usize) -> Result<Db, DbError> {
        let mut writer = Connection::open(path)?;
        writer.pragma_update(None, "journal_mode", "WAL")?;
        pragmas(&writer)?;
        migrate(&mut writer)?;
        let rev = read_rev(&writer)?;

        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let idle = (0..readers)
            .map(|_| {
                let conn = Connection::open_with_flags(path, flags)?;
                pragmas(&conn)?;
                Ok(conn)
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        Ok(Db {
            writer: Arc::new(Mutex::new(writer)),
            readers: Arc::new(ReadPool {
                idle: Mutex::new(idle),
                permits: Arc::new(Semaphore::new(readers)),
            }),
            rev: Arc::new(AtomicU64::new(rev)),
        })
    }

    /// Run `f` on a pooled read-only connection, inside one deferred read
    /// transaction so multi-query reads (home = two queries) see one snapshot.
    pub(crate) async fn read<T, F>(&self, f: F) -> Result<T, DbError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let permit = Arc::clone(&self.readers.permits)
            .acquire_owned()
            .await
            .map_err(|_| DbError::Panicked)?;
        let pool = Arc::clone(&self.readers);
        tokio::task::spawn_blocking(move || {
            // Declared before `returning`, so it drops after the connection is
            // back: a cancelled caller cannot free the permit early.
            let _permit = permit;
            let conn = lock(&pool.idle)
                .pop()
                .expect("a permit is held, so a connection is idle");
            let returning = Returning {
                pool: &pool,
                conn: Some(conn),
            };
            let conn = returning.conn.as_ref().expect("set above");
            let tx = conn.unchecked_transaction()?;
            let out = f(&tx)?;
            tx.commit()?;
            Ok(out)
        })
        .await
        .map_err(|_| DbError::Panicked)?
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
        let writer = Arc::clone(&self.writer);
        let rev = Arc::clone(&self.rev);
        tokio::task::spawn_blocking(move || {
            let mut conn = lock(&writer);
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let out = f(&tx)?;
            tx.commit()?;
            rev.store(read_rev(&conn)?, Ordering::Release);
            Ok(out)
        })
        .await
        .map_err(|_| DbError::Panicked)?
    }
}

/// Puts a read connection back in the pool, including when `f` panics.
struct Returning<'a> {
    pool: &'a ReadPool,
    conn: Option<Connection>,
}

impl Drop for Returning<'_> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            lock(&self.pool.idle).push(conn);
        }
    }
}

/// A poisoned mutex only means a task panicked while holding it; the
/// connection itself is still usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pragmas(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "mmap_size", 268_435_456)?;
    conn.pragma_update(None, "cache_size", -16_000)?;
    Ok(())
}

fn read_rev(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row("SELECT rev FROM catalog WHERE id = 1", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|r| r as u64)
}

/// Apply pending migrations in one transaction each, bumping `user_version`.
/// Idempotent: a second call is a no-op. Refuses a database from a newer build.
fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let applied = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
    if applied > MIGRATIONS.len() {
        return Err(DbError::TooNew {
            found: applied,
            supported: MIGRATIONS.len(),
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opens_migrates_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nuvora.db");
        let db = Db::open(&path, 2).unwrap();
        assert_eq!(db.rev().get(), 0);
        let version: i64 = db
            .read(|c| c.query_row("PRAGMA user_version", [], |r| r.get(0)))
            .await
            .unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
        drop(db);
        let db = Db::open(&path, 2).unwrap();
        let tables: i64 = db
            .read(|c| {
                c.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                    [],
                    |r| r.get(0),
                )
            })
            .await
            .unwrap();
        assert_eq!(
            tables, 13,
            "7 catalog tables + user, session, setting, watch_state + sqlite_sequence"
        );
    }

    #[tokio::test]
    async fn writes_publish_the_rev_and_reads_see_them() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 2).unwrap();
        db.write(|tx| {
            tx.execute(
                "INSERT INTO library (key, name, kind) VALUES ('movies', 'Movies', 'movies')",
                [],
            )?;
            tx.execute("UPDATE catalog SET rev = rev + 1", [])
        })
        .await
        .unwrap();
        assert_eq!(db.rev().get(), 1);
        let name: String = db
            .read(|c| {
                c.query_row("SELECT name FROM library WHERE key = 'movies'", [], |r| {
                    r.get(0)
                })
            })
            .await
            .unwrap();
        assert_eq!(name, "Movies");
    }

    #[tokio::test]
    async fn a_failed_write_rolls_back_and_keeps_the_rev() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 2).unwrap();
        let result = db
            .write(|tx| {
                tx.execute("UPDATE catalog SET rev = rev + 1", [])?;
                tx.execute(
                    "INSERT INTO library (key, name, kind) VALUES ('a', 'A', 'movies')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO library (key, name, kind) VALUES ('a', 'A', 'movies')",
                    [],
                )
            })
            .await;
        assert!(result.is_err());
        assert_eq!(db.rev().get(), 0);
        let rows: i64 = db
            .read(|c| c.query_row("SELECT count(*) FROM library", [], |r| r.get(0)))
            .await
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[tokio::test]
    async fn read_connections_survive_a_panicking_closure() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 1).unwrap();
        let panicked = db
            .read(|_| -> rusqlite::Result<()> { panic!("boom") })
            .await;
        assert!(matches!(panicked, Err(DbError::Panicked)));
        let one: i64 = db
            .read(|c| c.query_row("SELECT 1", [], |r| r.get(0)))
            .await
            .unwrap();
        assert_eq!(one, 1);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nuvora.db");
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        assert!(matches!(
            Db::open(&path, 1),
            Err(DbError::TooNew { found: 99, .. })
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_cancelled_read_does_not_break_the_next_one() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 1).unwrap();
        let slow = db.clone();
        let reading = tokio::spawn(async move {
            slow.read(|_| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(())
            })
            .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        reading.abort();
        let _ = reading.await;
        let next = db
            .read(|c| c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0)))
            .await;
        assert_eq!(next.unwrap(), 1);
    }
}
