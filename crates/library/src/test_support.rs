//! Test data: `seed_movies` writes `n` movies straight into the tables for
//! tests that need a populated catalog without a scan, and `movie_library`
//! generates a small on-disk library with ffmpeg for scan tests.

use std::path::Path;
use std::process::Command;

use rusqlite::{Transaction, params};

/// Run `ffmpeg -y -nostdin -v error <args> <out>`; panics with ffmpeg's stderr on failure.
pub(crate) fn ffmpeg(args: &[&str], out: &Path) {
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let output = Command::new("ffmpeg")
        .args(["-y", "-nostdin", "-v", "error"])
        .args(args)
        .arg(out)
        .output()
        .expect("ffmpeg on PATH");
    assert!(
        output.status.success(),
        "ffmpeg {args:?} -> {}: {}",
        out.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// One second of `testsrc2` + `sine`, SDR H.264 + AAC, in the container `out` names.
pub(crate) fn sdr_movie(out: &Path) {
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=24:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-ac",
            "2",
        ],
        out,
    );
}

/// The verification library's shape: four SDR movies (one with local poster
/// and fanart, one named scene-style, one with a sample next to it, one loose
/// file) and one HDR10 movie that must get no frame backdrop.
pub(crate) fn movie_library(root: &Path) {
    sdr_movie(&root.join("Big Buck (2008)/Big Buck (2008).mp4"));
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=600x900",
            "-frames:v",
            "1",
        ],
        &root.join("Big Buck (2008)/poster.jpg"),
    );
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720",
            "-frames:v",
            "1",
        ],
        &root.join("Big Buck (2008)/fanart.jpg"),
    );
    sdr_movie(&root.join("Sintel (2010)/Sintel.2010.1080p.BluRay.x264-GRP.mkv"));
    sdr_movie(&root.join("Elephants (2006)/Elephants.2006.mp4"));
    sdr_movie(&root.join("Elephants (2006)/Elephants.2006.sample.mp4"));
    sdr_movie(&root.join("Loose/The.Spring.2019.720p.WEB-DL.mp4"));
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=24:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx265",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p10le",
            "-x265-params",
            "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:log-level=error",
            "-c:a",
            "aac",
        ],
        &root.join("Bright (2020)/Bright (2020).mkv"),
    );
    std::fs::write(root.join("Big Buck (2008)/movie.nfo"), "<movie/>").unwrap();
}

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
