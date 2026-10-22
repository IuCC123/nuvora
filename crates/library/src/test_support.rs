//! Direct database seeding for tests that need a populated catalog without a
//! scan: `seed_movies` writes `n` movies straight into the tables.

use rusqlite::{Transaction, params};

/// `n` movies in one `movies` library. Titles alternate between `Movie NNNN`
/// and `A Movie NNNN` (so sort titles differ from titles), `added_at` repeats
/// every three items (so keyset paging has ties to get right), every second
/// item has a poster, and item 1 has a second 4K Matroska version.
pub(crate) fn seed_movies(tx: &Transaction<'_>, n: usize) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO library (key, name, kind) VALUES ('movies', 'Movies', 'movies')",
        [],
    )?;
    tx.execute(
        "INSERT INTO library_root (library_id, path) VALUES (1, '/media')",
        [],
    )?;
    let mut item = tx.prepare(
        "INSERT INTO item (library_id, kind, identity_key, added_at) VALUES (1, 'movie', ?1, ?2)",
    )?;
    let mut meta = tx.prepare(
        "INSERT INTO item_metadata (item_id, source, title, sort_title, year, updated_at) VALUES (?1, 'filename', ?2, ?3, ?4, 0)",
    )?;
    let mut file = tx.prepare(
        "INSERT INTO media_file (item_id, root_id, rel_path, size, mtime_ns, fingerprint, added_at, probe_version, \
         container, duration_ms, bitrate) VALUES (?1, 1, ?2, ?3, 0, zeroblob(16), ?4, 1, ?5, 5400000, ?6)",
    )?;
    let mut stream = tx.prepare(
        "INSERT INTO media_stream (file_id, stream_index, kind, codec, bit_depth, width, height, hdr, channels, is_default) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)",
    )?;
    let mut art = tx.prepare(
        "INSERT INTO artwork (item_id, role, source, source_sig, hash, width, height, placeholder) \
         VALUES (?1, 'poster', 'local', ?2, ?3, 640, 960, X'0102')",
    )?;
    for i in 0..n {
        let title = if i % 5 == 0 {
            format!("A Movie {i:04}")
        } else {
            format!("Movie {i:04}")
        };
        let sort_title = title
            .strip_prefix("A ")
            .map(|t| format!("{t}, A"))
            .unwrap_or_else(|| title.clone());
        let year = 2000 + (i % 25) as i64;
        let added_at = 1_700_000_000_000 + (i / 3) as i64 * 1000;
        item.execute(params![
            format!("{}|{year}", title.to_lowercase()),
            added_at
        ])?;
        let item_id = tx.last_insert_rowid();
        meta.execute(params![item_id, title, sort_title, year])?;
        let folder = format!("{title} ({year})");
        file.execute(params![
            item_id,
            format!("{folder}/{folder}.mp4"),
            4_000_000_000_i64,
            added_at,
            "mp4",
            8_000_000_i64
        ])?;
        let file_id = tx.last_insert_rowid();
        stream.execute(params![
            file_id,
            0,
            "video",
            "h264",
            8,
            1920,
            1080,
            "sdr",
            Option::<i64>::None
        ])?;
        stream.execute(params![
            file_id,
            1,
            "audio",
            "aac",
            Option::<i64>::None,
            Option::<i64>::None,
            Option::<i64>::None,
            Option::<String>::None,
            2
        ])?;
        if i == 0 {
            file.execute(params![
                item_id,
                format!("{folder}/{folder} - 4K.mkv"),
                30_000_000_000_i64,
                added_at,
                "mkv",
                40_000_000_i64
            ])?;
            let file_id = tx.last_insert_rowid();
            stream.execute(params![
                file_id,
                0,
                "video",
                "hevc",
                10,
                3840,
                2160,
                "hdr10",
                Option::<i64>::None
            ])?;
            stream.execute(params![
                file_id,
                1,
                "audio",
                "truehd",
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<String>::None,
                8
            ])?;
        }
        if i % 2 == 0 {
            let mut hash = [0u8; 16];
            hash[..8].copy_from_slice(&(i as u64).to_le_bytes());
            art.execute(params![
                item_id,
                format!("local:1:{folder}/poster.jpg:1:1"),
                hash.to_vec()
            ])?;
        }
    }
    tx.execute("UPDATE catalog SET rev = rev + 1", [])?;
    Ok(())
}
