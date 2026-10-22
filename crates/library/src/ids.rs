//! Typed ids. Integer ids are SQLite `AUTOINCREMENT` rowids: stable for the life
//! of the database, never reused after deletion.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub(crate) i64);

/// Content address of one rendered artwork: the first 16 bytes of
/// blake3(PIPELINE_VERSION || role || source bytes). Same source => same id, so
/// re-rendering is a no-op and URLs are immutable. Rendered as 32 hex chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtworkId(pub(crate) [u8; 16]);

#[derive(Debug, thiserror::Error)]
#[error("malformed id")]
pub struct ParseIdError;

impl ItemId {
    /// For the HTTP layer, which receives ids as path segments.
    pub fn from_raw(raw: i64) -> ItemId {
        ItemId(raw)
    }
    pub fn raw(self) -> i64 {
        self.0
    }
}

impl FileId {
    pub fn from_raw(raw: i64) -> FileId {
        FileId(raw)
    }
    pub fn raw(self) -> i64 {
        self.0
    }
}

impl fmt::Display for ArtworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl FromStr for ArtworkId {
    type Err = ParseIdError;
    /// Exactly 32 lowercase hex chars; anything else is rejected before it can
    /// reach a filesystem path.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let _ = s;
        todo!()
    }
}
