//! The library page: one library's movies (or shows, in a TV library),
//! filtered for one user, sorted, keyset paged, and grouped into headers.
//!
//! A library's cards are read whole, then filtered and sorted here. That
//! keeps every sort (accent-folded titles, unknown years last, a show's
//! newest episode) in one comparator that the pages, the cursors and the
//! group index all share, so a page boundary or a group never disagrees
//! with the order. A cursor is the sort key of the last card of a page, not
//! an offset: cards added or removed meanwhile neither shift nor repeat the
//! rest.

use std::cmp::Ordering;
use std::collections::HashMap;

use nuvora_media::fold_for_sort;
use rusqlite::{Connection, OptionalExtension};

use crate::shows;
use crate::views::{self, Card, InvalidCursor, ItemKind};
use crate::{ItemId, LibraryId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Sort title, accents folded; `#` (digits, symbols, other scripts) first.
    #[default]
    Title,
    /// Unknown years last.
    Year,
    /// A movie's added time; a show's newest episode's.
    Added,
    /// The user's last finish; never watched last.
    Watched,
    /// A movie's longest version; a show's episodes summed. Unknown last.
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    /// Ascending for titles, descending (newest, longest, latest) otherwise.
    pub fn default_for(sort: Sort) -> Order {
        match sort {
            Sort::Title => Order::Asc,
            _ => Order::Desc,
        }
    }
}

/// For the requesting user. A show is watched when every episode is, in
/// progress when some are watched or started, unwatched when none is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatchFilter {
    #[default]
    All,
    Unwatched,
    InProgress,
    Watched,
}

/// A movie's resolution class, by its video stream: wide or tall enough for
/// the class (so a 1920x800 scope encode is 1080p).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    UltraHd,
    FullHd,
    Hd,
    Sd,
}

impl Resolution {
    fn of(width: u32, height: u32) -> Resolution {
        if width >= 3200 || height >= 2000 {
            Resolution::UltraHd
        } else if width >= 1700 || height >= 1000 {
            Resolution::FullHd
        } else if width >= 1200 || height >= 700 {
            Resolution::Hd
        } else {
            Resolution::Sd
        }
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// What the library page asks for. Resolution and HDR apply to movies only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryQuery {
    pub sort: Sort,
    pub order: Order,
    pub filter: WatchFilter,
    pub resolution: Option<Resolution>,
    pub hdr: bool,
}

impl Default for LibraryQuery {
    /// By title, A to Z, everything.
    fn default() -> Self {
        LibraryQuery {
            sort: Sort::Title,
            order: Order::Asc,
            filter: WatchFilter::All,
            resolution: None,
            hdr: false,
        }
    }
}

/// One page of the library grid.
#[derive(Debug, Clone)]
pub struct LibraryPage {
    pub rev: crate::CatalogRev,
    /// Cards matching the query, across every page.
    pub total: u32,
    pub cards: Vec<Card>,
    pub next: Option<LibraryCursor>,
}

/// A group header of the library grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexGroup {
    pub key: GroupKey,
    pub count: u32,
    /// The cursor whose page starts with this group; `None` for the first.
    pub after: Option<LibraryCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKey {
    /// `'#'` or `'A'..='Z'`.
    Letter(char),
    /// The decade's first year: 1990 for the 1990s.
    Decade(u16),
    Month {
        year: i32,
        /// 1 to 12.
        month: u32,
    },
    /// No year (year sort), or no added time (never, in practice).
    Unknown,
}

/// The sort position of one card. `missing` sorts last in either order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    missing: bool,
    num: i64,
    /// Accent-folded, lowercased sort title.
    title: String,
    id: ItemId,
}

/// An opaque keyset position on the library page: the key of the last card
/// of the previous page, for one sort and order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryCursor {
    sort: Sort,
    order: Order,
    key: Key,
}

impl LibraryCursor {
    /// Parses an untrusted cursor. It must have been minted for `query`'s
    /// sort and order; the filters may differ (a cursor is a position).
    pub fn parse(query: &LibraryQuery, raw: &str) -> Result<LibraryCursor, InvalidCursor> {
        let rest = raw.strip_prefix('l').ok_or(InvalidCursor)?;
        let mut chars = rest.chars();
        let sort = match chars.next() {
            Some('t') => Sort::Title,
            Some('y') => Sort::Year,
            Some('a') => Sort::Added,
            Some('w') => Sort::Watched,
            Some('r') => Sort::Runtime,
            _ => return Err(InvalidCursor),
        };
        let order = match chars.next() {
            Some('a') => Order::Asc,
            Some('d') => Order::Desc,
            _ => return Err(InvalidCursor),
        };
        let missing = match chars.next() {
            Some('0') => false,
            Some('1') => true,
            _ => return Err(InvalidCursor),
        };
        if (sort, order) != (query.sort, query.order) {
            return Err(InvalidCursor);
        }
        let mut parts = chars.as_str().splitn(3, '.');
        let num = parts
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or(InvalidCursor)?;
        let id = views::parse_id(parts.next().ok_or(InvalidCursor)?)?;
        let title = views::hex_decode_utf8(parts.next().ok_or(InvalidCursor)?)?;
        Ok(LibraryCursor {
            sort,
            order,
            key: Key {
                missing,
                num,
                title,
                id,
            },
        })
    }
}

/// URL-safe: `l<sort><order><missing><num>.<id>.<hex title>`.
impl std::fmt::Display for LibraryCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sort = match self.sort {
            Sort::Title => 't',
            Sort::Year => 'y',
            Sort::Added => 'a',
            Sort::Watched => 'w',
            Sort::Runtime => 'r',
        };
        let order = match self.order {
            Order::Asc => 'a',
            Order::Desc => 'd',
        };
        let key = &self.key;
        write!(
            f,
            "l{sort}{order}{}{}.{}.",
            u8::from(key.missing),
            key.num,
            key.id.0
        )?;
        key.title.bytes().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Unwatched,
    InProgress,
    Watched,
}

/// One card of the library with everything its sorts and filters read.
struct Entry {
    card: Card,
    folded_title: String,
    added_at: i64,
    runtime_ms: Option<u64>,
    status: Status,
    /// [`Resolution::bit`]s of the movie's versions.
    resolutions: u8,
    hdr: bool,
}

impl Entry {
    fn matches(&self, query: &LibraryQuery) -> bool {
        let status = match query.filter {
            WatchFilter::All => true,
            WatchFilter::Unwatched => self.status == Status::Unwatched,
            WatchFilter::InProgress => self.status == Status::InProgress,
            WatchFilter::Watched => self.status == Status::Watched,
        };
        let movie = self.card.kind == ItemKind::Movie;
        let resolution = query
            .resolution
            .is_none_or(|r| !movie || self.resolutions & r.bit() != 0);
        status && resolution && (!query.hdr || !movie || self.hdr)
    }

    fn key(&self, sort: Sort) -> Key {
        let (missing, num) = match sort {
            Sort::Title => (false, i64::from(self.letter() != '#')),
            Sort::Year => (
                self.card.year.is_none(),
                self.card.year.map_or(0, i64::from),
            ),
            Sort::Added => (false, self.added_at),
            Sort::Watched => (
                self.card.watch.last_watched_at.is_none(),
                self.card.watch.last_watched_at.unwrap_or(0),
            ),
            Sort::Runtime => (
                self.runtime_ms.is_none(),
                self.runtime_ms.map_or(0, |r| r as i64),
            ),
        };
        Key {
            missing,
            num,
            title: self.folded_title.clone(),
            id: self.card.id,
        }
    }

    /// `'A'..='Z'` from the folded title, else `'#'`.
    fn letter(&self) -> char {
        match self.folded_title.chars().next() {
            Some(c) if c.is_ascii_lowercase() => c.to_ascii_uppercase(),
            _ => '#',
        }
    }

    fn group(&self, sort: Sort) -> Option<GroupKey> {
        match sort {
            Sort::Title => Some(GroupKey::Letter(self.letter())),
            Sort::Year => Some(
                self.card
                    .year
                    .map_or(GroupKey::Unknown, |y| GroupKey::Decade(y / 10 * 10)),
            ),
            Sort::Added => {
                let (year, month) = year_month(self.added_at);
                Some(GroupKey::Month { year, month })
            }
            Sort::Watched | Sort::Runtime => None,
        }
    }
}

/// The page order: known before unknown in either direction, then the sort
/// value, then the title (A to Z unless sorting by title), then the id.
fn compare(a: &Key, b: &Key, sort: Sort, order: Order) -> Ordering {
    let dir = |o: Ordering| match order {
        Order::Asc => o,
        Order::Desc => o.reverse(),
    };
    a.missing
        .cmp(&b.missing)
        .then_with(|| dir(a.num.cmp(&b.num)))
        .then_with(|| match sort {
            Sort::Title => dir(a.title.cmp(&b.title)),
            _ => a.title.cmp(&b.title),
        })
        .then_with(|| dir(a.id.cmp(&b.id)))
}

/// The library's present movies or shows for `user`; `None` when the
/// library does not exist.
fn entries(
    conn: &Connection,
    library: LibraryId,
    user: UserId,
) -> rusqlite::Result<Option<Vec<Entry>>> {
    let kind: Option<String> = conn
        .prepare_cached("SELECT kind FROM library WHERE id = ?1")?
        .query_row([library.0], |r| r.get(0))
        .optional()?;
    let Some(kind) = kind else {
        return Ok(None);
    };
    let item_kind = if kind == "shows" { "show" } else { "movie" };
    let rows: Vec<(Card, Option<String>, i64, Option<i64>)> = conn
        .prepare_cached(
            "SELECT d.id, d.kind, d.title, d.year, d.added_at, d.sort_title, \
             COALESCE(p.hash, cb.hash), COALESCE(p.width, cb.width), COALESCE(p.height, cb.height), COALESCE(p.placeholder, cb.placeholder), \
             w.file_id, w.position_ms, w.duration_ms, w.play_count, w.last_watched_at, w.updated_at, \
             (SELECT max(f.duration_ms) FROM media_file f WHERE f.item_id = d.id) \
             FROM item_display d LEFT JOIN item_artwork p ON p.item_id = d.id AND p.role = 'poster' LEFT JOIN item_artwork cb ON cb.item_id = d.id AND cb.role = 'backdrop' \
             LEFT JOIN watch_state w ON w.user_id = ?1 AND w.item_id = d.id \
             WHERE d.library_id = ?2 AND d.kind = ?3 AND d.missing_since IS NULL",
        )?
        .query_map(rusqlite::params![user.0, library.0, item_kind], |r| {
            Ok((views::card(r)?, r.get(5)?, r.get(4)?, r.get(16)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut video: HashMap<ItemId, (u8, bool)> = HashMap::new();
    let mut stats = HashMap::new();
    if item_kind == "movie" {
        let mut stmt = conn.prepare_cached(
            "SELECT f.item_id, s.width, s.height, s.hdr FROM media_stream s \
             JOIN media_file f ON f.id = s.file_id JOIN item i ON i.id = f.item_id \
             WHERE i.library_id = ?1 AND s.kind = 'video'",
        )?;
        let mut streams = stmt.query([library.0])?;
        while let Some(r) = streams.next()? {
            let entry = video.entry(ItemId(r.get(0)?)).or_default();
            let (width, height): (Option<u32>, Option<u32>) = (r.get(1)?, r.get(2)?);
            entry.0 |= Resolution::of(width.unwrap_or(0), height.unwrap_or(0)).bit();
            entry.1 |= r.get::<_, Option<String>>(3)?.is_some_and(|h| h != "sdr");
        }
    } else {
        stats = shows::show_stats(conn, "e.library_id = ?2", library.0, user)?;
    }

    let entries = rows
        .into_iter()
        .map(|(mut card, sort_title, added_at, runtime)| {
            let folded_title = fold_for_sort(sort_title.as_deref().unwrap_or(&card.title));
            let (resolutions, hdr) = video.get(&card.id).copied().unwrap_or_default();
            let (status, added_at, runtime_ms) = match card.kind {
                ItemKind::Show => {
                    let s = stats.get(&card.id).copied().unwrap_or_default();
                    card.watch = s.watch();
                    card.unwatched = Some(s.episodes - s.watched);
                    let status = if s.episodes > 0 && s.watched == s.episodes {
                        Status::Watched
                    } else if s.watched > 0 || s.in_progress > 0 {
                        Status::InProgress
                    } else {
                        Status::Unwatched
                    };
                    let runtime = (s.runtime_ms > 0).then_some(s.runtime_ms);
                    (status, s.newest_added_at.max(added_at), runtime)
                }
                _ => {
                    let status = if card.watch.resume.is_some() {
                        Status::InProgress
                    } else if card.watch.watched() {
                        Status::Watched
                    } else {
                        Status::Unwatched
                    };
                    (status, added_at, runtime.map(|r| r.max(0) as u64))
                }
            };
            Entry {
                card,
                folded_title,
                added_at,
                runtime_ms,
                status,
                resolutions,
                hdr,
            }
        })
        .collect();
    Ok(Some(entries))
}

/// The library's entries matching `query`, in page order, with their keys.
fn ordered(
    conn: &Connection,
    library: LibraryId,
    query: &LibraryQuery,
    user: UserId,
) -> rusqlite::Result<Option<Vec<(Entry, Key)>>> {
    let Some(entries) = entries(conn, library, user)? else {
        return Ok(None);
    };
    let mut keyed: Vec<(Entry, Key)> = entries
        .into_iter()
        .filter(|e| e.matches(query))
        .map(|e| {
            let key = e.key(query.sort);
            (e, key)
        })
        .collect();
    keyed.sort_by(|(_, a), (_, b)| compare(a, b, query.sort, query.order));
    Ok(Some(keyed))
}

pub(crate) struct Page {
    pub total: u32,
    pub cards: Vec<Card>,
    pub next: Option<LibraryCursor>,
}

/// The cards after `after` (from the start when `None`), at most `limit`.
/// `None` when the library does not exist.
pub(crate) fn page(
    conn: &Connection,
    library: LibraryId,
    query: &LibraryQuery,
    after: Option<&LibraryCursor>,
    limit: u32,
    user: UserId,
) -> rusqlite::Result<Option<Page>> {
    let Some(keyed) = ordered(conn, library, query, user)? else {
        return Ok(None);
    };
    let total = keyed.len() as u32;
    let start = after.map_or(0, |cursor| {
        keyed
            .iter()
            .position(|(_, key)| {
                compare(key, &cursor.key, query.sort, query.order) == Ordering::Greater
            })
            .unwrap_or(keyed.len())
    });
    let limit = limit.max(1) as usize;
    let mut page: Vec<(Entry, Key)> = keyed.into_iter().skip(start).take(limit + 1).collect();
    let next = (page.len() > limit).then(|| {
        page.truncate(limit);
        LibraryCursor {
            sort: query.sort,
            order: query.order,
            key: page.last().expect("limit >= 1").1.clone(),
        }
    });
    Ok(Some(Page {
        total,
        cards: page.into_iter().map(|(entry, _)| entry.card).collect(),
        next,
    }))
}

/// The group headers for `query`, in page order: letters for the title
/// sort, decades for year, months for added, none for the other sorts.
/// `None` when the library does not exist.
pub(crate) fn index(
    conn: &Connection,
    library: LibraryId,
    query: &LibraryQuery,
    user: UserId,
) -> rusqlite::Result<Option<Vec<IndexGroup>>> {
    let Some(keyed) = ordered(conn, library, query, user)? else {
        return Ok(None);
    };
    let mut groups: Vec<IndexGroup> = Vec::new();
    let mut previous: Option<&Key> = None;
    for (entry, key) in &keyed {
        let Some(group) = entry.group(query.sort) else {
            return Ok(Some(Vec::new()));
        };
        match groups.last_mut() {
            Some(last) if last.key == group => last.count += 1,
            _ => groups.push(IndexGroup {
                key: group,
                count: 1,
                after: previous.map(|key| LibraryCursor {
                    sort: query.sort,
                    order: query.order,
                    key: key.clone(),
                }),
            }),
        }
        previous = Some(key);
    }
    Ok(Some(groups))
}

/// The UTC calendar month of a unix-ms time.
fn year_month(ms: i64) -> (i32, u32) {
    // Howard Hinnant's civil_from_days.
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as i32, month as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use rusqlite::params;

    const ADA: UserId = UserId(1);
    const JAN: i64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z
    const MONTH: i64 = 31 * 24 * 3600 * 1000;

    /// (title, sort title, year, added_at, width, height, hdr, runtime).
    type MovieRow = (
        &'static str,
        &'static str,
        Option<u16>,
        i64,
        u32,
        u32,
        &'static str,
        Option<i64>,
    );

    const MOVIES: &[MovieRow] = &[
        (
            "Alien",
            "Alien",
            Some(1979),
            JAN,
            1920,
            1080,
            "sdr",
            Some(7_000),
        ),
        (
            "Amélie",
            "Amélie",
            Some(2001),
            JAN + MONTH,
            3840,
            2160,
            "hdr10",
            Some(7_200),
        ),
        (
            "The Matrix",
            "Matrix, The",
            Some(1999),
            JAN + MONTH,
            1280,
            720,
            "sdr",
            Some(8_000),
        ),
        (
            "Ñandú",
            "Ñandú",
            None,
            JAN + 2 * MONTH,
            640,
            480,
            "sdr",
            None,
        ),
        (
            "1917",
            "1917",
            Some(2019),
            JAN + 2 * MONTH,
            3840,
            1600,
            "sdr",
            Some(6_600),
        ),
        (
            "東京物語",
            "東京物語",
            Some(1953),
            JAN + 2 * MONTH,
            1920,
            800,
            "sdr",
            Some(8_100),
        ),
        (
            "Zodiac",
            "Zodiac",
            Some(2007),
            JAN,
            1920,
            1080,
            "dolby_vision",
            Some(9_000),
        ),
    ];

    /// The movies above plus 40 `Movie NN` fillers (years and added times
    /// repeating, so every sort has ties), and ada's watch state: Alien and
    /// The Matrix watched, Zodiac in progress.
    async fn seeded() -> (tempfile::TempDir, Db, LibraryId) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 2).unwrap();
        db.write(|tx| {
            tx.execute(
                "INSERT INTO library (key, name, kind) VALUES ('movies', 'Movies', 'movies')",
                [],
            )?;
            tx.execute("INSERT INTO library_root (library_id, path) VALUES (1, '/m')", [])?;
            tx.execute(
                "INSERT INTO user (username, display_name, password_hash, role, created_at) \
                 VALUES ('ada', 'Ada', 'x', 'admin', 0)",
                [],
            )?;
            let fillers: Vec<MovieRow> = (0..40)
                .map(|i| {
                    let title: &'static str = Box::leak(format!("Movie {i:02}").into_boxed_str());
                    let year = (i % 3 != 0).then_some(1990 + (i % 4) as u16);
                    (title, title, year, JAN + (i % 5) * 1000, 1920, 1080, "sdr", Some(5_000 + i % 2))
                })
                .collect();
            for (n, (title, sort, year, added, w, h, hdr, runtime)) in
                MOVIES.iter().chain(&fillers).enumerate()
            {
                let id = n as i64 + 1;
                tx.execute(
                    "INSERT INTO item (id, library_id, kind, identity_key, added_at) VALUES (?1, 1, 'movie', ?2, ?3)",
                    params![id, format!("k{id}"), added],
                )?;
                tx.execute(
                    "INSERT INTO item_metadata (item_id, source, title, sort_title, year, updated_at) \
                     VALUES (?1, 'filename', ?2, ?3, ?4, 0)",
                    params![id, title, sort, year],
                )?;
                tx.execute(
                    "INSERT INTO media_file (id, item_id, root_id, rel_path, size, mtime_ns, fingerprint, added_at, \
                     probe_version, duration_ms) VALUES (?1, ?1, 1, ?2, 1, 0, zeroblob(16), ?3, 1, ?4)",
                    params![id, format!("{id}.mkv"), added, runtime],
                )?;
                tx.execute(
                    "INSERT INTO media_stream (file_id, stream_index, kind, codec, width, height, hdr) \
                     VALUES (?1, 0, 'video', 'hevc', ?2, ?3, ?4)",
                    params![id, w, h, hdr],
                )?;
            }
            for (item, position, plays, last) in [
                (1, None, 1, Some(100)),
                (3, None, 2, Some(200)),
                (7, Some(60_000), 0, None),
            ] {
                tx.execute(
                    "INSERT INTO watch_state (user_id, item_id, file_id, position_ms, duration_ms, play_count, \
                     last_watched_at, updated_at) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, 1)",
                    params![
                        item,
                        position.map(|_| item),
                        position,
                        position.map(|_| 100_000),
                        plays,
                        last
                    ],
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
        (dir, db, LibraryId(1))
    }

    async fn titles(db: &Db, query: LibraryQuery) -> Vec<String> {
        db.read(move |c| page(c, LibraryId(1), &query, None, 200, ADA))
            .await
            .unwrap()
            .unwrap()
            .cards
            .into_iter()
            .map(|c| c.title)
            .collect()
    }

    fn query(sort: Sort, order: Order, filter: WatchFilter) -> LibraryQuery {
        LibraryQuery {
            sort,
            order,
            filter,
            resolution: None,
            hdr: false,
        }
    }

    #[tokio::test]
    async fn sorts_fold_accents_and_put_unknowns_last() {
        let (_dir, db, _) = seeded().await;
        let by_title = titles(&db, LibraryQuery::default()).await;
        assert_eq!(
            by_title[..5],
            ["1917", "東京物語", "Alien", "Amélie", "The Matrix"],
            "# first, then A; Amélie beside Alien; The Matrix under M"
        );
        assert_eq!(by_title[by_title.len() - 2..], ["Ñandú", "Zodiac"]);
        let by_year = titles(&db, query(Sort::Year, Order::Desc, WatchFilter::All)).await;
        assert_eq!(by_year[..3], ["1917", "Zodiac", "Amélie"]);
        assert_eq!(
            by_year.last().map(String::as_str),
            Some("Ñandú"),
            "no year sorts last, descending too"
        );
        let by_year_asc = titles(&db, query(Sort::Year, Order::Asc, WatchFilter::All)).await;
        assert_eq!(by_year_asc[..2], ["東京物語", "Alien"]);
        let watched = titles(&db, query(Sort::Watched, Order::Desc, WatchFilter::Watched)).await;
        assert_eq!(watched, ["The Matrix", "Alien"]);
        let runtime = titles(&db, query(Sort::Runtime, Order::Desc, WatchFilter::All)).await;
        assert_eq!(runtime[..2], ["Zodiac", "東京物語"]);
    }

    #[tokio::test]
    async fn filters_match_watch_state_resolution_and_hdr() {
        let (_dir, db, _) = seeded().await;
        let all = LibraryQuery::default();
        let with = |filter, resolution, hdr| LibraryQuery {
            filter,
            resolution,
            hdr,
            ..all.clone()
        };
        assert_eq!(
            titles(&db, with(WatchFilter::InProgress, None, false)).await,
            ["Zodiac"]
        );
        assert_eq!(
            titles(&db, with(WatchFilter::Watched, None, false)).await,
            ["Alien", "The Matrix"]
        );
        assert_eq!(
            titles(&db, with(WatchFilter::Unwatched, None, false))
                .await
                .len(),
            47 - 3
        );
        assert_eq!(
            titles(
                &db,
                with(WatchFilter::All, Some(Resolution::UltraHd), false)
            )
            .await,
            ["1917", "Amélie"],
            "3840x1600 scope is 4K"
        );
        assert_eq!(
            titles(&db, with(WatchFilter::All, Some(Resolution::Hd), false)).await,
            ["The Matrix"]
        );
        assert_eq!(
            titles(&db, with(WatchFilter::All, Some(Resolution::Sd), false)).await,
            ["Ñandú"]
        );
        assert_eq!(
            titles(&db, with(WatchFilter::All, None, true)).await,
            ["Amélie", "Zodiac"]
        );
        let ben = db
            .read(|c| {
                page(
                    c,
                    LibraryId(1),
                    &LibraryQuery::default(),
                    None,
                    200,
                    UserId(2),
                )
            })
            .await
            .unwrap()
            .unwrap();
        assert!(
            ben.cards.iter().all(|c| c.watch == Default::default()),
            "another user sees none of ada's state"
        );
    }

    #[tokio::test]
    async fn paging_every_sort_and_filter_neither_skips_nor_repeats() {
        let (_dir, db, library) = seeded().await;
        let sorts = [
            Sort::Title,
            Sort::Year,
            Sort::Added,
            Sort::Watched,
            Sort::Runtime,
        ];
        let filters = [
            WatchFilter::All,
            WatchFilter::Unwatched,
            WatchFilter::InProgress,
            WatchFilter::Watched,
        ];
        for sort in sorts {
            for order in [Order::Asc, Order::Desc] {
                for filter in filters {
                    let q = query(sort, order, filter);
                    let whole: Vec<ItemId> = {
                        let q = q.clone();
                        db.read(move |c| page(c, library, &q, None, 200, ADA))
                            .await
                            .unwrap()
                            .unwrap()
                            .cards
                            .iter()
                            .map(|c| c.id)
                            .collect()
                    };
                    let mut paged = Vec::new();
                    let mut after: Option<String> = None;
                    loop {
                        let cursor = after
                            .as_deref()
                            .map(|raw| LibraryCursor::parse(&q, raw).unwrap());
                        let q2 = q.clone();
                        let p = db
                            .read(move |c| page(c, library, &q2, cursor.as_ref(), 4, ADA))
                            .await
                            .unwrap()
                            .unwrap();
                        assert_eq!(p.total as usize, whole.len());
                        paged.extend(p.cards.iter().map(|c| c.id));
                        let Some(next) = p.next else { break };
                        after = Some(next.to_string());
                    }
                    assert_eq!(paged, whole, "{sort:?} {order:?} {filter:?}");
                }
            }
        }
    }

    #[tokio::test]
    async fn the_index_counts_groups_and_points_at_their_first_card() {
        let (_dir, db, library) = seeded().await;
        let index_of = |q: LibraryQuery| {
            let db = db.clone();
            async move {
                db.read(move |c| index(c, library, &q, ADA))
                    .await
                    .unwrap()
                    .unwrap()
            }
        };
        let letters = index_of(LibraryQuery::default()).await;
        let counts: Vec<(GroupKey, u32)> = letters.iter().map(|g| (g.key, g.count)).collect();
        assert_eq!(
            counts,
            [
                (GroupKey::Letter('#'), 2),
                (GroupKey::Letter('A'), 2),
                (GroupKey::Letter('M'), 41),
                (GroupKey::Letter('N'), 1),
                (GroupKey::Letter('Z'), 1),
            ]
        );
        assert!(letters[0].after.is_none());
        for group in &letters {
            let first = {
                let after = group.after.clone();
                db.read(move |c| page(c, library, &LibraryQuery::default(), after.as_ref(), 1, ADA))
                    .await
                    .unwrap()
                    .unwrap()
                    .cards[0]
                    .title
                    .clone()
            };
            let expected = match group.key {
                GroupKey::Letter('#') => "1917",
                GroupKey::Letter('A') => "Alien",
                GroupKey::Letter('M') => "The Matrix",
                GroupKey::Letter('N') => "Ñandú",
                _ => "Zodiac",
            };
            assert_eq!(first, expected, "{:?}", group.key);
        }

        let decades = index_of(query(Sort::Year, Order::Desc, WatchFilter::All)).await;
        let counts: Vec<(GroupKey, u32)> = decades.iter().map(|g| (g.key, g.count)).collect();
        assert_eq!(
            counts,
            [
                (GroupKey::Decade(2010), 1),
                (GroupKey::Decade(2000), 2),
                (GroupKey::Decade(1990), 27),
                (GroupKey::Decade(1970), 1),
                (GroupKey::Decade(1950), 1),
                (GroupKey::Unknown, 15),
            ]
        );
        let months = index_of(query(Sort::Added, Order::Asc, WatchFilter::Unwatched)).await;
        let counts: Vec<(GroupKey, u32)> = months.iter().map(|g| (g.key, g.count)).collect();
        assert_eq!(
            counts,
            [
                (
                    GroupKey::Month {
                        year: 2026,
                        month: 1
                    },
                    40
                ),
                (
                    GroupKey::Month {
                        year: 2026,
                        month: 2
                    },
                    1
                ),
                (
                    GroupKey::Month {
                        year: 2026,
                        month: 3
                    },
                    3
                ),
            ],
            "filtered: Alien, The Matrix and Zodiac are left out"
        );
        assert!(
            index_of(query(Sort::Runtime, Order::Desc, WatchFilter::All))
                .await
                .is_empty()
        );
    }

    #[test]
    fn cursors_round_trip_and_reject_another_sort() {
        let q = query(Sort::Year, Order::Desc, WatchFilter::All);
        let cursor = LibraryCursor {
            sort: Sort::Year,
            order: Order::Desc,
            key: Key {
                missing: true,
                num: -5,
                title: "amélie".into(),
                id: ItemId(9),
            },
        };
        let text = cursor.to_string();
        assert_eq!(text, "lyd1-5.9.616dc3a96c6965");
        assert_eq!(LibraryCursor::parse(&q, &text).unwrap(), cursor);
        assert!(LibraryCursor::parse(&LibraryQuery::default(), &text).is_err());
        for raw in [
            "",
            "l",
            "lyd",
            "lyd2-5.9.61",
            "lyd1x.9.61",
            "lyd1-5.x.61",
            "lyd1-5.9.6",
            "t7.41",
        ] {
            assert!(LibraryCursor::parse(&q, raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn months_are_utc_calendar_months() {
        assert_eq!(year_month(JAN), (2026, 1));
        assert_eq!(year_month(JAN - 1), (2025, 12));
        assert_eq!(year_month(0), (1970, 1));
        assert_eq!(year_month(951_782_400_000), (2000, 2), "2000-02-29");
    }
}
