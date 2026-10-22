//! Every catalog write the scan makes, one function per transaction. Each runs
//! inside `Db::write`, so each is atomic; each is safe to repeat. Each one that
//! changes what clients see calls [`bump_rev`] in the same transaction.

use std::collections::HashSet;

use nuvora_media::{
    ArtworkRole, HdrFormat, MediaInfo, PARSE_VERSION, PROBE_VERSION, ProbeError, RenderedArtwork,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::place::{self, Placement};
use super::plan::{
    ArtKind, FileForArtwork, ItemFiles, KnownFile, Move, ProbeJob, StoredArtwork, WantedSource,
};
use super::walk::{RelPath, RootId};
use crate::{ArtworkId, FileId, ItemId, LibraryKind, LibrarySpec};

pub(crate) struct RootRow {
    pub id: RootId,
    pub library_id: i64,
    pub kind: LibraryKind,
    pub path: std::path::PathBuf,
}

/// Add the configured libraries and roots that do not exist yet. Libraries
/// match by `key`; a root is skipped when any library already has its path,
/// and a new library is created only when it brings a root. Nothing is
/// updated or removed: the admin panel owns a library once it exists.
pub(crate) fn add_libraries(tx: &Transaction<'_>, specs: &[LibrarySpec]) -> rusqlite::Result<()> {
    for spec in specs {
        let mut new_roots = Vec::new();
        for root in &spec.roots {
            let path = root.to_string_lossy();
            let used = tx
                .query_row(
                    "SELECT 1 FROM library_root WHERE path = ?1",
                    [&path],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !used {
                new_roots.push(path);
            }
        }
        if new_roots.is_empty() {
            continue;
        }
        tx.execute(
            "INSERT INTO library (key, name, kind) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO NOTHING",
            params![spec.key, spec.name, spec.kind.as_str()],
        )?;
        let library_id: i64 =
            tx.query_row("SELECT id FROM library WHERE key = ?1", [&spec.key], |r| {
                r.get(0)
            })?;
        for path in new_roots {
            tx.execute(
                "INSERT INTO library_root (library_id, path) VALUES (?1, ?2)",
                params![library_id, path],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn load_roots(conn: &Connection) -> rusqlite::Result<Vec<RootRow>> {
    conn.prepare_cached(
        "SELECT r.id, r.library_id, r.path, l.kind FROM library_root r JOIN library l ON l.id = r.library_id \
         ORDER BY r.id",
    )?
    .query_map([], |r| {
        let kind: String = r.get(3)?;
        Ok(RootRow {
            id: RootId(r.get(0)?),
            library_id: r.get(1)?,
            kind: LibraryKind::from_name(&kind).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    format!("unknown library kind {kind:?}").into(),
                )
            })?,
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
        .prepare_cached(
            "SELECT id, root_id, rel_path, size, mtime_ns, fingerprint, probe_version, parse_version FROM media_file",
        )?
        .query_map([], |r| {
            Ok(KnownFile {
                id: FileId(r.get(0)?),
                root: RootId(r.get(1)?),
                rel: RelPath(r.get(2)?),
                size: r.get::<_, i64>(3)? as u64,
                mtime_ns: r.get(4)?,
                fingerprint: blob16(r.get(5)?),
                probe_version: r.get::<_, i64>(6)? as u32,
                parse_version: r.get::<_, i64>(7)? as u32,
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
/// 1. place it: find-or-create its movie, or its show, season and episode,
///    by the name's identity keys, clearing `missing_since` (a returning file
///    revives its old items and ids);
/// 2. upsert the `media_file` row (by `(root, rel_path)`, so a crash-and-retry
///    of the same insert updates instead of duplicating);
/// 3. replace its `media_stream` rows (or set `probe_error` on failure, with
///    a stale `probe_version` when the failure is transient);
/// 4. rewrite the `filename` metadata layers from the *oldest* files, so
///    which version was probed last never changes a title.
///
/// If the file moved to a different item (its name now parses differently),
/// the old item is marked missing when it has no files left. `None`, with
/// nothing written, when the name does not fit the library's type (the
/// library's type changed during the scan).
pub(crate) fn upsert_file(
    tx: &Transaction<'_>,
    job: &ProbeJob,
    fingerprint: [u8; 16],
    probe: &Result<MediaInfo, ProbeError>,
    now_ms: i64,
) -> rusqlite::Result<Option<FileId>> {
    let kind = place::library_kind(tx, job.library_id)?;
    let Some(placement) = Placement::of(kind, &job.disk.rel.0) else {
        return Ok(None);
    };
    let item_id = place::place(tx, job.library_id, &placement, now_ms)?;
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
    // A stale version makes the next scan probe the file again.
    let probe_version = match probe {
        Err(e) if e.is_transient() => 0,
        _ => PROBE_VERSION,
    };
    let file_id = match existing {
        Some((id, _)) => {
            tx.execute(
                "UPDATE media_file SET item_id = ?1, size = ?2, mtime_ns = ?3, fingerprint = ?4, probe_version = ?5, \
                 probe_error = ?6, container = ?7, duration_ms = ?8, bitrate = ?9, parse_version = ?11 WHERE id = ?10",
                params![
                    item_id,
                    job.disk.size as i64,
                    job.disk.mtime_ns,
                    fingerprint.to_vec(),
                    probe_version,
                    probe_error,
                    container,
                    duration_ms.map(|d| d as i64),
                    bitrate.map(|b| b as i64),
                    id,
                    PARSE_VERSION,
                ],
            )?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO media_file (item_id, root_id, rel_path, size, mtime_ns, fingerprint, added_at, probe_version, \
                 probe_error, container, duration_ms, bitrate, parse_version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    item_id,
                    job.root.0,
                    job.disk.rel.0,
                    job.disk.size as i64,
                    job.disk.mtime_ns,
                    fingerprint.to_vec(),
                    now_ms,
                    probe_version,
                    probe_error,
                    container,
                    duration_ms.map(|d| d as i64),
                    bitrate.map(|b| b as i64),
                    PARSE_VERSION,
                ],
            )?;
            tx.last_insert_rowid()
        }
    };
    tx.execute("DELETE FROM media_stream WHERE file_id = ?1", [file_id])?;
    if let Ok(info) = probe {
        insert_streams(tx, file_id, info)?;
    }
    place::write_filename_layers(tx, item_id, now_ms)?;
    if let Some((_, old_item)) = existing
        && old_item != item_id
    {
        place::write_filename_layers(tx, old_item, now_ms)?;
        place::mark_missing_if_empty(tx, old_item, now_ms)?;
    }
    bump_rev(tx)?;
    Ok(Some(FileId(file_id)))
}

fn insert_streams(tx: &Transaction<'_>, file_id: i64, info: &MediaInfo) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO media_stream (file_id, stream_index, kind, codec, profile, level, bit_depth, width, height, \
         frame_rate, hdr, dv_profile, color_primaries, color_transfer, color_space, channels, channel_layout, \
         sample_rate, language, title, is_default, is_forced, dv_compatibility) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
    )?;
    for v in &info.video {
        let (dv_profile, dv_compatibility) = match v.hdr {
            HdrFormat::DolbyVision {
                profile,
                compatibility,
            } => (Some(profile), Some(compatibility)),
            _ => (None, None),
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
            dv_compatibility,
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
            Option::<u8>::None,
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
            Option::<u8>::None,
        ])?;
    }
    Ok(())
}

/// Apply a rename/move: update `root_id`, `rel_path`, `mtime_ns`; keep the id,
/// streams and artwork. Then place the file by its new name
/// ([`place::repoint`]): same key, nothing else changes; a typo fix re-keys
/// the item in place, so its id survives. A name that no longer fits the
/// library's type takes the file out of the catalog.
pub(crate) fn move_file(tx: &Transaction<'_>, mv: &Move, now_ms: i64) -> rusqlite::Result<()> {
    let old_item: i64 = tx.query_row(
        "SELECT item_id FROM media_file WHERE id = ?1",
        [mv.id.0],
        |r| r.get(0),
    )?;
    tx.execute(
        "UPDATE media_file SET root_id = ?1, rel_path = ?2, mtime_ns = ?3 WHERE id = ?4",
        params![mv.to_root.0, mv.to.rel.0, mv.to.mtime_ns, mv.id.0],
    )?;
    let kind = place::library_kind(tx, mv.to_library_id)?;
    match Placement::of(kind, &mv.to.rel.0) {
        Some(placement) => {
            place::repoint(tx, mv.id.0, old_item, mv.to_library_id, &placement, now_ms)?
        }
        None => {
            tx.execute("DELETE FROM media_file WHERE id = ?1", [mv.id.0])?;
            place::mark_missing_if_empty(tx, old_item, now_ms)?;
        }
    }
    bump_rev(tx)
}

/// Places a stored, unchanged file again with the current parser (its
/// `parse_version` is older than [`PARSE_VERSION`]): its titles are
/// rewritten, and it moves to another item only when its name now parses
/// to one. Nothing is probed.
pub(crate) fn rederive(tx: &Transaction<'_>, file: FileId, now_ms: i64) -> rusqlite::Result<()> {
    let Some((item, library, rel)): Option<(i64, i64, String)> = tx
        .query_row(
            "SELECT f.item_id, r.library_id, f.rel_path FROM media_file f \
             JOIN library_root r ON r.id = f.root_id WHERE f.id = ?1",
            [file.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
    else {
        return Ok(());
    };
    let kind = place::library_kind(tx, library)?;
    let Some(placement) = Placement::of(kind, &rel) else {
        return Ok(());
    };
    place::repoint(tx, file.0, item, library, &placement, now_ms)?;
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

/// Per show while loading: `(specials last, season, episode)` of its first
/// episode, that episode's files, and the first folder every episode file so
/// far shares.
type FirstEpisode = ((bool, i64, i64), Vec<FileForArtwork>, Option<String>);

/// Present items with their files, for artwork planning: movies and
/// episodes with all their files, and shows with their first episode's (in
/// order, specials last) and the folder every episode of the show is under.
/// A file is SDR when it probed and its primary video stream is not HDR.
pub(crate) fn load_present_items(conn: &Connection) -> rusqlite::Result<Vec<ItemFiles>> {
    let mut stmt = conn.prepare_cached(
        "SELECT i.id, i.kind, f.id, f.root_id, f.rel_path, f.fingerprint, f.duration_ms, f.probe_error, \
         (SELECT s.hdr FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'video' ORDER BY s.stream_index LIMIT 1), \
         (SELECT s.height FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'video' ORDER BY s.stream_index LIMIT 1), \
         se.parent_id, se.index_number, i.index_number \
         FROM item i JOIN media_file f ON f.item_id = i.id LEFT JOIN item se ON se.id = i.parent_id \
         WHERE i.missing_since IS NULL ORDER BY i.id, f.id",
    )?;
    let mut items: Vec<ItemFiles> = Vec::new();
    // Per show: (season, episode) of the first episode, its files, and the
    // common first folder of every episode file so far.
    let mut shows: std::collections::BTreeMap<i64, FirstEpisode> =
        std::collections::BTreeMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let item = ItemId(r.get(0)?);
        let episode = r.get_ref(1)?.as_str()? == "episode";
        let probe_error: Option<String> = r.get(7)?;
        let hdr: Option<String> = r.get(8)?;
        let file = FileForArtwork {
            id: FileId(r.get(2)?),
            root: RootId(r.get(3)?),
            rel: RelPath(r.get(4)?),
            fingerprint: blob16(r.get(5)?),
            duration_ms: r.get::<_, Option<i64>>(6)?.map(|d| d as u64),
            sdr: probe_error.is_none() && hdr.as_deref().is_none_or(|h| h == "sdr"),
            height: r.get::<_, Option<u32>>(9)?.unwrap_or(0),
        };
        if let (true, Some(show)) = (episode, r.get::<_, Option<i64>>(10)?) {
            let season: i64 = r.get::<_, Option<i64>>(11)?.unwrap_or(0);
            let order = (
                season == 0,
                season,
                r.get::<_, Option<i64>>(12)?.unwrap_or(0),
            );
            let top = file.rel.0.split_once('/').map(|(top, _)| top.to_owned());
            match shows.get_mut(&show) {
                None => {
                    shows.insert(show, (order, vec![file.clone()], top));
                }
                Some((first, files, folder)) => {
                    if *folder != top {
                        *folder = None;
                    }
                    if order < *first {
                        *first = order;
                        files.clear();
                    }
                    if order == *first {
                        files.push(file.clone());
                    }
                }
            }
        }
        let kind = if episode {
            ArtKind::Episode
        } else {
            ArtKind::Movie
        };
        match items.last_mut() {
            Some(last) if last.item == item => last.files.push(file),
            _ => items.push(ItemFiles {
                item,
                kind,
                files: vec![file],
            }),
        }
    }
    items.extend(
        shows
            .into_iter()
            .map(|(show, (_, files, folder))| ItemFiles {
                item: ItemId(show),
                kind: ArtKind::Show {
                    folder: folder.map(RelPath),
                },
                files,
            }),
    );
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
        changes += place::mark_empty_items_missing(tx, now_ms)?;
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
    conn.prepare_cached(
        "SELECT hash FROM artwork UNION SELECT image FROM item_credit WHERE image IS NOT NULL \
         UNION SELECT character_image FROM item_credit WHERE character_image IS NOT NULL",
    )?
    .query_map([], |r| Ok(ArtworkId(blob16(r.get(0)?))))?
    .collect()
}

/// `UPDATE catalog SET rev = rev + 1`. Home and row ETags are this number, so a
/// write that changes visible rows without bumping it would serve stale 304s.
pub(crate) fn bump_rev(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("UPDATE catalog SET rev = rev + 1 WHERE id = 1", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::scan::walk::DiskFile;
    use crate::views;
    use crate::{Item, UserId, VersionMedia};
    use nuvora_media::{ColorInfo, Container, DeviceProfile, VideoCodec, VideoStream};

    fn dolby_vision(profile: u8, compatibility: u8) -> MediaInfo {
        MediaInfo {
            container: Container::Matroska,
            duration_ms: Some(10_000),
            bitrate: None,
            video: vec![VideoStream {
                index: 0,
                codec: VideoCodec::Hevc,
                profile: None,
                level: None,
                width: 3840,
                height: 2160,
                bit_depth: Some(10),
                frame_rate: None,
                hdr: HdrFormat::DolbyVision {
                    profile,
                    compatibility,
                },
                color: ColorInfo::default(),
                language: None,
                title: None,
                is_default: true,
            }],
            audio: vec![],
            subtitles: vec![],
        }
    }

    /// Store `info` as the only file of a fresh library and read its video HDR back.
    async fn round_trip(info: MediaInfo) -> HdrFormat {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("nuvora.db"), 1).unwrap();
        db.write(move |tx| {
            tx.execute(
                "INSERT INTO library (key, name, kind) VALUES ('movies', 'Movies', 'movies')",
                [],
            )?;
            tx.execute(
                "INSERT INTO library_root (library_id, path) VALUES (1, '/media')",
                [],
            )?;
            let job = ProbeJob {
                root: RootId(1),
                library_id: 1,
                disk: DiskFile {
                    rel: RelPath("Heat (1995)/Heat (1995).mkv".into()),
                    size: 1,
                    mtime_ns: 1,
                },
                existing: None,
            };
            upsert_file(tx, &job, [0; 16], &Ok(info), 0)
        })
        .await
        .unwrap();
        let item = db
            .read(|c| views::item(c, ItemId(1), &DeviceProfile::baseline(), UserId(0)))
            .await
            .unwrap();
        let Some(Item::Movie(movie)) = item else {
            panic!("stored movie")
        };
        let VersionMedia::Probed { info, .. } = &movie.versions[0].media else {
            panic!("probed version")
        };
        info.video[0].hdr
    }

    #[tokio::test]
    async fn dolby_vision_compatibility_survives_the_database() {
        for hdr in [dolby_vision(7, 6), dolby_vision(8, 4), dolby_vision(5, 0)] {
            let stored = hdr.video[0].hdr;
            assert_eq!(round_trip(hdr).await, stored);
        }
    }
}
