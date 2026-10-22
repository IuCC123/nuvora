//! Each user's home screen: which rows, in what order, with what sort and
//! filter. The API owns the JSON format and validates it; this crate stores
//! it per user and builds the rows it describes.

use rusqlite::{OptionalExtension, params};

use crate::browse::LibraryQuery;
use crate::ids::{LibraryId, UserId};
use crate::views::{self, Home};
use crate::{Library, QueryError};

/// One row of a custom home screen.
#[derive(Debug, Clone)]
pub enum HomeRowSpec {
    ContinueWatching {
        title: Option<String>,
    },
    RecentlyAdded {
        title: Option<String>,
    },
    /// A library's cards with the library page's sort and filter, paged
    /// through `GET /api/libraries/{id}/items` with the same query.
    Library {
        library: LibraryId,
        query: LibraryQuery,
        title: Option<String>,
    },
}

impl Library {
    /// The user's saved layout JSON, if they ever saved one.
    pub async fn home_layout(&self, user: UserId) -> Result<Option<String>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| {
                c.query_row(
                    "SELECT layout FROM user_home WHERE user_id = ?1",
                    [user.0],
                    |r| r.get(0),
                )
                .optional()
            })
            .await?)
    }

    /// Save (or with `None`, forget) the user's layout. Bumps their watch
    /// revision, which is half of the home ETag, so the next home request
    /// is never answered from an old layout.
    pub async fn set_home_layout(
        &self,
        user: UserId,
        layout: Option<String>,
    ) -> Result<(), QueryError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        Ok(self
            .inner
            .db
            .write(move |tx| {
                match layout {
                    Some(json) => tx.execute(
                        "INSERT INTO user_home (user_id, layout, updated_at) VALUES (?1, ?2, ?3) \
                         ON CONFLICT(user_id) DO UPDATE SET layout = excluded.layout, updated_at = excluded.updated_at",
                        params![user.0, json, now],
                    )?,
                    None => tx.execute("DELETE FROM user_home WHERE user_id = ?1", [user.0])?,
                };
                tx.execute("UPDATE catalog SET watch_seq = watch_seq + 1 WHERE id = 1", [])?;
                tx.execute(
                    "UPDATE user SET watch_rev = (SELECT watch_seq FROM catalog WHERE id = 1) WHERE id = ?1",
                    [user.0],
                )?;
                Ok(())
            })
            .await?)
    }

    /// `user`'s home with `rows` in order. Continue Watching is left out
    /// while empty, and a row for a library that no longer exists is skipped.
    pub async fn home_with(
        &self,
        user: UserId,
        rows: Vec<HomeRowSpec>,
    ) -> Result<Home, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| views::home_with(c, user, &rows))
            .await?)
    }
}
