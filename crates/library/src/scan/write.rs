//! Every catalog write the scan makes, one function per transaction. Each runs
//! inside `Db::write`, so each is atomic; each is safe to repeat. Each one that
//! changes what clients see calls [`bump_rev`] in the same transaction.

use std::collections::HashSet;
use std::path::Path;

use nuvora_media::{
    ArtworkRole, HdrFormat, MediaInfo, MovieName, PROBE_VERSION, ProbeError, RenderedArtwork,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::plan::{
    FileForArtwork, ItemFiles, KnownFile, Move, ProbeJob, StoredArtwork, WantedSource,
};
use super::walk::{RelPath, RootId};
use crate::{ArtworkId, FileId, ItemId, LibrarySpec};

pub(crate) struct RootRow {
    pub id: RootId,
    pub library_id: i64,
    pub path: std::path::PathBuf,
}

/// Add the configured libraries and roots that do not exist yet. Libraries
/// match by `key`; a root is skipped when any library already has its path.
/// Nothing is updated or removed: the admin panel owns a library once it exists.
pub(crate) fn add_libraries(tx: &Transaction<'_>, specs: &[LibrarySpec]) -> rusqlite::Result<()> {
    for spec in specs {
        tx.execute(
            "INSERT INTO library (key, name, kind) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO NOTHING",
            params![spec.key, spec.name, "movies"],
        )?;
        let library_id: i64 =
            tx.query_row("SELECT id FROM library WHERE key = ?1", [&spec.key], |r| {
                r.get(0)
            })?;
        for root in &spec.roots {
            tx.execute(
                "INSERT INTO library_root (library_id, path) SELECT ?1, ?2 \
                 WHERE NOT EXISTS (SELECT 1 FROM library_root WHERE path = ?2)",
                params![library_id, root.to_string_lossy()],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn load_roots(conn: &Connection) -> rusqlite::Result<Vec<RootRow>> {
    conn.prepare_cached("SELECT id, library_id, path FROM library_root ORDER BY id")?
        .query_map([], |r| {
            Ok(RootRow {
                id: RootId(r.get(0)?),
                library_id: r.get(1)?,
                path: r.get::<_, String>(2)?.into(),
            })
        })?
        .collect()
}

/// One SELECT over `media_file` for the given roots.
pub(crate) fn load_known_files(
    conn: &Connection,
    roots: &[RootId],
) -> rusqlite::Result<Vec<KnownFile>> {
    let rows = conn
        .prepare_cached("SELECT id, root_id, rel_path, size, mtime_ns, fingerprint, probe_version FROM media_file")?
        .query_map([], |r| {
            Ok(KnownFile {
                id: FileId(r.get(0)?),
                root: RootId(r.get(1)?),
                rel: RelPath(r.get(2)?),
                size: r.get::<_, i64>(3)? as u64,
                mtime_ns: r.get(4)?,
                fingerprint: blob16(r.get(5)?),
                probe_version: r.get::<_, i64>(6)? as u32,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter(|k| roots.contains(&k.root))
        .collect())
}

fn blob16(blob: Vec<u8>) -> [u8; 16] {
    let mut out = [0u8; 16];
    let n = blob.len().min(16);
    out[..n].copy_from_slice(&blob[..n]);
    out
}

/// Insert or update one probed file and everything derived from it:
/// 1. find-or-create the item by `(library_id, name.identity_key())`, clearing
///    `missing_since` (a returning file revives its old item and id);
/// 2. upsert the `media_file` row (by `(root, rel_path)`, so a crash-and-retry
///    of the same insert updates instead of duplicating);
/// 3. replace its `media_stream` rows (or set `probe_error` on failure);
/// 4. rewrite the item's `filename` metadata layer from the item's *oldest*
///    file, so which version was probed last never changes the title.
///
/// If the file moved to a different item (its name now parses differently),
/// the old item is marked missing when it has no files left.
pub(crate) fn upsert_file(
    tx: &Transaction<'_>,
    job: &ProbeJob,
    fingerprint: [u8; 16],
    name: &MovieName,
    probe: &Result<MediaInfo, ProbeError>,
    now_ms: i64,
) -> rusqlite::Result<FileId> {
    let item_id = find_or_create_item(tx, job.library_id, name.identity_key().as_str(), now_ms)?;
    let existing: Option<(i64, i64)> = tx
        .query_row(
            "SELECT id, item_id FROM media_file WHERE root_id = ?1 AND rel_path = ?2",
            params![job.root.0, job.disk.rel.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (container, duration_ms, bitrate, probe_error) = match probe {
        Ok(info) => (
            Some(info.container.as_str()),
            info.duration_ms,
            info.bitrate,
            None,
        ),
        Err(e) => (None, None, None, Some(e.to_string())),
    };
    let file_id = match existing {
        Some((id, _)) => {
            tx.execute(
                "UPDATE media_file SET item_id = ?1, size = ?2, mtime_ns = ?3, fingerprint = ?4, probe_version = ?5, \
                 probe_error = ?6, container = ?7, duration_ms = ?8, bitrate = ?9 WHERE id = ?10",
                params![
                    item_id,
                    job.disk.size as i64,
                    job.disk.mtime_ns,
                    fingerprint.to_vec(),
                    PROBE_VERSION,
                    probe_error,
                    container,
                    duration_ms.map(|d| d as i64),
                    bitrate.map(|b| b as i64),
                    id
                ],
            )?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO media_file (item_id, root_id, rel_path, size, mtime_ns, fingerprint, added_at, probe_version, \
                 probe_error, container, duration_ms, bitrate) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    item_id,
                    job.root.0,
                    job.disk.rel.0,
                    job.disk.size as i64,
                    job.disk.mtime_ns,
                    fingerprint.to_vec(),
                    now_ms,
                    PROBE_VERSION,
                    probe_error,
                    container,
                    duration_ms.map(|d| d as i64),
                    bitrate.map(|b| b as i64),
                ],
            )?;
            tx.last_insert_rowid()
        }
    };
    tx.execute("DELETE FROM media_stream WHERE file_id = ?1", [file_id])?;
    if let Ok(info) = probe {
        insert_streams(tx, file_id, info)?;
    }
    write_filename_layer(tx, item_id, now_ms)?;
    if let Some((_, old_item)) = existing
        && old_item != item_id
    {
        write_filename_layer(tx, old_item, now_ms)?;
        mark_item_missing_if_empty(tx, old_item, now_ms)?;
    }
    bump_rev(tx)?;
    Ok(FileId(file_id))
}

fn insert_streams(tx: &Transaction<'_>, file_id: i64, info: &MediaInfo) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO media_stream (file_id, stream_index, kind, codec, profile, level, bit_depth, width, height, \
         frame_rate, hdr, dv_profile, color_primaries, color_transfer, color_space, channels, channel_layout, \
         sample_rate, language, title, is_default, is_forced) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
    )?;
    for v in &info.video {
        let dv_profile = match v.hdr {
            HdrFormat::DolbyVision { profile, .. } => Some(profile),
            _ => None,
        };
        stmt.execute(params![
            file_id,
            v.index,
            "video",
            v.codec.as_str(),
            v.profile,
            v.level,
            v.bit_depth,
            v.width,
            v.height,
            v.frame_rate,
            v.hdr.as_str(),
            dv_profile,
            v.color.primaries,
            v.color.transfer,
            v.color.space,
            Option::<u8>::None,
            Option::<String>::None,
            Option::<u32>::None,
            v.language,
            v.title,
            v.is_default,
            false,
        ])?;
    }
    for a in &info.audio {
        stmt.execute(params![
            file_id,
            a.index,
            "audio",
            a.codec.as_str(),
            a.profile,
            Option::<i32>::None,
            Option::<u8>::None,
            Option::<u32>::None,
            Option::<u32>::None,
            Option::<f64>::None,
            Option::<String>::None,
            Option::<u8>::None,
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
            a.channels,
            a.channel_layout,
            a.sample_rate,
            a.language,
            a.title,
            a.is_default,
            false,
        ])?;
    }
    for s in &info.subtitles {
        stmt.execute(params![
            file_id,
            s.index,
            "subtitle",
            s.codec.as_str(),
            Option::<String>::None,
            Option::<i32>::None,
            Option::<u8>::None,
            Option::<u32>::None,
            Option::<u32>::None,
            Option::<f64>::None,
            Option::<String>::None,
            Option::<u8>::None,
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
            Option::<u8>::None,
            Option::<String>::None,
            Option::<u32>::None,
            s.language,
            s.title,
            s.is_default,
            s.is_forced,
        ])?;
    }
    Ok(())
}

/// The item owning `(library, identity_key)`, revived if it was missing, or a
/// new one.
fn find_or_create_item(
    tx: &Transaction<'_>,
    library_id: i64,
    key: &str,
    now_ms: i64,
) -> rusqlite::Result<i64> {
    let found: Option<(i64, Option<i64>)> = tx
        .query_row(
            "SELECT id, missing_since FROM item WHERE library_id = ?1 AND identity_key = ?2",
            params![library_id, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match found {
        Some((id, missing_since)) => {
            if missing_since.is_some() {
                tx.execute("UPDATE item SET missing_since = NULL WHERE id = ?1", [id])?;
            }
            Ok(id)
        }
        None => {
            tx.execute(
                "INSERT INTO item (library_id, kind, identity_key, added_at) VALUES (?1, 'movie', ?2, ?3)",
                params![library_id, key, now_ms],
            )?;
            Ok(tx.last_insert_rowid())
        }
    }
}

/// The `filename` metadata layer of `item`, parsed from its oldest file.
/// Written only when it differs, so an unchanged title is not a write.
fn write_filename_layer(tx: &Transaction<'_>, item: i64, now_ms: i64) -> rusqlite::Result<()> {
    let oldest: Option<String> = tx
        .query_row(
            "SELECT rel_path FROM media_file WHERE item_id = ?1 ORDER BY added_at, id LIMIT 1",
            [item],
            |r| r.get(0),
        )
        .optional()?;
    let Some(rel) = oldest else {
        return Ok(());
    };
    let name = MovieName::parse(Path::new(&rel));
    tx.execute(
        "INSERT INTO item_metadata (item_id, source, title, sort_title, year, updated_at) \
         VALUES (?1, 'filename', ?2, ?3, ?4, ?5) \
         ON CONFLICT(item_id, source) DO UPDATE SET title = excluded.title, sort_title = excluded.sort_title, \
         year = excluded.year, updated_at = excluded.updated_at \
         WHERE title IS NOT excluded.title OR sort_title IS NOT excluded.sort_title OR year IS NOT excluded.year",
        params![item, name.title, name.sort_title, name.year, now_ms],
    )?;
    Ok(())
}

fn mark_item_missing_if_empty(
    tx: &Transaction<'_>,
    item: i64,
    now_ms: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE item SET missing_since = ?1 WHERE id = ?2 AND missing_since IS NULL \
         AND NOT EXISTS (SELECT 1 FROM media_file WHERE item_id = ?2)",
        params![now_ms, item],
    )?;
    Ok(())
}

pub(crate) fn mark_empty_items_missing(
    tx: &Transaction<'_>,
    now_ms: i64,
) -> rusqlite::Result<usize> {
    tx.execute(
        "UPDATE item SET missing_since = ?1 WHERE missing_since IS NULL \
         AND NOT EXISTS (SELECT 1 FROM media_file f WHERE f.item_id = item.id)",
        [now_ms],
    )
}

/// Apply a rename/move: update `root_id`, `rel_path`, `mtime_ns`; keep the id,
/// streams and artwork. Then re-derive the item from the new name:
/// - same identity key => nothing else changes;
/// - new key owned by another item => re-parent the file (becomes a version
///   there); the old item is marked missing if now empty;
/// - new key unowned and the file is its item's only file => re-key the item in
///   place and rewrite its filename layer (the user fixed a typo; the id survives);
/// - otherwise => create the new item and re-parent.
pub(crate) fn move_file(
    tx: &Transaction<'_>,
    mv: &Move,
    name: &MovieName,
    now_ms: i64,
) -> rusqlite::Result<()> {
    let old_item: i64 = tx.query_row(
        "SELECT item_id FROM media_file WHERE id = ?1",
        [mv.id.0],
        |r| r.get(0),
    )?;
    tx.execute(
        "UPDATE media_file SET root_id = ?1, rel_path = ?2, mtime_ns = ?3 WHERE id = ?4",
        params![mv.to_root.0, mv.to.rel.0, mv.to.mtime_ns, mv.id.0],
    )?;
    let (old_library, old_key): (i64, String) = tx.query_row(
        "SELECT library_id, identity_key FROM item WHERE id = ?1",
        [old_item],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let key = name.identity_key();
    if old_library == mv.to_library_id && old_key == key.as_str() {
        return bump_rev(tx);
    }
    let owner: Option<i64> = tx
        .query_row(
            "SELECT id FROM item WHERE library_id = ?1 AND identity_key = ?2",
            params![mv.to_library_id, key.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    let files_in_old: i64 = tx.query_row(
        "SELECT count(*) FROM media_file WHERE item_id = ?1",
        [old_item],
        |r| r.get(0),
    )?;
    let target = match owner {
        Some(id) => {
            tx.execute(
                "UPDATE item SET missing_since = NULL WHERE id = ?1 AND missing_since IS NOT NULL",
                [id],
            )?;
            id
        }
        None if files_in_old == 1 => {
            tx.execute(
                "UPDATE item SET library_id = ?1, identity_key = ?2 WHERE id = ?3",
                params![mv.to_library_id, key.as_str(), old_item],
            )?;
            old_item
        }
        None => {
            tx.execute(
                "INSERT INTO item (library_id, kind, identity_key, added_at) VALUES (?1, 'movie', ?2, ?3)",
                params![mv.to_library_id, key.as_str(), now_ms],
            )?;
            tx.last_insert_rowid()
        }
    };
    if target != old_item {
        tx.execute(
            "UPDATE media_file SET item_id = ?1 WHERE id = ?2",
            params![target, mv.id.0],
        )?;
        write_filename_layer(tx, old_item, now_ms)?;
        mark_item_missing_if_empty(tx, old_item, now_ms)?;
    }
    write_filename_layer(tx, target, now_ms)?;
    bump_rev(tx)
}

/// Stored artwork state for planning: every `artwork` row's identity and sig.
pub(crate) fn load_artwork_sigs(conn: &Connection) -> rusqlite::Result<Vec<StoredArtwork>> {
    let rows = conn
        .prepare_cached("SELECT item_id, role, source, source_sig FROM artwork")?
        .query_map([], |r| {
            Ok((
                ItemId(r.get(0)?),
                r.get::<_, String>(1)?,
                r.get(2)?,
                r.get(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<(ItemId, String, String, String)>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(item, role, source, sig)| {
            Some(StoredArtwork {
                item,
                role: ArtworkRole::from_name(&role)?,
                source,
                sig,
            })
        })
        .collect())
}

/// Present items with their files, for artwork planning. A file is SDR when it
/// probed and its primary video stream is not HDR.
pub(crate) fn load_present_items(conn: &Connection) -> rusqlite::Result<Vec<ItemFiles>> {
    let mut stmt = conn.prepare_cached(
        "SELECT i.id, f.id, f.root_id, f.rel_path, f.fingerprint, f.duration_ms, f.probe_error, \
         (SELECT s.hdr FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'video' ORDER BY s.stream_index LIMIT 1), \
         (SELECT s.height FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'video' ORDER BY s.stream_index LIMIT 1) \
         FROM item i JOIN media_file f ON f.item_id = i.id \
         WHERE i.missing_since IS NULL ORDER BY i.id, f.id",
    )?;
    let mut items: Vec<ItemFiles> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let item = ItemId(r.get(0)?);
        let probe_error: Option<String> = r.get(6)?;
        let hdr: Option<String> = r.get(7)?;
        let file = FileForArtwork {
            id: FileId(r.get(1)?),
            root: RootId(r.get(2)?),
            rel: RelPath(r.get(3)?),
            fingerprint: blob16(r.get(4)?),
            duration_ms: r.get::<_, Option<i64>>(5)?.map(|d| d as u64),
            sdr: probe_error.is_none() && hdr.as_deref().is_none_or(|h| h == "sdr"),
            height: r.get::<_, Option<u32>>(8)?.unwrap_or(0),
        };
        match items.last_mut() {
            Some(last) if last.item == item => last.files.push(file),
            _ => items.push(ItemFiles {
                item,
                files: vec![file],
            }),
        }
    }
    Ok(items)
}

/// Upsert one `artwork` row. Called only after the store holds `id`'s files.
/// A poster's [`LocalImage`](super::plan::LocalImage) is passed as `WantedSource::Local`.
pub(crate) fn set_artwork(
    tx: &Transaction<'_>,
    item: ItemId,
    role: ArtworkRole,
    source: &WantedSource,
    id: ArtworkId,
    rendered: &RenderedArtwork,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO artwork (item_id, role, source, source_sig, hash, width, height, placeholder) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(item_id, role, source) DO UPDATE SET source_sig = excluded.source_sig, hash = excluded.hash, \
         width = excluded.width, height = excluded.height, placeholder = excluded.placeholder",
        params![
            item.0,
            role.as_str(),
            source.name(),
            source.sig(),
            id.0.to_vec(),
            rendered.width,
            rendered.height,
            rendered.placeholder.0
        ],
    )?;
    bump_rev(tx)
}

/// Delete gone files (streams cascade), mark items with no remaining files
/// missing (`missing_since = now` where NULL), and delete artwork rows whose
/// source is gone. Idempotent: deleting a deleted id is a no-op, and a call
/// that changes nothing does not bump the rev.
pub(crate) fn remove_gone(
    tx: &Transaction<'_>,
    gone: &[FileId],
    stale_artwork: &[(ItemId, ArtworkRole, &'static str)],
    now_ms: i64,
) -> rusqlite::Result<()> {
    let mut changes = 0;
    for id in gone {
        changes += tx.execute("DELETE FROM media_file WHERE id = ?1", [id.0])?;
    }
    if changes > 0 {
        changes += mark_empty_items_missing(tx, now_ms)?;
    }
    for (item, role, source) in stale_artwork {
        changes += tx.execute(
            "DELETE FROM artwork WHERE item_id = ?1 AND role = ?2 AND source = ?3",
            params![item.0, role.as_str(), source],
        )?;
    }
    if changes > 0 {
        bump_rev(tx)?;
    }
    Ok(())
}

pub(crate) fn live_artwork_ids(conn: &Connection) -> rusqlite::Result<HashSet<ArtworkId>> {
    conn.prepare_cached("SELECT DISTINCT hash FROM artwork")?
        .query_map([], |r| Ok(ArtworkId(blob16(r.get(0)?))))?
        .collect()
}

/// `UPDATE catalog SET rev = rev + 1`. Home and row ETags are this number, so a
/// write that changes visible rows without bumping it would serve stale 304s.
pub(crate) fn bump_rev(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("UPDATE catalog SET rev = rev + 1 WHERE id = 1", [])?;
    Ok(())
}
