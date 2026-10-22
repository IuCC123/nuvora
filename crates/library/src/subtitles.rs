//! What the server needs to extract a file's subtitles and fonts: where the
//! file is, which subtitle streams it has, and its fingerprint, which keys
//! the extraction cache (a changed file gets a new fingerprint, so a stale
//! extraction is never served).

use std::fmt::Write;
use std::path::PathBuf;

use nuvora_media::{SidecarName, SubtitleCodec, SubtitleStream};
use rusqlite::{Connection, OptionalExtension};

use crate::{FileId, Library, QueryError};

#[derive(Debug, Clone)]
pub struct SubtitleSource {
    pub path: PathBuf,
    /// The file's 16-byte fingerprint as 32 hex chars.
    pub fingerprint: String,
    /// In stream order.
    pub subtitles: Vec<SubtitleStream>,
    /// Subtitle files next to it, in name order.
    pub sidecars: Vec<Sidecar>,
}

/// The track index of a video's first subtitle file; the others follow in
/// name order. Stream indexes in a file stay far below it.
pub const SIDECAR_INDEX_BASE: u32 = 1000;

/// A subtitle file next to a video, found by the scan.
#[derive(Debug, Clone)]
pub struct Sidecar {
    /// [`SIDECAR_INDEX_BASE`] plus its place in name order.
    pub index: u32,
    pub codec: SubtitleCodec,
    pub name: SidecarName,
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ns: i64,
}

/// `file`'s subtitle files, in name order.
pub(crate) fn sidecars(c: &Connection, file: FileId) -> rusqlite::Result<Vec<Sidecar>> {
    let rows = c
        .prepare_cached(
            "SELECT r.path, s.rel_path, s.format, s.language, s.title, s.is_forced, s.is_sdh, s.is_default, \
             s.size, s.mtime_ns FROM media_sidecar s JOIN media_file f ON f.id = s.file_id \
             JOIN library_root r ON r.id = f.root_id WHERE s.file_id = ?1 ORDER BY s.rel_path",
        )?
        .query_map([file.0], |r| {
            Ok((
                PathBuf::from(r.get::<_, String>(0)?).join(r.get::<_, String>(1)?),
                r.get::<_, String>(2)?,
                SidecarName {
                    language: r.get(3)?,
                    title: r.get(4)?,
                    forced: r.get(5)?,
                    hearing_impaired: r.get(6)?,
                    default: r.get(7)?,
                },
                r.get::<_, i64>(8)? as u64,
                r.get::<_, i64>(9)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(i, (path, format, name, size, mtime_ns))| Sidecar {
            index: SIDECAR_INDEX_BASE + i as u32,
            codec: match format.as_str() {
                "ass" => SubtitleCodec::Ass,
                "vtt" => SubtitleCodec::WebVtt,
                _ => SubtitleCodec::Srt,
            },
            name,
            path,
            size,
            mtime_ns,
        })
        .collect())
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
        sidecars: sidecars(c, id)?,
    }))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
