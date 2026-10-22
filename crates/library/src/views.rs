//! Read models: what each screen needs, assembled from SQL in one read
//! transaction. These are domain types (no serde); the server maps them to its
//! wire types. All SQL that reads the catalog lives in this file.

use std::path::PathBuf;
use std::time::SystemTime;

use nuvora_media::{ArtworkRole, Container, DeviceProfile, MediaInfo, Placeholder, PlaybackDecision};
use rusqlite::Connection;

use crate::{ArtworkId, FileId, ItemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Movie,
    // Step 2: Show, Season, Episode. No DB change needed (item.kind has no CHECK
    // list; `parent_id` and `index_number` already exist).
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

/// The home screen: an ordered list of rows, each bounded to its first page so
/// the payload does not grow with the library. The server decides which rows
/// exist and in what order, so Step 2's Continue Watching and Next Up appear on
/// every client without a client release.
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
/// Adding a row (Step 2's Continue Watching) is a variant here plus its arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Newest first: `added_at DESC, id DESC`.
    RecentlyAdded,
    /// By title: `sort_title COLLATE NOCASE, id`.
    AllMovies,
}

impl RowKind {
    /// Home order.
    pub const ALL: [RowKind; 2] = [RowKind::RecentlyAdded, RowKind::AllMovies];

    /// The keyset `ORDER BY` over `item_display`. The trailing id makes the
    /// order total, so a page boundary between equal keys never skips or repeats a card.
    pub(crate) const fn order_by(self) -> &'static str {
        match self {
            RowKind::RecentlyAdded => "added_at DESC, id DESC",
            RowKind::AllMovies => "sort_title COLLATE NOCASE, id",
        }
    }
}

/// An opaque keyset position inside one row: the sort key of the last card of
/// the previous page. Clients only pass it back. The encoding is private, so a
/// sort key can change without a client change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor(CursorKey);

#[derive(Debug, Clone, PartialEq, Eq)]
enum CursorKey {
    RecentlyAdded { added_at_ms: i64, id: ItemId },
    AllMovies { sort_title: String, id: ItemId },
}

#[derive(Debug, thiserror::Error)]
#[error("invalid cursor")]
pub struct InvalidCursor;

impl Cursor {
    /// Parse an untrusted string (a query parameter) as a cursor of `kind`'s row.
    /// Garbage, or a cursor minted by another row, is an error, never a panic.
    pub fn parse(kind: RowKind, raw: &str) -> Result<Cursor, InvalidCursor> {
        let key = match (kind, raw.split_at_checked(1)) {
            (RowKind::RecentlyAdded, Some(("r", rest))) => {
                let (at, id) = rest.split_once('.').ok_or(InvalidCursor)?;
                CursorKey::RecentlyAdded { added_at_ms: at.parse().map_err(|_| InvalidCursor)?, id: parse_id(id)? }
            }
            (RowKind::AllMovies, Some(("t", rest))) => {
                let (id, title) = rest.split_once('.').ok_or(InvalidCursor)?;
                CursorKey::AllMovies { sort_title: hex_decode_utf8(title)?, id: parse_id(id)? }
            }
            _ => return Err(InvalidCursor),
        };
        Ok(Cursor(key))
    }

    pub fn kind(&self) -> RowKind {
        match self.0 {
            CursorKey::RecentlyAdded { .. } => RowKind::RecentlyAdded,
            CursorKey::AllMovies { .. } => RowKind::AllMovies,
        }
    }
}

/// URL-safe without escaping: `r<added_at_ms>.<id>` or `t<id>.<hex sort_title>`.
impl std::fmt::Display for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            CursorKey::RecentlyAdded { added_at_ms, id } => write!(f, "r{added_at_ms}.{}", id.0),
            CursorKey::AllMovies { sort_title, id } => {
                write!(f, "t{}.", id.0)?;
                sort_title.bytes().try_for_each(|b| write!(f, "{b:02x}"))
            }
        }
    }
}

fn parse_id(s: &str) -> Result<ItemId, InvalidCursor> {
    s.parse().map(ItemId).map_err(|_| InvalidCursor)
}

fn hex_decode_utf8(s: &str) -> Result<String, InvalidCursor> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(InvalidCursor);
    }
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| InvalidCursor))
        .collect::<Result<Vec<u8>, _>>()?;
    String::from_utf8(bytes).map_err(|_| InvalidCursor)
}

/// What a poster card needs, and nothing more.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: ItemId,
    pub kind: ItemKind,
    pub title: String,
    pub year: Option<u16>,
    pub poster: Option<Artwork>,
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
}

impl Movie {
    /// Runtime of the first version. Derived, never stored.
    pub fn runtime_ms(&self) -> Option<u64> {
        self.versions.first().and_then(|v| match &v.media {
            VersionMedia::Probed { info, .. } => info.duration_ms,
            VersionMedia::Unreadable { .. } => None,
        })
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
    Probed { info: MediaInfo, playback: PlaybackDecision },
    Unreadable { reason: String },
}

/// Everything the HTTP layer needs to serve a direct-play stream.
#[derive(Debug, Clone)]
pub struct StreamSource {
    pub path: PathBuf,
    pub container: Container,
}

pub(crate) fn rev(conn: &Connection) -> rusqlite::Result<CatalogRev> {
    let _ = conn;
    todo!("SELECT rev FROM catalog")
}

/// `rev`, then `row_page(kind, None, HOME_ROW_LIMIT)` for each of [`RowKind::ALL`].
pub(crate) fn home(conn: &Connection) -> rusqlite::Result<Home> {
    let _ = conn;
    todo!()
}

/// `SELECT ... FROM item_display d LEFT JOIN item_artwork p ON (p.item_id, p.role) = (d.id, 'poster')
///  WHERE d.library_id IN (movie libraries) AND d.missing_since IS NULL
///    AND (<sort key>) > (<cursor key>)  ORDER BY kind.order_by() LIMIT limit + 1`
/// (the extra row decides `next`), plus `COUNT(*)` over the same predicate
/// without the cursor. Statements via `prepare_cached`.
pub(crate) fn row_page(
    conn: &Connection,
    kind: RowKind,
    after: Option<&Cursor>,
    limit: u32,
) -> rusqlite::Result<(u32, Vec<Card>, Option<Cursor>)> {
    let _ = (conn, kind, after, limit);
    todo!()
}

/// Three statements: the item (+ poster/backdrop), its files, all their streams
/// (`WHERE file_id IN (...)` ordered by file, index). Rows are parsed into
/// `MediaInfo` here (codec strings via `from_name`), then `device.decide()`
/// runs per version. The row -> domain mapping exists only in this file.
pub(crate) fn item(conn: &Connection, id: ItemId, device: &DeviceProfile) -> rusqlite::Result<Option<Item>> {
    let _ = (conn, id, device);
    todo!()
}

/// `media_file JOIN library_root`: root.path joined with rel_path.
pub(crate) fn stream_source(conn: &Connection, id: FileId) -> rusqlite::Result<Option<StreamSource>> {
    let _ = (conn, id);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_through_their_text_form() {
        let recent = Cursor(CursorKey::RecentlyAdded { added_at_ms: 1_760_000_000_000, id: ItemId(812) });
        let title = Cursor(CursorKey::AllMovies { sort_title: "Amélie".into(), id: ItemId(7) });
        assert_eq!(recent.to_string(), "r1760000000000.812");
        assert_eq!(title.to_string(), "t7.416dc3a96c6965");
        assert_eq!(Cursor::parse(RowKind::RecentlyAdded, "r1760000000000.812").unwrap(), recent);
        assert_eq!(Cursor::parse(RowKind::AllMovies, "t7.416dc3a96c6965").unwrap(), title);
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
        ] {
            assert!(Cursor::parse(kind, raw).is_err(), "{kind:?} {raw:?}");
        }
    }
}
