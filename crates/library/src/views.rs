//! Read models: what each screen needs, assembled from SQL in one read
//! transaction. These are domain types (no serde); the server maps them to its
//! wire types. All SQL that reads the catalog lives in this file.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use nuvora_media::{
    ArtworkRole, AudioCodec, AudioStream, ColorInfo, Container, DeviceProfile, HdrFormat,
    MediaInfo, Placeholder, PlaybackDecision, SubtitleCodec, SubtitleStream, VideoCodec,
    VideoStream,
};
use rusqlite::Connection;

use crate::browse::{self, LibraryCursor, LibraryQuery};
use crate::shows::{self, Episode, Season, Show};
use crate::watch_state::{self, WatchState};
use crate::{ArtworkId, FileId, ItemId, LibraryId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Movie,
    Show,
    Season,
    Episode,
}

/// The catalog revision: bumped by every write transaction that changes what a
/// client can see. Equal revs mean equal home and row responses, so the server
/// uses it as their ETag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CatalogRev(pub(crate) u64);

impl CatalogRev {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// One user's home screen: an ordered list of rows, each bounded to its first
/// page so the payload does not grow with the library. The server decides which
/// rows exist and in what order, so a new row appears on every client without a
/// client release.
#[derive(Debug, Clone)]
pub struct Home {
    /// Read in the same transaction as the rows, so it is the ETag of exactly this body.
    pub rev: CatalogRev,
    pub rows: Vec<Row>,
}

/// The first page of one row.
#[derive(Debug, Clone)]
pub struct Row {
    pub kind: RowKind,
    /// Cards in the whole row, not the page.
    pub total: u32,
    /// At most [`HOME_ROW_LIMIT`].
    pub cards: Vec<Card>,
    /// `None` when `cards` is the whole row.
    pub next: Option<Cursor>,
    /// The library's name, on a [`RowKind::Library`] row.
    pub library_name: Option<String>,
    /// The library's preset icon, on a [`RowKind::Library`] row.
    pub library_icon: Option<String>,
    /// The user's own title for this row, from their home layout.
    pub title: Option<String>,
    /// On a custom library row: the query its later pages come from, through
    /// the library page, and the cursor to the second page.
    pub query: Option<crate::browse::LibraryQuery>,
    pub library_next: Option<crate::browse::LibraryCursor>,
    /// Index of the [`crate::HomeRowSpec`] this row was built from.
    pub spec: Option<usize>,
}

/// A later page of one row, from [`Library::row`](crate::Library::row).
#[derive(Debug, Clone)]
pub struct RowPage {
    pub rev: CatalogRev,
    pub cards: Vec<Card>,
    pub next: Option<Cursor>,
}

/// Cards per row on the home screen.
pub const HOME_ROW_LIMIT: u32 = 40;
/// Upper bound on a requested page size; larger requests are clamped.
pub const MAX_ROW_LIMIT: u32 = 100;

/// A home row. Each kind owns its sort key, and so its keyset cursor shape.
/// Adding a row is a variant here plus its arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// The user's movies and episodes in progress, plus the next episode of
    /// each show they are watching, most recent activity first:
    /// `activity DESC, id DESC`. On home only when non-empty.
    ContinueWatching,
    /// Movies, and shows by their newest episode, newest first:
    /// `added DESC, id DESC`.
    RecentlyAdded,
    /// Every movie of every movie library by title: `sort_title COLLATE
    /// NOCASE, id`. Not on home; kept for clients that page it.
    AllMovies,
    /// One library's movies or shows, as its library page sorts them by
    /// title. Home has one per library, in the order they were added.
    Library(LibraryId),
}

/// An opaque keyset position inside one row: the sort key of the last card of
/// the previous page. Clients only pass it back. The encoding is private, so a
/// sort key can change without a client change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor(CursorKey);

#[derive(Debug, Clone, PartialEq, Eq)]
enum CursorKey {
    ContinueWatching { updated_at_ms: i64, id: ItemId },
    RecentlyAdded { added_at_ms: i64, id: ItemId },
    AllMovies { sort_title: String, id: ItemId },
    Library(LibraryId, LibraryCursor),
}

#[derive(Debug, thiserror::Error)]
#[error("invalid cursor")]
pub struct InvalidCursor;

impl Cursor {
    /// Parse an untrusted string (a query parameter) as a cursor of `kind`'s row.
    /// Garbage, or a cursor minted by another row, is an error, never a panic.
    pub fn parse(kind: RowKind, raw: &str) -> Result<Cursor, InvalidCursor> {
        let key = match (kind, raw.split_at_checked(1)) {
            (RowKind::ContinueWatching, Some(("c", rest))) => {
                let (at, id) = rest.split_once('.').ok_or(InvalidCursor)?;
                CursorKey::ContinueWatching {
                    updated_at_ms: at.parse().map_err(|_| InvalidCursor)?,
                    id: parse_id(id)?,
                }
            }
            (RowKind::RecentlyAdded, Some(("r", rest))) => {
                let (at, id) = rest.split_once('.').ok_or(InvalidCursor)?;
                CursorKey::RecentlyAdded {
                    added_at_ms: at.parse().map_err(|_| InvalidCursor)?,
                    id: parse_id(id)?,
                }
            }
            (RowKind::AllMovies, Some(("t", rest))) => {
                let (id, title) = rest.split_once('.').ok_or(InvalidCursor)?;
                CursorKey::AllMovies {
                    sort_title: hex_decode_utf8(title)?,
                    id: parse_id(id)?,
                }
            }
            (RowKind::Library(library), _) => CursorKey::Library(
                library,
                LibraryCursor::parse(&LibraryQuery::default(), raw)?,
            ),
            _ => return Err(InvalidCursor),
        };
        Ok(Cursor(key))
    }

    pub fn kind(&self) -> RowKind {
        match self.0 {
            CursorKey::ContinueWatching { .. } => RowKind::ContinueWatching,
            CursorKey::RecentlyAdded { .. } => RowKind::RecentlyAdded,
            CursorKey::AllMovies { .. } => RowKind::AllMovies,
            CursorKey::Library(library, _) => RowKind::Library(library),
        }
    }
}

/// URL-safe without escaping: `c<updated_at_ms>.<id>`, `r<added_at_ms>.<id>`
/// or `t<id>.<hex sort_title>`.
impl std::fmt::Display for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            CursorKey::ContinueWatching { updated_at_ms, id } => {
                write!(f, "c{updated_at_ms}.{}", id.0)
            }
            CursorKey::RecentlyAdded { added_at_ms, id } => write!(f, "r{added_at_ms}.{}", id.0),
            CursorKey::AllMovies { sort_title, id } => {
                write!(f, "t{}.", id.0)?;
                sort_title.bytes().try_for_each(|b| write!(f, "{b:02x}"))
            }
            CursorKey::Library(_, cursor) => cursor.fmt(f),
        }
    }
}

pub(crate) fn parse_id(s: &str) -> Result<ItemId, InvalidCursor> {
    s.parse().map(ItemId).map_err(|_| InvalidCursor)
}

pub(crate) fn hex_decode_utf8(s: &str) -> Result<String, InvalidCursor> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return Err(InvalidCursor);
    }
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| InvalidCursor))
        .collect::<Result<Vec<u8>, _>>()?;
    String::from_utf8(bytes).map_err(|_| InvalidCursor)
}

/// What a poster card needs, and nothing more. An episode card (in Continue
/// Watching) carries its show's title, year and poster.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: ItemId,
    pub kind: ItemKind,
    pub title: String,
    pub year: Option<u16>,
    pub poster: Option<Artwork>,
    /// The requesting user's. A show's is `watched` when every episode is.
    pub watch: WatchState,
    /// What the card's second line says.
    pub note: CardNote,
    /// On episode cards: the show.
    pub show: Option<ItemId>,
    /// On show cards: episodes the requesting user has not watched.
    pub unwatched: Option<u32>,
}

impl Card {
    pub(crate) fn new(id: ItemId, kind: ItemKind, title: String, year: Option<u16>) -> Card {
        Card {
            id,
            kind,
            title,
            year,
            poster: None,
            watch: WatchState::default(),
            note: CardNote::None,
            show: None,
            unwatched: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardNote {
    None,
    /// An episode in progress.
    Episode(EpisodeNumber),
    /// The next episode of a show the user is watching, not started.
    NextUp(EpisodeNumber),
    /// A show in Recently Added: episodes added in the week before its newest.
    NewEpisodes(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EpisodeNumber {
    /// 0 for Specials.
    pub season: u32,
    pub episode: u32,
}

/// A stored artwork as clients see it: an id to build size URLs from, its
/// aspect ratio for layout before load, and the inline placeholder.
#[derive(Debug, Clone)]
pub struct Artwork {
    pub id: ArtworkId,
    pub role: ArtworkRole,
    pub width: u32,
    pub height: u32,
    pub placeholder: Placeholder,
}

#[derive(Debug, Clone)]
pub enum Item {
    Movie(Movie),
    Show(Show),
    Season(Season),
    Episode(Episode),
}

#[derive(Debug, Clone)]
pub struct Movie {
    pub id: ItemId,
    pub title: String,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub added_at: SystemTime,
    pub poster: Option<Artwork>,
    pub backdrop: Option<Artwork>,
    /// Best first: playable before not, then height, then bitrate. Empty when
    /// every file is gone (the item is "missing" but its id still resolves).
    pub versions: Vec<Version>,
    /// The requesting user's.
    pub watch: WatchState,
    pub extras: crate::metadata::read::Extras,
}

impl Movie {
    /// Runtime of the first version. Derived, never stored.
    pub fn runtime_ms(&self) -> Option<u64> {
        runtime_ms(&self.versions)
    }
}

#[derive(Debug, Clone)]
pub struct Version {
    pub file_id: FileId,
    /// File name only (no directories): enough for "which file is this" in the UI
    /// without exposing the server's filesystem layout.
    pub file_name: String,
    pub size_bytes: u64,
    pub media: VersionMedia,
}

/// A probed file always has a playback decision; an unreadable one never does.
/// The type makes "decision without probe data" unrepresentable.
#[derive(Debug, Clone)]
pub enum VersionMedia {
    Probed {
        info: MediaInfo,
        playback: PlaybackDecision,
    },
    Unreadable {
        reason: String,
    },
}

/// Everything the HTTP layer needs to serve a direct-play stream.
#[derive(Debug, Clone)]
pub struct StreamSource {
    pub path: PathBuf,
    pub container: Container,
}

pub(crate) fn rev(conn: &Connection) -> rusqlite::Result<CatalogRev> {
    conn.prepare_cached("SELECT rev FROM catalog WHERE id = 1")?
        .query_row([], |r| r.get::<_, i64>(0))
        .map(|rev| CatalogRev(rev as u64))
}

/// `rev`, then `row_page(kind, None, HOME_ROW_LIMIT)` for Continue Watching
/// (left out when empty), Recently Added, and each library in the order
/// they were added.
pub(crate) fn home(conn: &Connection, user: UserId) -> rusqlite::Result<Home> {
    let rev = rev(conn)?;
    let libraries: Vec<(i64, String)> = conn
        .prepare_cached("SELECT id, name FROM library ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let kinds = [
        (RowKind::ContinueWatching, None),
        (RowKind::RecentlyAdded, None),
    ]
    .into_iter()
    .chain(
        libraries
            .into_iter()
            .map(|(id, name)| (RowKind::Library(LibraryId(id)), Some(name))),
    );
    let mut rows = Vec::new();
    for (kind, library_name) in kinds {
        let (total, cards, next) = row_page(conn, kind, None, HOME_ROW_LIMIT, user)?;
        if kind == RowKind::ContinueWatching && total == 0 {
            continue;
        }
        rows.push(Row {
            kind,
            total,
            cards,
            next,
            library_name,
            library_icon: None,
            title: None,
            query: None,
            library_next: None,
            spec: None,
        });
    }
    Ok(Home { rev, rows })
}

/// The rows of a custom home layout, in order. See [`crate::HomeRowSpec`].
pub(crate) fn home_with(
    conn: &Connection,
    user: UserId,
    specs: &[crate::HomeRowSpec],
) -> rusqlite::Result<Home> {
    use crate::HomeRowSpec;
    use rusqlite::OptionalExtension;
    let rev = rev(conn)?;
    let mut rows = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        match spec {
            HomeRowSpec::ContinueWatching { title } | HomeRowSpec::RecentlyAdded { title } => {
                let kind = if matches!(spec, HomeRowSpec::ContinueWatching { .. }) {
                    RowKind::ContinueWatching
                } else {
                    RowKind::RecentlyAdded
                };
                let (total, cards, next) = row_page(conn, kind, None, HOME_ROW_LIMIT, user)?;
                if kind == RowKind::ContinueWatching && total == 0 {
                    continue;
                }
                rows.push(Row {
                    kind,
                    total,
                    cards,
                    next,
                    library_name: None,
                    library_icon: None,
                    title: title.clone(),
                    query: None,
                    library_next: None,
                    spec: Some(index),
                });
            }
            HomeRowSpec::Library {
                library,
                query,
                title,
            } => {
                let Some((name, icon)) = conn
                    .prepare_cached("SELECT name, icon FROM library WHERE id = ?1")?
                    .query_row([library.0], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
                    })
                    .optional()?
                else {
                    continue;
                };
                let Some(page) =
                    crate::browse::page(conn, *library, query, None, HOME_ROW_LIMIT, user)?
                else {
                    continue;
                };
                rows.push(Row {
                    kind: RowKind::Library(*library),
                    total: page.total,
                    cards: page.cards,
                    next: None,
                    library_name: Some(name),
                    library_icon: icon,
                    title: title.clone(),
                    query: Some(query.clone()),
                    library_next: page.next,
                    spec: Some(index),
                });
            }
        }
    }
    Ok(Home { rev, rows })
}

/// Present movies, as `d`, with their poster as `p` and user `?1`'s watch
/// state as `w`.
const CARDS: &str = "SELECT d.id, d.kind, d.title, d.year, d.added_at, d.sort_title, \
    COALESCE(p.hash, cb.hash), COALESCE(p.width, cb.width), COALESCE(p.height, cb.height), COALESCE(p.placeholder, cb.placeholder), \
    w.file_id, w.position_ms, w.duration_ms, w.play_count, w.last_watched_at, w.updated_at \
    FROM item_display d LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' LEFT JOIN item_artwork cb ON cb.item_id = d.id AND cb.role = 'backdrop' \
    LEFT JOIN watch_state w ON w.user_id = ?1 AND w.item_id = d.id \
    WHERE d.library_id IN (SELECT id FROM library WHERE kind = 'movies') AND d.missing_since IS NULL";

const CARD_COUNT: &str = "SELECT count(*) FROM item_display d \
    WHERE d.library_id IN (SELECT id FROM library WHERE kind = 'movies') AND d.missing_since IS NULL";

/// Present movies and shows, as `id` and `at`: a movie's added time, a
/// show's newest episode's.
const RECENT: &str = "WITH recent(id, at) AS ( \
    SELECT id, added_at FROM item WHERE kind = 'movie' AND missing_since IS NULL \
    UNION ALL \
    SELECT se.parent_id, max(e.added_at) FROM item e JOIN item se ON se.id = e.parent_id \
    WHERE e.kind = 'episode' AND e.missing_since IS NULL GROUP BY se.parent_id) ";

#[cfg(test)]
pub(crate) const CARDS_PLAN_RECENT: &str = "SELECT d.id FROM item_display d LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' \
    WHERE d.library_id IN (SELECT id FROM library WHERE kind = 'movies') AND d.missing_since IS NULL \
    ORDER BY added_at DESC, id DESC LIMIT 41";
#[cfg(test)]
pub(crate) const CARDS_PLAN_TITLE: &str = "SELECT d.id FROM item_display d LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' \
    WHERE d.library_id IN (SELECT id FROM library WHERE kind = 'movies') AND d.missing_since IS NULL \
    ORDER BY sort_title COLLATE NOCASE, id LIMIT 41";

/// One page of `kind` after `after`, with the row's total. The extra row
/// fetched past `limit` decides `next`. Statements via `prepare_cached`.
pub(crate) fn row_page(
    conn: &Connection,
    kind: RowKind,
    after: Option<&Cursor>,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<(u32, Vec<Card>, Option<Cursor>)> {
    let wrong_cursor = || rusqlite::Error::InvalidParameterName("cursor is for another row".into());
    match (kind, after.map(|c| &c.0)) {
        (RowKind::ContinueWatching, None | Some(CursorKey::ContinueWatching { .. })) => {
            let all = shows::continue_watching(conn, user)?;
            let total = all.len() as u32;
            let start = match after.map(|c| &c.0) {
                Some(CursorKey::ContinueWatching { updated_at_ms, id }) => all
                    .iter()
                    .position(|(card, at)| (*at, card.id) < (*updated_at_ms, *id))
                    .unwrap_or(all.len()),
                _ => 0,
            };
            let mut page: Vec<(Card, i64)> = all
                .into_iter()
                .skip(start)
                .take(limit as usize + 1)
                .collect();
            let next = (page.len() > limit as usize).then(|| {
                page.truncate(limit as usize);
                let (card, at) = page.last().expect("limit >= 1");
                Cursor(CursorKey::ContinueWatching {
                    updated_at_ms: *at,
                    id: card.id,
                })
            });
            Ok((
                total,
                page.into_iter().map(|(card, _)| card).collect(),
                next,
            ))
        }
        (RowKind::Library(library), None | Some(CursorKey::Library(..))) => {
            let after = match after.map(|c| &c.0) {
                Some(CursorKey::Library(cursor_library, cursor)) if *cursor_library == library => {
                    Some(cursor)
                }
                Some(_) => return Err(wrong_cursor()),
                None => None,
            };
            let page = browse::page(conn, library, &LibraryQuery::default(), after, limit, user)?;
            Ok(page.map_or((0, Vec::new(), None), |page| {
                (
                    page.total,
                    page.cards,
                    page.next.map(|c| Cursor(CursorKey::Library(library, c))),
                )
            }))
        }
        (RowKind::RecentlyAdded, None | Some(CursorKey::RecentlyAdded { .. })) => {
            recently_added(conn, after.map(|c| &c.0), limit, user)
        }
        (RowKind::AllMovies, None | Some(CursorKey::AllMovies { .. })) => {
            all_movies(conn, after.map(|c| &c.0), limit, user)
        }
        _ => Err(wrong_cursor()),
    }
}

/// `CARDS ORDER BY sort_title COLLATE NOCASE, id` after the cursor, plus
/// `COUNT(*)` over the same predicate without it.
fn all_movies(
    conn: &Connection,
    after: Option<&CursorKey>,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<(u32, Vec<Card>, Option<Cursor>)> {
    let total = conn
        .prepare_cached(CARD_COUNT)?
        .query_row([], |r| r.get::<_, i64>(0))? as u32;
    let fetch = i64::from(limit) + 1;
    let order = "d.sort_title COLLATE NOCASE, d.id";
    let (sql, params): (String, Vec<rusqlite::types::Value>) = match after {
        Some(CursorKey::AllMovies { sort_title, id }) => (
            format!(
                "{CARDS} AND (d.sort_title COLLATE NOCASE > ?2 OR (d.sort_title COLLATE NOCASE = ?2 AND d.id > ?3)) \
                 ORDER BY {order} LIMIT ?4"
            ),
            vec![
                user.0.into(),
                sort_title.clone().into(),
                id.0.into(),
                fetch.into(),
            ],
        ),
        _ => (
            format!("{CARDS} ORDER BY {order} LIMIT ?2"),
            vec![user.0.into(), fetch.into()],
        ),
    };
    let mut cards = conn
        .prepare_cached(&sql)?
        .query_map(rusqlite::params_from_iter(params), |r| {
            let card = card(r)?;
            let key = CursorKey::AllMovies {
                sort_title: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                id: card.id,
            };
            Ok((card, key))
        })?
        .collect::<rusqlite::Result<Vec<(Card, CursorKey)>>>()?;
    let next = (cards.len() > limit as usize).then(|| {
        cards.truncate(limit as usize);
        Cursor(cards.last().expect("limit >= 1").1.clone())
    });
    Ok((
        total,
        cards.into_iter().map(|(card, _)| card).collect(),
        next,
    ))
}

/// [`RECENT`] joined to the cards' columns, `ORDER BY at DESC, id DESC`
/// after the cursor. Show cards get their counts from `shows`.
fn recently_added(
    conn: &Connection,
    after: Option<&CursorKey>,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<(u32, Vec<Card>, Option<Cursor>)> {
    let total = conn
        .prepare_cached(&format!("{RECENT}SELECT count(*) FROM recent"))?
        .query_row([], |r| r.get::<_, i64>(0))? as u32;
    let fetch = i64::from(limit) + 1;
    let select = format!(
        "{RECENT}SELECT d.id, d.kind, d.title, d.year, r.at, d.sort_title, \
         COALESCE(p.hash, cb.hash), COALESCE(p.width, cb.width), COALESCE(p.height, cb.height), COALESCE(p.placeholder, cb.placeholder), \
         w.file_id, w.position_ms, w.duration_ms, w.play_count, w.last_watched_at, w.updated_at \
         FROM recent r JOIN item_display d ON d.id = r.id \
         LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' LEFT JOIN item_artwork cb ON cb.item_id = d.id AND cb.role = 'backdrop' \
         LEFT JOIN watch_state w ON w.user_id = ?1 AND w.item_id = d.id"
    );
    let order = "ORDER BY r.at DESC, d.id DESC";
    let (sql, params): (String, Vec<rusqlite::types::Value>) = match after {
        Some(CursorKey::RecentlyAdded { added_at_ms, id }) => (
            format!("{select} WHERE r.at < ?2 OR (r.at = ?2 AND d.id < ?3) {order} LIMIT ?4"),
            vec![
                user.0.into(),
                (*added_at_ms).into(),
                id.0.into(),
                fetch.into(),
            ],
        ),
        _ => (
            format!("{select} {order} LIMIT ?2"),
            vec![user.0.into(), fetch.into()],
        ),
    };
    let mut cards = conn
        .prepare_cached(&sql)?
        .query_map(rusqlite::params_from_iter(params), |r| {
            let card = card(r)?;
            let key = CursorKey::RecentlyAdded {
                added_at_ms: r.get(4)?,
                id: card.id,
            };
            Ok((card, key))
        })?
        .collect::<rusqlite::Result<Vec<(Card, CursorKey)>>>()?;
    let next = (cards.len() > limit as usize).then(|| {
        cards.truncate(limit as usize);
        Cursor(cards.last().expect("limit >= 1").1.clone())
    });
    let mut out = Vec::with_capacity(cards.len());
    for (mut card, key) in cards {
        if card.kind == ItemKind::Show {
            let CursorKey::RecentlyAdded { added_at_ms, .. } = key else {
                unreachable!("keys of this row")
            };
            shows::fill_show_card(conn, &mut card, user)?;
            card.note = CardNote::NewEpisodes(shows::added_since(
                conn,
                card.id,
                added_at_ms - shows::NEW_EPISODE_WINDOW_MS,
            )?);
        }
        out.push(card);
    }
    Ok((total, out, next))
}

/// The card of one movie (or any item) by id, with `user`'s watch state.
pub(crate) fn card_by_id(
    conn: &Connection,
    id: ItemId,
    user: UserId,
) -> rusqlite::Result<Option<Card>> {
    use rusqlite::OptionalExtension;
    conn.prepare_cached(
        "SELECT d.id, d.kind, d.title, d.year, d.added_at, d.sort_title, \
         COALESCE(p.hash, cb.hash), COALESCE(p.width, cb.width), COALESCE(p.height, cb.height), COALESCE(p.placeholder, cb.placeholder), \
         w.file_id, w.position_ms, w.duration_ms, w.play_count, w.last_watched_at, w.updated_at \
         FROM item_display d LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' LEFT JOIN item_artwork cb ON cb.item_id = d.id AND cb.role = 'backdrop' \
         LEFT JOIN watch_state w ON w.user_id = ?1 AND w.item_id = d.id WHERE d.id = ?2",
    )?
    .query_row([user.0, id.0], card)
    .optional()
}

/// One row in [`CARDS`]' column order: `id, kind, title, year, (4), (5),
/// poster at 6..=9, watch state at 10..=14`.
pub(crate) fn card(r: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
    let mut card = Card::new(
        ItemId(r.get(0)?),
        item_kind(r.get_ref(1)?.as_str()?)?,
        r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        r.get(3)?,
    );
    card.poster = artwork(r, 6, ArtworkRole::Poster)?;
    card.watch = watch_state::state_columns(r, 10)?;
    Ok(card)
}

/// `hash, width, height, placeholder` starting at column `at`; all NULL when
/// the LEFT JOIN found no artwork.
pub(crate) fn artwork(
    r: &rusqlite::Row<'_>,
    at: usize,
    role: ArtworkRole,
) -> rusqlite::Result<Option<Artwork>> {
    let Some(hash) = r.get::<_, Option<Vec<u8>>>(at)? else {
        return Ok(None);
    };
    let id = ArtworkId(hash.try_into().map_err(|_| {
        rusqlite::Error::InvalidColumnType(at, "hash".into(), rusqlite::types::Type::Blob)
    })?);
    Ok(Some(Artwork {
        id,
        role,
        width: r.get(at + 1)?,
        height: r.get(at + 2)?,
        placeholder: Placeholder(r.get(at + 3)?),
    }))
}

pub(crate) fn item_kind(name: &str) -> rusqlite::Result<ItemKind> {
    match name {
        "movie" => Ok(ItemKind::Movie),
        "show" => Ok(ItemKind::Show),
        "season" => Ok(ItemKind::Season),
        "episode" => Ok(ItemKind::Episode),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "item kind {other:?}"
        ))),
    }
}

/// The item (+ poster/backdrop), then by kind: a movie's versions, or the
/// show, season or episode read from `shows`.
pub(crate) fn item(
    conn: &Connection,
    id: ItemId,
    device: &DeviceProfile,
    user: UserId,
) -> rusqlite::Result<Option<Item>> {
    use rusqlite::OptionalExtension;
    let head = conn
        .prepare_cached(
            "SELECT d.kind, d.title, d.year, d.overview, d.added_at, \
             p.hash, p.width, p.height, p.placeholder, b.hash, b.width, b.height, b.placeholder \
             FROM item_display d \
             LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' \
             LEFT JOIN item_artwork b ON b.item_id = d.id AND b.role = 'backdrop' \
             WHERE d.id = ?1",
        )?
        .query_row([id.0], |r| {
            Ok((
                item_kind(r.get_ref(0)?.as_str()?)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<u16>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                artwork(r, 5, ArtworkRole::Poster)?,
                artwork(r, 9, ArtworkRole::Backdrop)?,
            ))
        })
        .optional()?;
    let Some((kind, title, year, overview, added_at_ms, poster, backdrop)) = head else {
        return Ok(None);
    };
    match kind {
        ItemKind::Movie => {}
        ItemKind::Show => return Ok(Some(Item::Show(shows::show(conn, id, user)?))),
        ItemKind::Season => return Ok(shows::season(conn, id, user)?.map(Item::Season)),
        ItemKind::Episode => {
            return Ok(shows::episode(conn, id, device, user)?.map(Item::Episode));
        }
    }
    Ok(Some(Item::Movie(Movie {
        id,
        title,
        year,
        overview,
        added_at: SystemTime::UNIX_EPOCH + Duration::from_millis(added_at_ms.max(0) as u64),
        poster,
        backdrop,
        versions: versions(conn, id, device)?,
        watch: watch_state::load(conn, user, id)?,
        extras: crate::metadata::read::extras(conn, id)?,
    })))
}

/// Runtime of the first (best) version. Derived, never stored.
pub(crate) fn runtime_ms(versions: &[Version]) -> Option<u64> {
    versions.first().and_then(|v| match &v.media {
        VersionMedia::Probed { info, .. } => info.duration_ms,
        VersionMedia::Unreadable { .. } => None,
    })
}

/// Every file of `id`, best first. Two statements: its files, then all
/// their streams (ordered by file, index). Rows are parsed into `MediaInfo`
/// here (codec strings via `from_name`), then `device.decide()` runs per
/// version. The row -> domain mapping exists only in this file.
pub(crate) fn versions(
    conn: &Connection,
    id: ItemId,
    device: &DeviceProfile,
) -> rusqlite::Result<Vec<Version>> {
    struct FileRow {
        id: FileId,
        rel_path: String,
        size: u64,
        probe_error: Option<String>,
        container: Option<String>,
        duration_ms: Option<u64>,
        bitrate: Option<u64>,
    }
    let files = conn
        .prepare_cached(
            "SELECT id, rel_path, size, probe_error, container, duration_ms, bitrate \
             FROM media_file WHERE item_id = ?1 ORDER BY id",
        )?
        .query_map([id.0], |r| {
            Ok(FileRow {
                id: FileId(r.get(0)?),
                rel_path: r.get(1)?,
                size: r.get::<_, i64>(2)? as u64,
                probe_error: r.get(3)?,
                container: r.get(4)?,
                duration_ms: r.get::<_, Option<i64>>(5)?.map(|d| d as u64),
                bitrate: r.get::<_, Option<i64>>(6)?.map(|b| b as u64),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut streams: HashMap<FileId, MediaInfo> = files
        .iter()
        .map(|f| {
            let info = MediaInfo {
                container: f
                    .container
                    .as_deref()
                    .map(Container::from_name)
                    .unwrap_or(Container::Other),
                duration_ms: f.duration_ms,
                bitrate: f.bitrate,
                video: vec![],
                audio: vec![],
                subtitles: vec![],
            };
            (f.id, info)
        })
        .collect();
    let mut stmt = conn.prepare_cached(
        "SELECT s.file_id, s.stream_index, s.kind, s.codec, s.profile, s.level, s.bit_depth, s.width, s.height, \
         s.frame_rate, s.hdr, s.dv_profile, s.color_primaries, s.color_transfer, s.color_space, s.channels, \
         s.channel_layout, s.sample_rate, s.language, s.title, s.is_default, s.is_forced, s.dv_compatibility \
         FROM media_stream s JOIN media_file f ON f.id = s.file_id \
         WHERE f.item_id = ?1 ORDER BY s.file_id, s.stream_index",
    )?;
    let mut rows = stmt.query([id.0])?;
    while let Some(r) = rows.next()? {
        let file = FileId(r.get(0)?);
        if let Some(info) = streams.get_mut(&file) {
            stream_into(r, info)?;
        }
    }

    let mut versions: Vec<Version> = files
        .into_iter()
        .map(|f| {
            let info = streams.remove(&f.id).expect("inserted above");
            let media = match f.probe_error {
                Some(reason) => VersionMedia::Unreadable { reason },
                None => {
                    let playback = device.decide(&info);
                    VersionMedia::Probed { info, playback }
                }
            };
            Version {
                file_id: f.id,
                file_name: f
                    .rel_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&f.rel_path)
                    .to_owned(),
                size_bytes: f.size,
                media,
            }
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(version_rank(v)));
    Ok(versions)
}

/// Best first: playable before not, then height, then bitrate.
fn version_rank(v: &Version) -> (bool, u32, u64) {
    match &v.media {
        VersionMedia::Probed { info, playback } => (
            *playback == PlaybackDecision::DirectPlay,
            info.video.first().map_or(0, |s| s.height),
            info.bitrate.unwrap_or(0),
        ),
        VersionMedia::Unreadable { .. } => (false, 0, 0),
    }
}

/// One `media_stream` row into its file's `MediaInfo`.
fn stream_into(r: &rusqlite::Row<'_>, info: &mut MediaInfo) -> rusqlite::Result<()> {
    let index: u32 = r.get(1)?;
    let codec: String = r.get(3)?;
    let language: Option<String> = r.get(18)?;
    let title: Option<String> = r.get(19)?;
    let is_default: bool = r.get::<_, i64>(20)? != 0;
    match r.get_ref(2)?.as_str()? {
        "video" => info.video.push(VideoStream {
            index,
            codec: VideoCodec::from_name(&codec),
            profile: r.get(4)?,
            level: r.get(5)?,
            bit_depth: r.get(6)?,
            width: r.get::<_, Option<u32>>(7)?.unwrap_or(0),
            height: r.get::<_, Option<u32>>(8)?.unwrap_or(0),
            frame_rate: r.get(9)?,
            hdr: HdrFormat::from_name(
                r.get::<_, Option<String>>(10)?.as_deref().unwrap_or("sdr"),
                r.get(11)?,
                r.get(22)?,
            ),
            color: ColorInfo {
                primaries: r.get(12)?,
                transfer: r.get(13)?,
                space: r.get(14)?,
            },
            language,
            title,
            is_default,
        }),
        "audio" => info.audio.push(AudioStream {
            index,
            codec: AudioCodec::from_name(&codec),
            profile: r.get(4)?,
            channels: r.get::<_, Option<u8>>(15)?.unwrap_or(0),
            channel_layout: r.get(16)?,
            sample_rate: r.get(17)?,
            language,
            title,
            is_default,
        }),
        "subtitle" => info.subtitles.push(SubtitleStream {
            index,
            codec: SubtitleCodec::from_name(&codec),
            language,
            title,
            is_default,
            is_forced: r.get::<_, i64>(21)? != 0,
        }),
        _ => {}
    }
    Ok(())
}

/// `media_file JOIN library_root`: root.path joined with rel_path.
pub(crate) fn stream_source(
    conn: &Connection,
    id: FileId,
) -> rusqlite::Result<Option<StreamSource>> {
    use rusqlite::OptionalExtension;
    conn.prepare_cached(
        "SELECT r.path, f.rel_path, f.container FROM media_file f JOIN library_root r ON r.id = f.root_id WHERE f.id = ?1",
    )?
    .query_row([id.0], |r| {
        let root: String = r.get(0)?;
        let rel: String = r.get(1)?;
        let container: Option<String> = r.get(2)?;
        Ok(StreamSource {
            path: PathBuf::from(root).join(rel),
            container: container.as_deref().map(Container::from_name).unwrap_or(Container::Other),
        })
    })
    .optional()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_through_their_text_form() {
        let recent = Cursor(CursorKey::RecentlyAdded {
            added_at_ms: 1_760_000_000_000,
            id: ItemId(812),
        });
        let title = Cursor(CursorKey::AllMovies {
            sort_title: "Amélie".into(),
            id: ItemId(7),
        });
        let watching = Cursor(CursorKey::ContinueWatching {
            updated_at_ms: 1_760_000_000_001,
            id: ItemId(3),
        });
        assert_eq!(watching.to_string(), "c1760000000001.3");
        assert_eq!(
            Cursor::parse(RowKind::ContinueWatching, "c1760000000001.3").unwrap(),
            watching
        );
        assert_eq!(recent.to_string(), "r1760000000000.812");
        assert_eq!(title.to_string(), "t7.416dc3a96c6965");
        assert_eq!(
            Cursor::parse(RowKind::RecentlyAdded, "r1760000000000.812").unwrap(),
            recent
        );
        assert_eq!(
            Cursor::parse(RowKind::AllMovies, "t7.416dc3a96c6965").unwrap(),
            title
        );
    }

    #[test]
    fn untrusted_cursors_are_errors() {
        for (kind, raw) in [
            (RowKind::RecentlyAdded, ""),
            (RowKind::RecentlyAdded, "r"),
            (RowKind::RecentlyAdded, "r12"),
            (RowKind::RecentlyAdded, "rx.1"),
            (RowKind::RecentlyAdded, "t7.41"),
            (RowKind::AllMovies, "r1.2"),
            (RowKind::AllMovies, "t7.4"),
            (RowKind::AllMovies, "t7.zz"),
            (RowKind::AllMovies, "t7.ff"),
            (RowKind::AllMovies, "t.41"),
            (RowKind::AllMovies, "té.41"),
            (RowKind::AllMovies, "t1.é1"),
            (RowKind::ContinueWatching, "c"),
            (RowKind::ContinueWatching, "c12"),
            (RowKind::ContinueWatching, "r1.2"),
            (RowKind::RecentlyAdded, "c1.2"),
        ] {
            assert!(Cursor::parse(kind, raw).is_err(), "{kind:?} {raw:?}");
        }
    }

    use crate::db::Db;

    const NOBODY: UserId = UserId(0);
    use crate::test_support::seed_movies;
    use nuvora_media::DeviceProfile;

    async fn seeded(n: usize) -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 2).unwrap();
        db.write(move |tx| seed_movies(tx, n)).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn home_is_the_first_page_of_each_row() {
        let (_dir, db) = seeded(50).await;
        let home = db.read(|c| home(c, NOBODY)).await.unwrap();
        assert_eq!(home.rev.get(), 1);
        assert_eq!(
            home.rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [
                RowKind::RecentlyAdded,
                RowKind::Library(crate::LibraryId(1))
            ],
            "no Continue Watching without progress"
        );
        for row in &home.rows {
            assert_eq!((row.total, row.cards.len()), (50, 40), "{:?}", row.kind);
            assert!(row.next.is_some());
        }
        let recent = &home.rows[0].cards;
        assert_eq!(recent[0].title, "Movie 0049", "newest first");
        let titles: Vec<&str> = home.rows[1]
            .cards
            .iter()
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(
            titles[..6],
            [
                "A Movie 0000",
                "Movie 0001",
                "Movie 0002",
                "Movie 0003",
                "Movie 0004",
                "A Movie 0005"
            ],
            "ordered by sort title (`Movie 0000, A` < `Movie 0001`), not by title"
        );
        assert!(
            recent.iter().any(|c| c.poster.is_some()) && recent.iter().any(|c| c.poster.is_none())
        );
        let poster = recent.iter().find_map(|c| c.poster.as_ref()).unwrap();
        assert_eq!(
            (
                poster.role,
                poster.width,
                poster.height,
                poster.placeholder.0.len()
            ),
            (ArtworkRole::Poster, 640, 960, 2)
        );
    }

    #[tokio::test]
    async fn paging_through_a_row_neither_skips_nor_repeats() {
        let (_dir, db) = seeded(50).await;
        for kind in [
            RowKind::RecentlyAdded,
            RowKind::Library(crate::LibraryId(1)),
        ] {
            let mut seen = Vec::new();
            let mut after: Option<Cursor> = None;
            loop {
                let cursor = after.clone();
                let (total, cards, next) = db
                    .read(move |c| row_page(c, kind, cursor.as_ref(), 7, NOBODY))
                    .await
                    .unwrap();
                assert_eq!(total, 50);
                assert!(cards.len() <= 7);
                seen.extend(cards.iter().map(|c| c.id));
                let Some(next) = next else { break };
                after = Some(Cursor::parse(kind, &next.to_string()).unwrap());
            }
            let mut unique = seen.clone();
            unique.sort();
            unique.dedup();
            assert_eq!((seen.len(), unique.len()), (50, 50), "{kind:?}");
        }
    }

    #[tokio::test]
    async fn item_detail_lists_versions_best_first() {
        let (_dir, db) = seeded(3).await;
        let device = DeviceProfile::baseline();
        let found = db
            .read(move |c| item(c, ItemId(1), &device, NOBODY))
            .await
            .unwrap()
            .unwrap();
        let Item::Movie(movie) = found else {
            panic!("a movie")
        };
        assert_eq!(
            (movie.title.as_str(), movie.year),
            ("A Movie 0000", Some(2000))
        );
        assert_eq!(movie.versions.len(), 2);
        let names: Vec<&str> = movie
            .versions
            .iter()
            .map(|v| v.file_name.as_str())
            .collect();
        assert_eq!(
            names,
            ["A Movie 0000 (2000).mp4", "A Movie 0000 (2000) - 4K.mkv"],
            "playable 1080p mp4 before 4K mkv"
        );
        let VersionMedia::Probed { info, playback } = &movie.versions[0].media else {
            panic!("probed")
        };
        assert_eq!(playback, &PlaybackDecision::DirectPlay);
        assert_eq!(
            (info.video[0].width, info.audio[0].channels, info.container),
            (1920, 2, Container::Mp4)
        );
        assert_eq!(movie.runtime_ms(), Some(5_400_000));
        assert!(movie.backdrop.is_none() && movie.poster.is_some());

        let gone = db
            .read(move |c| item(c, ItemId(999), &DeviceProfile::baseline(), NOBODY))
            .await
            .unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn stream_source_joins_root_and_relative_path() {
        let (_dir, db) = seeded(1).await;
        let src = db
            .read(|c| stream_source(c, FileId(1)))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            src.path,
            PathBuf::from("/media/A Movie 0000 (2000)/A Movie 0000 (2000).mp4")
        );
        assert_eq!(src.container, Container::Mp4);
        assert!(
            db.read(|c| stream_source(c, FileId(999)))
                .await
                .unwrap()
                .is_none()
        );
    }
}
