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

/// The home screen: an ordered list of rows. The server decides which rows
/// exist and in what order, so Step 2's Continue Watching and Next Up appear on
/// every client without a client release.
#[derive(Debug, Clone)]
pub struct Home {
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub kind: RowKind,
    pub title: String,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    RecentlyAdded,
    AllMovies,
}

/// What a poster card needs, and nothing more (a 2,000-card home must stay small).
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

/// `SELECT ... FROM item_display d LEFT JOIN item_artwork p ON (p.item_id, p.role) = (d.id, 'poster')
///  WHERE d.library_id IN (movie libraries) AND d.missing_since IS NULL`
/// ordered twice: by `added_at DESC LIMIT 24` (uses `item_recent`) and by
/// `sort_title COLLATE NOCASE`. Statements via `prepare_cached`.
pub(crate) fn home(conn: &Connection) -> rusqlite::Result<Home> {
    let _ = conn;
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
