//! What the server needs to extract a file's subtitles and fonts: where the
//! file is, which subtitle streams it has, and its fingerprint, which keys
//! the extraction cache (a changed file gets a new fingerprint, so a stale
//! extraction is never served).

use std::fmt::Write;
use std::path::PathBuf;

use nuvora_media::{SubtitleCodec, SubtitleStream};
use rusqlite::{Connection, OptionalExtension};

use crate::{FileId, Library, QueryError};

#[derive(Debug, Clone)]
pub struct SubtitleSource {
    pub path: PathBuf,
    /// The file's 16-byte fingerprint as 32 hex chars.
    pub fingerprint: String,
    /// In stream order.
    pub subtitles: Vec<SubtitleStream>,
}

impl Library {
    pub async fn subtitle_source(&self, id: FileId) -> Result<Option<SubtitleSource>, QueryError> {
        Ok(self.inner.db.read(move |c| load(c, id)).await?)
    }
}

fn load(c: &Connection, id: FileId) -> rusqlite::Result<Option<SubtitleSource>> {
    let Some((path, fingerprint)) = c
        .prepare_cached(
            "SELECT r.path, f.rel_path, f.fingerprint FROM media_file f \
             JOIN library_root r ON r.id = f.root_id WHERE f.id = ?1",
        )?
        .query_row([id.0], |r| {
            let root: String = r.get(0)?;
            let rel: String = r.get(1)?;
            let fingerprint: Vec<u8> = r.get(2)?;
            Ok((PathBuf::from(root).join(rel), hex(&fingerprint)))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let subtitles = c
        .prepare_cached(
            "SELECT stream_index, codec, language, title, is_default, is_forced \
             FROM media_stream WHERE file_id = ?1 AND kind = 'subtitle' ORDER BY stream_index",
        )?
        .query_map([id.0], |r| {
            Ok(SubtitleStream {
                index: r.get(0)?,
                codec: SubtitleCodec::from_name(&r.get::<_, String>(1)?),
                language: r.get(2)?,
                title: r.get(3)?,
                is_default: r.get(4)?,
                is_forced: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(SubtitleSource {
        path,
        fingerprint,
        subtitles,
    }))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
