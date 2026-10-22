//! Shows, seasons and episodes as the show page, the player and Continue
//! Watching read them. Watch state stays per episode; everything a show
//! "has" (progress, the next episode, counts) is derived here from its
//! episodes' states, for one user.
//!
//! Episode order is seasons ascending, Specials (season 0) last, episodes
//! ascending. Specials never come next: Next Up follows the numbered seasons.

use std::collections::HashMap;

use nuvora_media::{ArtworkRole, DeviceProfile};
use rusqlite::{Connection, OptionalExtension};

use crate::views::{self, Artwork, Card, CardNote, EpisodeNumber, ItemKind, Version};
use crate::watch_state::{self, WatchState};
use crate::{ItemId, UserId};

/// A show in Recently Added counts the episodes added in this window before
/// its newest one as new.
pub(crate) const NEW_EPISODE_WINDOW_MS: i64 = 7 * 24 * 3600 * 1000;

#[derive(Debug, Clone)]
pub struct Show {
    pub id: ItemId,
    pub title: String,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub poster: Option<Artwork>,
    pub backdrop: Option<Artwork>,
    /// Numbered seasons in order, then Specials.
    pub seasons: Vec<SeasonSummary>,
    /// The first unwatched episode after the last watched one; the first
    /// episode when nothing is watched; `None` when the last one is watched.
    pub next: Option<EpisodeRef>,
    pub watched_count: u32,
    pub episode_count: u32,
}

#[derive(Debug, Clone)]
pub struct SeasonSummary {
    pub id: ItemId,
    /// 0 for Specials.
    pub number: u32,
    pub episode_count: u32,
    pub watched_count: u32,
}

#[derive(Debug, Clone)]
pub struct Season {
    pub id: ItemId,
    pub show: ItemId,
    pub show_title: String,
    pub number: u32,
    /// The show's.
    pub poster: Option<Artwork>,
    pub backdrop: Option<Artwork>,
    pub episode_count: u32,
    pub watched_count: u32,
}

#[derive(Debug, Clone)]
pub struct EpisodeRef {
    pub id: ItemId,
    pub season_id: ItemId,
    pub number: EpisodeNumber,
    /// From the file name; `None` when it has none.
    pub title: Option<String>,
    /// The requesting user's.
    pub watch: WatchState,
    pub thumb: Option<Artwork>,
}

#[derive(Debug, Clone)]
pub struct Episode {
    pub id: ItemId,
    pub show: ItemId,
    pub season_id: ItemId,
    pub show_title: String,
    pub number: EpisodeNumber,
    pub title: Option<String>,
    pub overview: Option<String>,
    /// The show's.
    pub poster: Option<Artwork>,
    /// The show's.
    pub backdrop: Option<Artwork>,
    pub thumb: Option<Artwork>,
    /// Best first; empty when every file is missing.
    pub versions: Vec<Version>,
    pub watch: WatchState,
    /// The episode after this one (Specials only lead to Specials). Set on
    /// the item read, not on season lists.
    pub next: Option<EpisodeRef>,
}

impl Episode {
    pub fn runtime_ms(&self) -> Option<u64> {
        views::runtime_ms(&self.versions)
    }
}

struct ShowHead {
    title: String,
    year: Option<u16>,
    overview: Option<String>,
    poster: Option<Artwork>,
    backdrop: Option<Artwork>,
}

fn show_head(conn: &Connection, show: ItemId) -> rusqlite::Result<Option<ShowHead>> {
    conn.prepare_cached(
        "SELECT d.title, d.year, d.overview, p.hash, p.width, p.height, p.placeholder, \
         b.hash, b.width, b.height, b.placeholder FROM item_display d \
         LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' \
         LEFT JOIN item_artwork b ON b.item_id = d.id AND b.role = 'backdrop' \
         WHERE d.id = ?1 AND d.kind = 'show'",
    )?
    .query_row([show.0], |r| {
        Ok(ShowHead {
            title: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            year: r.get(1)?,
            overview: r.get(2)?,
            poster: views::artwork(r, 3, ArtworkRole::Poster)?,
            backdrop: views::artwork(r, 7, ArtworkRole::Backdrop)?,
        })
    })
    .optional()
}

/// One episode of a show, with the user's state.
struct ShowEpisode {
    episode: EpisodeRef,
    present: bool,
    /// When the user's state last changed.
    updated_at: Option<i64>,
}

impl ShowEpisode {
    fn special(&self) -> bool {
        self.episode.number.season == 0
    }
}

/// Every episode of `show` in order, missing ones included (flagged).
fn show_episodes(
    conn: &Connection,
    show: ItemId,
    user: UserId,
) -> rusqlite::Result<Vec<ShowEpisode>> {
    conn.prepare_cached(
        "SELECT e.id, se.id, se.index_number, e.index_number, d.title, \
         w.file_id, w.position_ms, w.duration_ms, w.play_count, w.last_watched_at, w.updated_at, \
         t.hash, t.width, t.height, t.placeholder, e.missing_since IS NULL \
         FROM item se JOIN item e ON e.parent_id = se.id JOIN item_display d ON d.id = e.id \
         LEFT JOIN watch_state w ON w.user_id = ?2 AND w.item_id = e.id \
         LEFT JOIN item_artwork t ON t.item_id = e.id AND t.role = 'backdrop' \
         WHERE se.parent_id = ?1 AND se.kind = 'season' \
         ORDER BY se.index_number = 0, se.index_number, e.index_number, e.id",
    )?
    .query_map([show.0, user.0], |r| {
        Ok(ShowEpisode {
            episode: EpisodeRef {
                id: ItemId(r.get(0)?),
                season_id: ItemId(r.get(1)?),
                number: EpisodeNumber {
                    season: r.get::<_, Option<u32>>(2)?.unwrap_or(0),
                    episode: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                },
                title: r.get(4)?,
                watch: watch_state::state_columns(r, 5)?,
                thumb: views::artwork(r, 11, ArtworkRole::Backdrop)?,
            },
            updated_at: r.get(10)?,
            present: r.get(15)?,
        })
    })?
    .collect()
}

/// The last watched numbered episode, in order.
fn last_watched(episodes: &[ShowEpisode]) -> Option<usize> {
    episodes
        .iter()
        .rposition(|e| e.present && !e.special() && e.episode.watch.watched())
}

/// The episode Play starts: the first present numbered episode after the
/// last watched one, or the first one when none is watched.
fn next_up(episodes: &[ShowEpisode]) -> Option<usize> {
    let from = last_watched(episodes).map_or(0, |i| i + 1);
    (from..episodes.len()).find(|&i| episodes[i].present && !episodes[i].special())
}

pub(crate) fn show(conn: &Connection, id: ItemId, user: UserId) -> rusqlite::Result<Show> {
    let head = show_head(conn, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let episodes = show_episodes(conn, id, user)?;
    let mut seasons: Vec<SeasonSummary> = Vec::new();
    for e in episodes.iter().filter(|e| e.present) {
        let watched = u32::from(e.episode.watch.watched());
        match seasons.last_mut() {
            Some(season) if season.id == e.episode.season_id => {
                season.episode_count += 1;
                season.watched_count += watched;
            }
            _ => seasons.push(SeasonSummary {
                id: e.episode.season_id,
                number: e.episode.number.season,
                episode_count: 1,
                watched_count: watched,
            }),
        }
    }
    Ok(Show {
        id,
        title: head.title,
        year: head.year,
        overview: head.overview,
        poster: head.poster,
        backdrop: head.backdrop,
        next: next_up(&episodes).map(|i| episodes[i].episode.clone()),
        watched_count: seasons.iter().map(|s| s.watched_count).sum(),
        episode_count: seasons.iter().map(|s| s.episode_count).sum(),
        seasons,
    })
}

/// `(show, number)` of a season id; `None` for any other id.
fn season_of(conn: &Connection, season: ItemId) -> rusqlite::Result<Option<(ItemId, u32)>> {
    conn.prepare_cached(
        "SELECT parent_id, index_number FROM item WHERE id = ?1 AND kind = 'season'",
    )?
    .query_row([season.0], |r| {
        Ok((ItemId(r.get(0)?), r.get::<_, Option<u32>>(1)?.unwrap_or(0)))
    })
    .optional()
}

pub(crate) fn season(
    conn: &Connection,
    id: ItemId,
    user: UserId,
) -> rusqlite::Result<Option<Season>> {
    let Some((show, number)) = season_of(conn, id)? else {
        return Ok(None);
    };
    let head = show_head(conn, show)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let episodes = show_episodes(conn, show, user)?;
    let present = episodes
        .iter()
        .filter(|e| e.present && e.episode.season_id == id);
    let (episode_count, watched_count) = present.fold((0, 0), |(n, w), e| {
        (n + 1, w + u32::from(e.episode.watch.watched()))
    });
    Ok(Some(Season {
        id,
        show,
        show_title: head.title,
        number,
        poster: head.poster,
        backdrop: head.backdrop,
        episode_count,
        watched_count,
    }))
}

fn episode_from(
    conn: &Connection,
    show: ItemId,
    head: &ShowHead,
    e: &ShowEpisode,
    device: &DeviceProfile,
) -> rusqlite::Result<Episode> {
    let overview: Option<String> = conn
        .prepare_cached("SELECT overview FROM item_display WHERE id = ?1")?
        .query_row([e.episode.id.0], |r| r.get(0))?;
    Ok(Episode {
        id: e.episode.id,
        show,
        season_id: e.episode.season_id,
        show_title: head.title.clone(),
        number: e.episode.number,
        title: e.episode.title.clone(),
        overview,
        poster: head.poster.clone(),
        backdrop: head.backdrop.clone(),
        thumb: e.episode.thumb.clone(),
        versions: views::versions(conn, e.episode.id, device)?,
        watch: e.episode.watch,
        next: None,
    })
}

/// The show an episode id belongs to; `None` for any other id.
fn show_of_episode(conn: &Connection, episode: ItemId) -> rusqlite::Result<Option<ItemId>> {
    conn.prepare_cached(
        "SELECT se.parent_id FROM item e JOIN item se ON se.id = e.parent_id \
         WHERE e.id = ?1 AND e.kind = 'episode'",
    )?
    .query_row([episode.0], |r| Ok(ItemId(r.get(0)?)))
    .optional()
}

pub(crate) fn episode(
    conn: &Connection,
    id: ItemId,
    device: &DeviceProfile,
    user: UserId,
) -> rusqlite::Result<Option<Episode>> {
    let Some(show) = show_of_episode(conn, id)? else {
        return Ok(None);
    };
    let head = show_head(conn, show)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let episodes = show_episodes(conn, show, user)?;
    let Some(at) = episodes.iter().position(|e| e.episode.id == id) else {
        return Ok(None);
    };
    let mut episode = episode_from(conn, show, &head, &episodes[at], device)?;
    episode.next = episodes[at + 1..]
        .iter()
        .find(|e| e.present && e.special() == episodes[at].special())
        .map(|e| e.episode.clone());
    Ok(Some(episode))
}

/// A season's present episodes in order; `None` unless `season` is a season.
pub(crate) fn episodes(
    conn: &Connection,
    season: ItemId,
    device: &DeviceProfile,
    user: UserId,
) -> rusqlite::Result<Option<Vec<Episode>>> {
    let Some((show, _)) = season_of(conn, season)? else {
        return Ok(None);
    };
    let head = show_head(conn, show)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    show_episodes(conn, show, user)?
        .iter()
        .filter(|e| e.present && e.episode.season_id == season)
        .map(|e| episode_from(conn, show, &head, e, device))
        .collect::<rusqlite::Result<_>>()
        .map(Some)
}

fn episode_card(show: ItemId, head: &ShowHead, e: &EpisodeRef, note: CardNote) -> Card {
    let mut card = Card::new(e.id, ItemKind::Episode, head.title.clone(), head.year);
    card.poster = head.poster.clone();
    card.watch = e.watch;
    card.note = note;
    card.show = Some(show);
    card
}

/// `user`'s Continue Watching, most recent activity first (ties: higher id
/// first), each card with its activity time: movies and episodes in
/// progress, plus a Next Up entry for each show whose last watched episode
/// has a successor not yet started.
pub(crate) fn continue_watching(
    conn: &Connection,
    user: UserId,
) -> rusqlite::Result<Vec<(Card, i64)>> {
    let mut out = Vec::new();
    let in_progress: Vec<(ItemId, String, i64)> = conn
        .prepare_cached(
            "SELECT w.item_id, i.kind, w.updated_at FROM watch_state w JOIN item i ON i.id = w.item_id \
             WHERE w.user_id = ?1 AND w.position_ms IS NOT NULL AND i.missing_since IS NULL \
             AND i.kind IN ('movie', 'episode')",
        )?
        .query_map([user.0], |r| Ok((ItemId(r.get(0)?), r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut shows: HashMap<ItemId, (ShowHead, Vec<ShowEpisode>)> = HashMap::new();
    let mut load_show = |show: ItemId| -> rusqlite::Result<bool> {
        if let std::collections::hash_map::Entry::Vacant(slot) = shows.entry(show) {
            let Some(head) = show_head(conn, show)? else {
                return Ok(false);
            };
            slot.insert((head, show_episodes(conn, show, user)?));
        }
        Ok(true)
    };
    let mut episode_cards = Vec::new();
    for (id, kind, at) in in_progress {
        if kind == "movie" {
            if let Some(card) = views::card_by_id(conn, id, user)? {
                out.push((card, at));
            }
        } else if let Some(show) = show_of_episode(conn, id)?
            && load_show(show)?
        {
            episode_cards.push((show, id, at));
        }
    }
    let watched_shows: Vec<ItemId> = conn
        .prepare_cached(
            "SELECT DISTINCT se.parent_id FROM watch_state w JOIN item e ON e.id = w.item_id \
             JOIN item se ON se.id = e.parent_id JOIN item s ON s.id = se.parent_id \
             WHERE w.user_id = ?1 AND w.play_count > 0 AND e.kind = 'episode' \
             AND se.index_number != 0 AND s.missing_since IS NULL",
        )?
        .query_map([user.0], |r| Ok(ItemId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for show in &watched_shows {
        load_show(*show)?;
    }
    for (show, id, at) in episode_cards {
        let (head, episodes) = &shows[&show];
        if let Some(e) = episodes.iter().find(|e| e.episode.id == id) {
            let note = CardNote::Episode(e.episode.number);
            out.push((episode_card(show, head, &e.episode, note), at));
        }
    }
    for show in watched_shows {
        let Some((head, episodes)) = shows.get(&show) else {
            continue;
        };
        let (Some(last), Some(next)) = (last_watched(episodes), next_up(episodes)) else {
            continue;
        };
        let next = &episodes[next].episode;
        if next.watch.resume.is_none() {
            let at = episodes[last].updated_at.unwrap_or(0);
            let note = CardNote::NextUp(next.number);
            out.push((episode_card(show, head, next, note), at));
        }
    }
    out.sort_by(|(a, at_a), (b, at_b)| (at_b, b.id).cmp(&(at_a, a.id)));
    Ok(out)
}

/// A show's episode counts and watch totals for one user.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShowStats {
    pub episodes: u32,
    pub watched: u32,
    pub in_progress: u32,
    pub last_watched_at: Option<i64>,
    /// The newest episode's `added_at`.
    pub newest_added_at: i64,
    /// Sum of the episodes' longest file.
    pub runtime_ms: u64,
}

impl ShowStats {
    /// The show's watch state as a card shows it: watched once every
    /// present episode is.
    pub(crate) fn watch(&self) -> WatchState {
        WatchState {
            resume: None,
            play_count: u32::from(self.episodes > 0 && self.watched == self.episodes),
            last_watched_at: self.last_watched_at,
        }
    }
}

/// [`ShowStats`] of every show matched by `scope` (`se.parent_id = ?2` or
/// `e.library_id = ?2`), counting present episodes only.
pub(crate) fn show_stats(
    conn: &Connection,
    scope: &str,
    value: i64,
    user: UserId,
) -> rusqlite::Result<HashMap<ItemId, ShowStats>> {
    conn.prepare_cached(&format!(
        "SELECT se.parent_id, count(*), \
         coalesce(sum(w.play_count > 0), 0), coalesce(sum(w.position_ms IS NOT NULL), 0), \
         max(w.last_watched_at), max(e.added_at), \
         coalesce(sum((SELECT max(f.duration_ms) FROM media_file f WHERE f.item_id = e.id)), 0) \
         FROM item e JOIN item se ON se.id = e.parent_id \
         LEFT JOIN watch_state w ON w.user_id = ?1 AND w.item_id = e.id \
         WHERE e.kind = 'episode' AND e.missing_since IS NULL AND {scope} \
         GROUP BY se.parent_id"
    ))?
    .query_map([user.0, value], |r| {
        Ok((
            ItemId(r.get(0)?),
            ShowStats {
                episodes: r.get(1)?,
                watched: r.get(2)?,
                in_progress: r.get(3)?,
                last_watched_at: r.get(4)?,
                newest_added_at: r.get(5)?,
                runtime_ms: r.get::<_, i64>(6)?.max(0) as u64,
            },
        ))
    })?
    .collect()
}

/// Sets a show card's watch state and unwatched count for `user`.
pub(crate) fn fill_show_card(
    conn: &Connection,
    card: &mut Card,
    user: UserId,
) -> rusqlite::Result<()> {
    let stats = show_stats(conn, "se.parent_id = ?2", card.id.0, user)?
        .remove(&card.id)
        .unwrap_or_default();
    card.watch = stats.watch();
    card.unwatched = Some(stats.episodes - stats.watched);
    Ok(())
}

/// Present episodes of `show` added at or after `since_ms`.
pub(crate) fn added_since(conn: &Connection, show: ItemId, since_ms: i64) -> rusqlite::Result<u32> {
    conn.prepare_cached(
        "SELECT count(*) FROM item e JOIN item se ON se.id = e.parent_id \
         WHERE se.parent_id = ?1 AND e.kind = 'episode' AND e.missing_since IS NULL AND e.added_at >= ?2",
    )?
    .query_row([show.0, since_ms], |r| r.get(0))
}
