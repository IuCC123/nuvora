//! Typed ids. Integer ids are SQLite `AUTOINCREMENT` rowids: stable for the life
//! of the database, never reused after deletion.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LibraryId(pub(crate) i64);

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

impl UserId {
    pub fn from_raw(raw: i64) -> UserId {
        UserId(raw)
    }
    pub fn raw(self) -> i64 {
        self.0
    }
}

impl LibraryId {
    pub fn from_raw(raw: i64) -> LibraryId {
        LibraryId(raw)
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
        if s.len() != 32 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(ParseIdError);
        }
        let mut id = [0u8; 16];
        for (i, byte) in id.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|_| ParseIdError)?;
        }
        Ok(ArtworkId(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_ids_round_trip_as_lowercase_hex() {
        let id = ArtworkId([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 255]);
        let text = id.to_string();
        assert_eq!(text, "000102030405060708090a0b0c0d0e0f".replace("0f", "ff"));
        assert_eq!(text.parse::<ArtworkId>().unwrap(), id);
    }

    #[test]
    fn malformed_artwork_ids_are_rejected() {
        for bad in [
            "",
            "00",
            "ZZ0102030405060708090a0b0c0d0e0f",
            "000102030405060708090A0B0C0D0E0F",
            "../../etc/passwd/0000",
        ] {
            assert!(bad.parse::<ArtworkId>().is_err(), "{bad:?}");
        }
    }
}
