//! Discovery: what home shows beyond rows in a sort order. Featured titles
//! for the hero, Trending and Top lists from the household's watching,
//! genres, and "More like this". Read models like `views.rs`; all SQL here
//! runs inside the caller's read transaction.

use nuvora_media::ArtworkRole;
use rusqlite::{Connection, OptionalExtension};

use crate::genres;
use crate::shows::{self, EpisodeRef};
use crate::views::{self, Artwork, Card, ItemKind};
use crate::{ItemId, UserId};

/// Titles in the home hero.
pub const FEATURED_LIMIT: usize = 6;
/// Cards in the Top row: a ranked ten.
pub const TOP_LIMIT: u32 = 10;
/// Cards in the Trending row.
pub const TRENDING_LIMIT: u32 = 20;
/// Trending looks at the last week of watching.
const TRENDING_WINDOW_MS: i64 = 7 * 24 * 3600 * 1000;
/// A ranked list with fewer titles than this reads as broken, so it is left out.
const MIN_RANKED: usize = 3;
/// Genre tiles on home.
const GENRE_TILES: u32 = 14;
/// Cards in "More like this".
pub const SIMILAR_LIMIT: u32 = 20;

/// One hero title: its card (with backdrop) and what the hero writes.
#[derive(Debug, Clone)]
pub struct Featured {
    pub card: Card,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    /// Display names, the provider's order.
    pub genres: Vec<String>,
    /// 0..10.
    pub rating: Option<f32>,
    /// A movie's longest file.
    pub runtime_ms: Option<u64>,
    /// A show's numbered seasons.
    pub season_count: Option<u32>,
    /// A show's next episode for this user (what Play starts).
    pub next: Option<EpisodeRef>,
}

/// A genre on home: its key for the genre page, a name, how many titles,
/// and the backdrop of one of them.
#[derive(Debug, Clone)]
pub struct GenreTile {
    pub key: String,
    pub name: String,
    pub count: u32,
    pub art: Option<Artwork>,
}

/// A page of one genre's titles, newest first.
#[derive(Debug, Clone)]
pub struct GenrePage {
    pub name: String,
    pub total: u32,
    pub cards: Vec<Card>,
    /// `<added_at_ms>.<id>` of the last card, when there is more.
    pub next: Option<String>,
}

/// Sets each card's landscape art: the item's backdrop; for an episode its
/// own frame, else its show's backdrop.
pub(crate) fn fill_backdrops(conn: &Connection, cards: &mut [Card]) -> rusqlite::Result<()> {
    for card in cards {
        if card.backdrop.is_none() {
            card.backdrop = match backdrop(conn, card.id)? {
                Some(art) => Some(art),
                None => card
                    .show
                    .map(|show| backdrop(conn, show))
                    .transpose()?
                    .flatten(),
            };
        }
    }
    Ok(())
}

fn backdrop(conn: &Connection, id: ItemId) -> rusqlite::Result<Option<Artwork>> {
    conn.prepare_cached(
        "SELECT hash, width, height, placeholder FROM item_artwork WHERE item_id = ?1 AND role = 'backdrop'",
    )?
    .query_row([id.0], |r| views::artwork(r, 0, ArtworkRole::Backdrop))
    .optional()
    .map(Option::flatten)
}

/// A movie's or show's card for `user`, show counts filled, with backdrop.
fn title_card(conn: &Connection, id: ItemId, user: UserId) -> rusqlite::Result<Option<Card>> {
    let Some(mut card) = views::card_by_id(conn, id, user)? else {
        return Ok(None);
    };
    if card.kind == ItemKind::Show {
        shows::fill_show_card(conn, &mut card, user)?;
    }
    fill_backdrops(conn, std::slice::from_mut(&mut card))?;
    Ok(Some(card))
}

/// Present movies and shows ranked by the household's watching since
/// `since_ms`: most viewers, then most plays, then most recent. Episodes
/// count for their show.
fn ranked(
    conn: &Connection,
    since_ms: i64,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<Vec<Card>> {
    let ids: Vec<ItemId> = conn
        .prepare_cached(
            "WITH a AS ( \
               SELECT CASE WHEN i.kind = 'episode' THEN se.parent_id ELSE i.id END AS title, \
                      w.user_id, w.play_count, w.updated_at \
               FROM watch_state w JOIN item i ON i.id = w.item_id \
               LEFT JOIN item se ON se.id = i.parent_id AND i.kind = 'episode' \
               WHERE i.kind IN ('movie', 'episode') AND i.missing_since IS NULL AND w.updated_at >= ?1) \
             SELECT a.title FROM a JOIN item t ON t.id = a.title AND t.missing_since IS NULL \
             GROUP BY a.title \
             ORDER BY count(DISTINCT a.user_id) DESC, sum(a.play_count > 0) + count(*) DESC, max(a.updated_at) DESC, a.title \
             LIMIT ?2",
        )?
        .query_map([since_ms, i64::from(limit)], |r| Ok(ItemId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut cards = Vec::with_capacity(ids.len());
    for id in ids {
        cards.extend(title_card(conn, id, user)?);
    }
    if cards.len() < MIN_RANKED {
        cards.clear();
    }
    Ok(cards)
}

/// The last week's most watched titles; empty when fewer than three.
pub(crate) fn trending(
    conn: &Connection,
    user: UserId,
    now_ms: i64,
) -> rusqlite::Result<Vec<Card>> {
    ranked(conn, now_ms - TRENDING_WINDOW_MS, TRENDING_LIMIT, user)
}

/// The household's ten most watched titles of all time; empty when fewer than three.
pub(crate) fn top(conn: &Connection, user: UserId) -> rusqlite::Result<Vec<Card>> {
    ranked(conn, 0, TOP_LIMIT, user)
}

/// The most common genres, each with the backdrop of its best rated title
/// not already on an earlier tile, so neighbouring tiles differ.
pub(crate) fn genre_tiles(conn: &Connection) -> rusqlite::Result<Vec<GenreTile>> {
    let counts: Vec<(String, u32)> = conn
        .prepare_cached(
            "SELECT g.genre, count(*) FROM item_genre g \
             JOIN item i ON i.id = g.item_id AND i.missing_since IS NULL AND i.kind IN ('movie', 'show') \
             GROUP BY g.genre ORDER BY count(*) DESC, g.genre LIMIT ?1",
        )?
        .query_map([GENRE_TILES], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut used = std::collections::HashSet::new();
    let mut tiles = Vec::with_capacity(counts.len());
    for (key, count) in counts {
        let candidates: Vec<(i64, Option<Artwork>)> = conn
            .prepare_cached(
                "SELECT g.item_id, a.hash, a.width, a.height, a.placeholder FROM item_genre g \
                 JOIN item i ON i.id = g.item_id AND i.missing_since IS NULL \
                 JOIN item_artwork a ON a.item_id = g.item_id AND a.role = 'backdrop' \
                 LEFT JOIN item_metadata m ON m.item_id = g.item_id AND m.source = 'online' \
                 WHERE g.genre = ?1 ORDER BY m.rating DESC NULLS LAST, g.item_id DESC LIMIT 8",
            )?
            .query_map([&key], |r| {
                Ok((r.get(0)?, views::artwork(r, 1, ArtworkRole::Backdrop)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let pick = candidates
            .iter()
            .find(|(id, _)| !used.contains(id))
            .or(candidates.first())
            .cloned();
        if let Some((id, _)) = &pick {
            used.insert(*id);
        }
        tiles.push(GenreTile {
            name: genres::display_name(&key),
            key,
            count,
            art: pick.and_then(|(_, art)| art),
        });
    }
    Ok(tiles)
}

/// One genre's present movies and shows, newest first, after `after`
/// (`<added_at_ms>.<id>`). `None` for a genre no title has.
pub(crate) fn genre_page(
    conn: &Connection,
    key: &str,
    after: Option<(i64, i64)>,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<Option<GenrePage>> {
    let total: u32 = conn
        .prepare_cached(
            "SELECT count(*) FROM item_genre g JOIN item i ON i.id = g.item_id \
             WHERE g.genre = ?1 AND i.missing_since IS NULL AND i.kind IN ('movie', 'show')",
        )?
        .query_row([key], |r| r.get(0))?;
    if total == 0 {
        return Ok(None);
    }
    let (at, id) = after.unwrap_or((i64::MAX, i64::MAX));
    let mut rows: Vec<(ItemId, i64)> = conn
        .prepare_cached(
            "SELECT i.id, i.added_at FROM item_genre g JOIN item i ON i.id = g.item_id \
             WHERE g.genre = ?1 AND i.missing_since IS NULL AND i.kind IN ('movie', 'show') \
             AND (i.added_at < ?2 OR (i.added_at = ?2 AND i.id < ?3)) \
             ORDER BY i.added_at DESC, i.id DESC LIMIT ?4",
        )?
        .query_map(rusqlite::params![key, at, id, i64::from(limit) + 1], |r| {
            Ok((ItemId(r.get(0)?), r.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let next = (rows.len() > limit as usize).then(|| {
        rows.truncate(limit as usize);
        let (id, at) = rows.last().expect("limit >= 1");
        format!("{at}.{}", id.0)
    });
    let mut cards = Vec::with_capacity(rows.len());
    for (id, _) in rows {
        cards.extend(title_card(conn, id, user)?);
    }
    Ok(Some(GenrePage {
        name: genres::display_name(key),
        total,
        cards,
        next,
    }))
}

/// Titles sharing the most genres with `id`, best rated first; topped up
/// with the newest titles of the same library when genres run short.
pub(crate) fn similar(conn: &Connection, id: ItemId, user: UserId) -> rusqlite::Result<Vec<Card>> {
    let mut ids: Vec<ItemId> = conn
        .prepare_cached(
            "SELECT g2.item_id FROM item_genre g1 \
             JOIN item_genre g2 ON g2.genre = g1.genre AND g2.item_id != g1.item_id \
             JOIN item i ON i.id = g2.item_id AND i.missing_since IS NULL AND i.kind IN ('movie', 'show') \
             LEFT JOIN item_metadata m ON m.item_id = g2.item_id AND m.source = 'online' \
             WHERE g1.item_id = ?1 \
             GROUP BY g2.item_id ORDER BY count(*) DESC, max(m.rating) DESC NULLS LAST, g2.item_id DESC LIMIT ?2",
        )?
        .query_map([id.0, i64::from(SIMILAR_LIMIT)], |r| Ok(ItemId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if ids.len() < SIMILAR_LIMIT as usize {
        let more: Vec<ItemId> = conn
            .prepare_cached(
                "SELECT o.id FROM item t JOIN item o ON o.library_id = t.library_id AND o.kind = t.kind \
                 WHERE t.id = ?1 AND o.id != t.id AND o.missing_since IS NULL \
                 ORDER BY o.added_at DESC, o.id DESC LIMIT ?2",
            )?
            .query_map([id.0, i64::from(SIMILAR_LIMIT)], |r| Ok(ItemId(r.get(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for other in more {
            if ids.len() >= SIMILAR_LIMIT as usize {
                break;
            }
            if !ids.contains(&other) {
                ids.push(other);
            }
        }
    }
    let mut cards = Vec::with_capacity(ids.len());
    for other in ids {
        cards.extend(title_card(conn, other, user)?);
    }
    Ok(cards)
}

/// Up to [`FEATURED_LIMIT`] titles with a backdrop for the hero: new titles
/// the user has not finished, then what they are in the middle of, then the
/// best rated ones they have not watched.
pub(crate) fn featured(
    conn: &Connection,
    user: UserId,
    now_ms: i64,
) -> rusqlite::Result<Vec<Featured>> {
    const NEW_WINDOW_MS: i64 = 60 * 24 * 3600 * 1000;
    let mut picks: Vec<ItemId> = Vec::new();
    let take = |ids: Vec<ItemId>, max: usize, picks: &mut Vec<ItemId>| {
        let mut taken = 0;
        for id in ids {
            if taken == max || picks.len() == FEATURED_LIMIT {
                break;
            }
            if !picks.contains(&id) {
                picks.push(id);
                taken += 1;
            }
        }
    };
    // New: movies by added time, shows by their newest episode.
    let new: Vec<ItemId> = conn
        .prepare_cached(
            "WITH recent(id, at) AS ( \
               SELECT id, added_at FROM item WHERE kind = 'movie' AND missing_since IS NULL \
               UNION ALL \
               SELECT se.parent_id, max(e.added_at) FROM item e JOIN item se ON se.id = e.parent_id \
               WHERE e.kind = 'episode' AND e.missing_since IS NULL GROUP BY se.parent_id) \
             SELECT r.id FROM recent r \
             WHERE r.at >= ?1 AND EXISTS (SELECT 1 FROM item_artwork a WHERE a.item_id = r.id AND a.role = 'backdrop') \
             AND NOT EXISTS (SELECT 1 FROM watch_state w WHERE w.user_id = ?2 AND w.item_id = r.id AND w.play_count > 0) \
             ORDER BY r.at DESC, r.id DESC LIMIT 12",
        )?
        .query_map(rusqlite::params![now_ms - NEW_WINDOW_MS, user.0], |r| Ok(ItemId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    take(new, 3, &mut picks);
    let watching: Vec<ItemId> = shows::continue_watching(conn, user)?
        .into_iter()
        .map(|(card, _)| card.show.unwrap_or(card.id))
        .collect();
    take(watching, 2, &mut picks);
    let rated: Vec<ItemId> = conn
        .prepare_cached(
            "SELECT i.id FROM item i JOIN item_metadata m ON m.item_id = i.id AND m.source = 'online' \
             WHERE i.kind IN ('movie', 'show') AND i.missing_since IS NULL AND m.rating IS NOT NULL \
             AND EXISTS (SELECT 1 FROM item_artwork a WHERE a.item_id = i.id AND a.role = 'backdrop') \
             AND NOT EXISTS (SELECT 1 FROM watch_state w WHERE w.user_id = ?1 AND w.item_id = i.id AND w.play_count > 0) \
             ORDER BY m.rating DESC, i.id DESC LIMIT 12",
        )?
        .query_map([user.0], |r| Ok(ItemId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    take(rated, FEATURED_LIMIT, &mut picks);
    // Anything else with a backdrop, so a fresh server still has a hero.
    if picks.is_empty() {
        let any: Vec<ItemId> = conn
            .prepare_cached(
                "SELECT i.id FROM item i WHERE i.kind IN ('movie', 'show') AND i.missing_since IS NULL \
                 AND EXISTS (SELECT 1 FROM item_artwork a WHERE a.item_id = i.id AND a.role = 'backdrop') \
                 ORDER BY i.added_at DESC, i.id DESC LIMIT ?1",
            )?
            .query_map([FEATURED_LIMIT as i64], |r| Ok(ItemId(r.get(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        take(any, FEATURED_LIMIT, &mut picks);
    }
    let mut out = Vec::with_capacity(picks.len());
    for id in picks {
        if let Some(f) = featured_one(conn, id, user)? {
            out.push(f);
        }
    }
    Ok(out)
}

fn featured_one(conn: &Connection, id: ItemId, user: UserId) -> rusqlite::Result<Option<Featured>> {
    let Some(card) = title_card(conn, id, user)? else {
        return Ok(None);
    };
    if card.backdrop.is_none() {
        return Ok(None);
    }
    let (overview, tagline, labels, rating): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<f64>,
    ) = conn
        .prepare_cached(
            "SELECT d.overview, d.tagline, m.genres, m.rating FROM item_display d \
             LEFT JOIN item_metadata m ON m.item_id = d.id AND m.source = 'online' WHERE d.id = ?1",
        )?
        .query_row([id.0], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let genres = labels
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default()
        .iter()
        .filter_map(|label| genres::key(label))
        .fold(Vec::new(), |mut names: Vec<String>, key| {
            let name = genres::display_name(&key);
            if !names.contains(&name) {
                names.push(name);
            }
            names
        });
    let (runtime_ms, season_count, next) = match card.kind {
        ItemKind::Show => {
            let show = shows::show(conn, id, user)?;
            let seasons = show.seasons.iter().filter(|s| s.number > 0).count() as u32;
            (None, Some(seasons), show.next)
        }
        _ => {
            let runtime: Option<i64> = conn
                .prepare_cached("SELECT max(duration_ms) FROM media_file WHERE item_id = ?1")?
                .query_row([id.0], |r| r.get(0))?;
            (runtime.map(|ms| ms.max(0) as u64), None, None)
        }
    };
    Ok(Some(Featured {
        card,
        overview,
        tagline,
        genres,
        rating: rating.map(|r| r as f32),
        runtime_ms,
        season_count,
        next,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::test_support::seed_movies;

    const DAY: i64 = 24 * 3600 * 1000;
    const NOW: i64 = 1_800_000_000_000;

    /// Twelve movies (ids 1..=12), three users, genres from three providers'
    /// spellings, and a backdrop on movies 1..=6.
    async fn seeded() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 2).unwrap();
        db.write(|tx| {
            seed_movies(tx, 12)?;
            for name in ["ada", "bo", "cy"] {
                tx.execute(
                    "INSERT INTO user (username, display_name, password_hash, role, created_at) VALUES (?1, ?1, 'x', 'member', 0)",
                    [name],
                )?;
            }
            let genres: [(i64, &str, Option<f64>); 5] = [
                (1, r#"["Science-Fiction","Drama"]"#, Some(8.0)),
                (2, r#"["Sci-Fi","Thriller"]"#, Some(9.0)),
                (3, r#"["science fiction film"]"#, None),
                (4, r#"["Drama"]"#, Some(7.0)),
                (5, r#"["Comedy"]"#, Some(6.0)),
            ];
            for (id, json, rating) in genres {
                tx.execute(
                    "INSERT INTO item_metadata (item_id, source, title, updated_at, genres, rating) VALUES (?1, 'online', NULL, 0, ?2, ?3)",
                    rusqlite::params![id, json, rating],
                )?;
                let labels: Vec<String> = serde_json::from_str(json).unwrap();
                genres::index_item(tx, ItemId(id), &labels)?;
            }
            for id in 1..=6_i64 {
                tx.execute(
                    "INSERT INTO artwork (item_id, role, source, source_sig, hash, width, height, placeholder) \
                     VALUES (?1, 'backdrop', 'local', 'b', randomblob(16), 1920, 1080, X'0102')",
                    [id],
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
        (dir, db)
    }

    async fn watch(db: &Db, user: i64, item: i64, plays: i64, at: i64) {
        db.write(move |tx| {
            tx.execute(
                "INSERT INTO watch_state (user_id, item_id, play_count, updated_at) VALUES (?1, ?2, ?3, ?4)",
                [user, item, plays, at],
            )
            .map(drop)
        })
        .await
        .unwrap();
    }

    fn ids(cards: &[Card]) -> Vec<i64> {
        cards.iter().map(|c| c.id.0).collect()
    }

    #[tokio::test]
    async fn top_ranks_by_viewers_then_plays_and_trending_keeps_to_the_week() {
        let (_dir, db) = seeded().await;
        let me = UserId(1);
        // Movie 7: three viewers. Movie 8: two viewers. Movie 9: one viewer, watched twice, a month ago.
        for user in 1..=3 {
            watch(&db, user, 7, 1, NOW - DAY).await;
        }
        watch(&db, 1, 8, 1, NOW - 2 * DAY).await;
        watch(&db, 2, 8, 1, NOW - 2 * DAY).await;
        watch(&db, 3, 9, 2, NOW - 30 * DAY).await;
        let (top, trending) = db
            .read(move |c| Ok((top(c, me)?, trending(c, me, NOW)?)))
            .await
            .unwrap();
        assert_eq!(ids(&top), [7, 8, 9]);
        assert!(
            top.iter().all(|c| c.backdrop.is_none()),
            "movies 7-9 have no backdrop"
        );
        assert!(
            trending.is_empty(),
            "two titles this week is too few to rank"
        );
    }

    #[tokio::test]
    async fn genres_merge_provider_spellings_and_similar_shares_them() {
        let (_dir, db) = seeded().await;
        let (tiles, similar, page) = db
            .read(|c| {
                Ok((
                    genre_tiles(c)?,
                    similar(c, ItemId(1), UserId(1))?,
                    genre_page(c, "science fiction", None, 2, UserId(1))?.unwrap(),
                ))
            })
            .await
            .unwrap();
        let named: Vec<(&str, u32)> = tiles.iter().map(|t| (t.name.as_str(), t.count)).collect();
        assert_eq!(
            named,
            [
                ("Science Fiction", 3),
                ("Drama", 2),
                ("Comedy", 1),
                ("Thriller", 1)
            ]
        );
        assert_ne!(
            tiles[0].art.as_ref().map(|a| a.id),
            tiles[1].art.as_ref().map(|a| a.id),
            "neighbouring tiles use different titles' art"
        );
        // Each shares one genre with movie 1, so rating decides: 2 (9.0), 4 (7.0), then unrated 3.
        assert_eq!(&ids(&similar)[..3], [2, 4, 3]);
        assert!(!ids(&similar).contains(&1));
        assert_eq!(
            (page.total, page.cards.len(), page.next.is_some()),
            (3, 2, true)
        );
    }

    #[tokio::test]
    async fn featured_prefers_titles_with_backdrops_and_skips_what_is_finished() {
        let (_dir, db) = seeded().await;
        // Every seeded movie is older than two months, so the hero falls back
        // to ratings: 2 (9.0), 1 (8.0), 4, 5; then the finished one is dropped.
        watch(&db, 1, 2, 1, NOW).await;
        let featured = db.read(|c| featured(c, UserId(1), NOW)).await.unwrap();
        let picked: Vec<i64> = featured.iter().map(|f| f.card.id.0).collect();
        assert_eq!(picked, [1, 4, 5]);
        assert!(featured.iter().all(|f| f.card.backdrop.is_some()));
        assert_eq!(featured[0].genres, ["Science Fiction", "Drama"]);
    }
}
