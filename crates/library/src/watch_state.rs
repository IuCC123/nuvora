//! Per-user watch state. The client only reports where playback is; every rule
//! about what that means lives in [`apply`], a pure function, and the write
//! path stores what it returns.

use rusqlite::{Connection, OptionalExtension, params};

use crate::{FileId, ItemId, Library, QueryError, UserId};

/// Under this, a position stores nothing: opening a movie by accident does not
/// put it in Continue Watching.
const TOO_EARLY_MS: u64 = 60_000;
/// A report within this long of the last finish, while the position is
/// already cleared, is the same viewing (a seek near the end), not a new one.
const SAME_VIEWING_MS: i64 = 3 * 3600 * 1000;

/// One user's state for one item. The default is "never touched".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WatchState {
    /// Set only while the item is in progress.
    pub resume: Option<Resume>,
    pub play_count: u32,
    /// Unix ms of the last finish.
    pub last_watched_at: Option<i64>,
}

impl WatchState {
    pub fn watched(&self) -> bool {
        self.play_count > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resume {
    pub file_id: FileId,
    pub position_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEvent {
    /// Where playback is in `file_id`. The caller has checked
    /// `duration_ms >= 1` and that the position is not far past the end.
    Progress {
        file_id: FileId,
        position_ms: u64,
        duration_ms: u64,
    },
    Watched,
    Unwatched,
}

/// A user's watch revision: half of their home, row and item ETags. Values
/// come from one sequence shared by every user, so equal revisions of two
/// different users are both 0, meaning neither has any state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatchRev(pub(crate) u64);

impl WatchRev {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("no such item")]
    ItemNotFound,
    #[error("that file is not a version of this item")]
    FileNotOnItem,
    #[error(transparent)]
    Query(#[from] QueryError),
}

impl From<crate::db::DbError> for WatchError {
    fn from(e: crate::db::DbError) -> Self {
        WatchError::Query(QueryError(e))
    }
}

/// The state after `event` at `now_ms`.
pub(crate) fn apply(state: WatchState, event: WatchEvent, now_ms: i64) -> WatchState {
    match event {
        WatchEvent::Unwatched => WatchState::default(),
        WatchEvent::Watched => finish(state, now_ms),
        WatchEvent::Progress {
            file_id,
            position_ms,
            duration_ms,
        } => {
            if u128::from(position_ms) * 10 >= u128::from(duration_ms) * 9 {
                finish(state, now_ms)
            } else if position_ms < TOO_EARLY_MS {
                WatchState {
                    resume: None,
                    ..state
                }
            } else {
                WatchState {
                    resume: Some(Resume {
                        file_id,
                        position_ms,
                        duration_ms,
                    }),
                    ..state
                }
            }
        }
    }
}

fn finish(state: WatchState, now_ms: i64) -> WatchState {
    let same_viewing = state.resume.is_none()
        && state
            .last_watched_at
            .is_some_and(|at| now_ms - at < SAME_VIEWING_MS);
    if same_viewing {
        return state;
    }
    WatchState {
        resume: None,
        play_count: state.play_count + 1,
        last_watched_at: Some(now_ms),
    }
}

/// `file_id, position_ms, duration_ms, play_count, last_watched_at` starting
/// at column `at`; all NULL when a LEFT JOIN found no row.
pub(crate) fn state_columns(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<WatchState> {
    let file_id: Option<i64> = r.get(at)?;
    let position_ms: Option<i64> = r.get(at + 1)?;
    let duration_ms: Option<i64> = r.get(at + 2)?;
    let resume = match (file_id, position_ms, duration_ms) {
        (Some(file_id), Some(position_ms), Some(duration_ms)) => Some(Resume {
            file_id: FileId(file_id),
            position_ms: position_ms as u64,
            duration_ms: duration_ms as u64,
        }),
        _ => None,
    };
    Ok(WatchState {
        resume,
        play_count: r.get::<_, Option<u32>>(at + 3)?.unwrap_or(0),
        last_watched_at: r.get(at + 4)?,
    })
}

pub(crate) fn load(c: &Connection, user: UserId, item: ItemId) -> rusqlite::Result<WatchState> {
    Ok(c.prepare_cached(
        "SELECT file_id, position_ms, duration_ms, play_count, last_watched_at \
             FROM watch_state WHERE user_id = ?1 AND item_id = ?2",
    )?
    .query_row([user.0, item.0], |r| state_columns(r, 0))
    .optional()?
    .unwrap_or_default())
}

impl Library {
    /// Applies `event` to `user`'s state for `item` and bumps the user's
    /// [`WatchRev`] in the same transaction. An event that changes nothing
    /// writes nothing.
    pub async fn record_watch(
        &self,
        user: UserId,
        item: ItemId,
        event: WatchEvent,
        now_ms: i64,
    ) -> Result<(), WatchError> {
        self.inner
            .db
            .write(move |tx| {
                let exists = tx
                    .query_row("SELECT 1 FROM item WHERE id = ?1", [item.0], |_| Ok(()))
                    .optional()?
                    .is_some();
                if !exists {
                    return Ok(Err(WatchError::ItemNotFound));
                }
                if let WatchEvent::Progress { file_id, .. } = event {
                    let owner: Option<i64> = tx
                        .query_row(
                            "SELECT item_id FROM media_file WHERE id = ?1",
                            [file_id.0],
                            |r| r.get(0),
                        )
                        .optional()?;
                    if owner != Some(item.0) {
                        return Ok(Err(WatchError::FileNotOnItem));
                    }
                }
                let old = load(tx, user, item)?;
                let new = apply(old, event, now_ms);
                if new == old {
                    return Ok(Ok(()));
                }
                if new == WatchState::default() {
                    tx.execute(
                        "DELETE FROM watch_state WHERE user_id = ?1 AND item_id = ?2",
                        [user.0, item.0],
                    )?;
                } else {
                    let resume = new.resume;
                    tx.execute(
                        "INSERT INTO watch_state (user_id, item_id, file_id, position_ms, duration_ms, \
                         play_count, last_watched_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                         ON CONFLICT (user_id, item_id) DO UPDATE SET file_id = excluded.file_id, \
                         position_ms = excluded.position_ms, duration_ms = excluded.duration_ms, \
                         play_count = excluded.play_count, last_watched_at = excluded.last_watched_at, \
                         updated_at = excluded.updated_at",
                        params![
                            user.0,
                            item.0,
                            resume.map(|r| r.file_id.0),
                            resume.map(|r| r.position_ms as i64),
                            resume.map(|r| r.duration_ms as i64),
                            new.play_count,
                            new.last_watched_at,
                            now_ms,
                        ],
                    )?;
                }
                tx.execute("UPDATE catalog SET watch_seq = watch_seq + 1 WHERE id = 1", [])?;
                tx.execute(
                    "UPDATE user SET watch_rev = (SELECT watch_seq FROM catalog WHERE id = 1) \
                     WHERE id = ?1",
                    [user.0],
                )?;
                Ok(Ok(()))
            })
            .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;
    const HOUR: i64 = 3600 * 1000;
    const MOVIE: u64 = 100 * MIN;
    const NOW: i64 = 1_760_000_000_000;

    fn progress(position_ms: u64) -> WatchEvent {
        WatchEvent::Progress {
            file_id: FileId(7),
            position_ms,
            duration_ms: MOVIE,
        }
    }

    fn at(position_ms: u64) -> Option<Resume> {
        Some(Resume {
            file_id: FileId(7),
            position_ms,
            duration_ms: MOVIE,
        })
    }

    fn state(resume: Option<Resume>, play_count: u32, last: Option<i64>) -> WatchState {
        WatchState {
            resume,
            play_count,
            last_watched_at: last,
        }
    }

    #[test]
    fn rules() {
        let fresh = WatchState::default();
        let watched_long_ago = state(None, 1, Some(NOW - 4 * HOUR));
        let just_finished = state(None, 1, Some(NOW - HOUR));
        let rewatching = state(at(50 * MIN), 1, Some(NOW - HOUR));
        let cases = [
            ("30 s stores nothing", fresh, progress(30_000), fresh),
            ("59.999 s stores nothing", fresh, progress(MIN - 1), fresh),
            (
                "60 s is in progress",
                fresh,
                progress(MIN),
                state(at(MIN), 0, None),
            ),
            (
                "under 60 s clears earlier progress",
                state(at(40 * MIN), 0, None),
                progress(10_000),
                fresh,
            ),
            (
                "under 60 s keeps the play count",
                state(at(40 * MIN), 2, Some(NOW - 9 * HOUR)),
                progress(0),
                state(None, 2, Some(NOW - 9 * HOUR)),
            ),
            (
                "just under 90% is in progress",
                fresh,
                progress(90 * MIN - 1),
                state(at(90 * MIN - 1), 0, None),
            ),
            (
                "90% finishes",
                fresh,
                progress(90 * MIN),
                state(None, 1, Some(NOW)),
            ),
            (
                "90% clears the position",
                state(at(80 * MIN), 0, None),
                progress(95 * MIN),
                state(None, 1, Some(NOW)),
            ),
            (
                "past the end finishes",
                fresh,
                progress(MOVIE + 4_000),
                state(None, 1, Some(NOW)),
            ),
            (
                "90% again within 3 h does not count",
                just_finished,
                progress(97 * MIN),
                just_finished,
            ),
            (
                "90% after 3 h counts again",
                watched_long_ago,
                progress(92 * MIN),
                state(None, 2, Some(NOW)),
            ),
            (
                "a rewatch that reached the middle counts within 3 h",
                rewatching,
                progress(95 * MIN),
                state(None, 2, Some(NOW)),
            ),
            (
                "progress on a watched movie keeps the count",
                watched_long_ago,
                progress(10 * MIN),
                state(at(10 * MIN), 1, Some(NOW - 4 * HOUR)),
            ),
            (
                "mark watched finishes",
                state(at(20 * MIN), 0, None),
                WatchEvent::Watched,
                state(None, 1, Some(NOW)),
            ),
            (
                "mark watched twice counts once",
                just_finished,
                WatchEvent::Watched,
                just_finished,
            ),
            (
                "mark unwatched clears everything",
                state(at(20 * MIN), 3, Some(NOW - HOUR)),
                WatchEvent::Unwatched,
                fresh,
            ),
        ];
        for (name, before, event, after) in cases {
            assert_eq!(apply(before, event, NOW), after, "{name}");
        }
    }

    #[test]
    fn a_short_clip_finishes_rather_than_being_too_early() {
        let clip = WatchEvent::Progress {
            file_id: FileId(1),
            position_ms: 28_000,
            duration_ms: 30_000,
        };
        assert_eq!(
            apply(WatchState::default(), clip, NOW),
            state(None, 1, Some(NOW))
        );
    }
}
