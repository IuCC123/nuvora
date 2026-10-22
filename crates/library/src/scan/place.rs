//! Which item a file belongs to. A movie library puts each file in a movie
//! item; a TV library puts it in an episode, under a season, under a show.
//! Every function here runs inside a write transaction and is safe to repeat.
//!
//! An item's `identity_key` is derived from file names only (see the `item`
//! table). Shows, seasons and episodes have keys of their own shape
//! (`show:<title>|<year>`, `...|s1`, `...|s1|e2`), so they never collide with
//! a movie key in the same library.

use std::path::Path;

use nuvora_media::{EpisodeName, MovieName, PARSE_VERSION};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::LibraryKind;

/// What a file's name says it is, for its library's type.
#[derive(Debug, Clone)]
pub(crate) enum Placement {
    Movie(MovieName),
    Episode(EpisodeName),
}

impl Placement {
    /// `None` in a TV library for a name without an episode number.
    pub(crate) fn of(kind: LibraryKind, rel: &str) -> Option<Placement> {
        match kind {
            LibraryKind::Movies => Some(Placement::Movie(MovieName::parse(Path::new(rel)))),
            LibraryKind::Shows => EpisodeName::parse(Path::new(rel)).map(Placement::Episode),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Placement::Movie(_) => "movie",
            Placement::Episode(_) => "episode",
        }
    }

    fn key(&self) -> String {
        match self {
            Placement::Movie(name) => name.identity_key().as_str().to_owned(),
            Placement::Episode(name) => name.episode_key().as_str().to_owned(),
        }
    }
}

pub(crate) fn library_kind(tx: &Transaction<'_>, library: i64) -> rusqlite::Result<LibraryKind> {
    let name: String = tx.query_row("SELECT kind FROM library WHERE id = ?1", [library], |r| {
        r.get(0)
    })?;
    LibraryKind::from_name(&name).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("unknown library kind {name:?}").into(),
        )
    })
}

/// The movie or episode item for `placement`, with its show and season,
/// each created when new and revived when missing.
pub(crate) fn place(
    tx: &Transaction<'_>,
    library: i64,
    placement: &Placement,
    now_ms: i64,
) -> rusqlite::Result<i64> {
    match placement {
        Placement::Movie(name) => find_or_create(
            tx,
            library,
            "movie",
            name.identity_key().as_str(),
            None,
            None,
            now_ms,
        ),
        Placement::Episode(name) => {
            let season = place_season(tx, library, name, now_ms)?;
            find_or_create(
                tx,
                library,
                "episode",
                name.episode_key().as_str(),
                Some(season),
                Some(name.episode),
                now_ms,
            )
        }
    }
}

fn place_season(
    tx: &Transaction<'_>,
    library: i64,
    name: &EpisodeName,
    now_ms: i64,
) -> rusqlite::Result<i64> {
    let show = find_or_create(
        tx,
        library,
        "show",
        name.show_key().as_str(),
        None,
        None,
        now_ms,
    )?;
    find_or_create(
        tx,
        library,
        "season",
        name.season_key().as_str(),
        Some(show),
        Some(name.season),
        now_ms,
    )
}

fn find_or_create(
    tx: &Transaction<'_>,
    library: i64,
    kind: &str,
    key: &str,
    parent: Option<i64>,
    index: Option<u32>,
    now_ms: i64,
) -> rusqlite::Result<i64> {
    let found: Option<(i64, Option<i64>)> = tx
        .query_row(
            "SELECT id, missing_since FROM item WHERE library_id = ?1 AND identity_key = ?2",
            params![library, key],
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
                "INSERT INTO item (library_id, kind, parent_id, index_number, identity_key, added_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![library, kind, parent, index, key, now_ms],
            )?;
            Ok(tx.last_insert_rowid())
        }
    }
}

/// Points stored file `file` (now in `old_item`) at the item its name gives
/// in `library`, keeping ids where it can:
/// - same key => nothing moves;
/// - the key belongs to another item => the file becomes a version there;
/// - the key is unused and the file is its item's only file => the item is
///   re-keyed in place (and re-parented, for an episode), so its id and its
///   watch state survive a rename;
/// - otherwise a new item is created.
///
/// The old item is marked missing when it is left empty, and so are its
/// season and show when they are left empty. Records [`PARSE_VERSION`].
pub(crate) fn repoint(
    tx: &Transaction<'_>,
    file: i64,
    old_item: i64,
    library: i64,
    placement: &Placement,
    now_ms: i64,
) -> rusqlite::Result<()> {
    let (old_library, old_kind, old_key): (i64, String, String) = tx.query_row(
        "SELECT library_id, kind, identity_key FROM item WHERE id = ?1",
        [old_item],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let key = placement.key();
    let target = if old_library == library && old_key == key {
        old_item
    } else {
        let owner: Option<i64> = tx
            .query_row(
                "SELECT id FROM item WHERE library_id = ?1 AND identity_key = ?2",
                params![library, key],
                |r| r.get(0),
            )
            .optional()?;
        let files_in_old: i64 = tx.query_row(
            "SELECT count(*) FROM media_file WHERE item_id = ?1",
            [old_item],
            |r| r.get(0),
        )?;
        match owner {
            Some(_) => place(tx, library, placement, now_ms)?,
            None if files_in_old == 1 && old_kind == placement.kind() => {
                let old_parent: Option<i64> = tx.query_row(
                    "SELECT parent_id FROM item WHERE id = ?1",
                    [old_item],
                    |r| r.get(0),
                )?;
                let (parent, index) = match placement {
                    Placement::Movie(_) => (None, None),
                    Placement::Episode(name) => (
                        Some(place_season(tx, library, name, now_ms)?),
                        Some(name.episode),
                    ),
                };
                tx.execute(
                    "UPDATE item SET library_id = ?1, identity_key = ?2, parent_id = ?3, index_number = ?4, \
                     missing_since = NULL WHERE id = ?5",
                    params![library, key, parent, index, old_item],
                )?;
                if let Some(old_parent) = old_parent.filter(|p| Some(*p) != parent) {
                    mark_missing_if_empty(tx, old_parent, now_ms)?;
                }
                old_item
            }
            None => place(tx, library, placement, now_ms)?,
        }
    };
    tx.execute(
        "UPDATE media_file SET item_id = ?1, parse_version = ?2 WHERE id = ?3",
        params![target, PARSE_VERSION, file],
    )?;
    if target != old_item {
        write_filename_layers(tx, old_item, now_ms)?;
        mark_missing_if_empty(tx, old_item, now_ms)?;
    }
    write_filename_layers(tx, target, now_ms)
}

/// The `filename` metadata layer of `item` and, for an episode, of its show.
/// A movie's or an episode's comes from its oldest file, a show's from the
/// oldest file of any of its episodes, so which file was read last never
/// changes a title. A season has none: its title is its number. Written
/// only when it differs.
pub(crate) fn write_filename_layers(
    tx: &Transaction<'_>,
    item: i64,
    now_ms: i64,
) -> rusqlite::Result<()> {
    let Some((kind, parent)): Option<(String, Option<i64>)> = tx
        .query_row(
            "SELECT kind, parent_id FROM item WHERE id = ?1",
            [item],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
    else {
        return Ok(());
    };
    match kind.as_str() {
        "movie" => {
            if let Some(rel) = oldest_file(tx, "WHERE f.item_id = ?1", item)? {
                let name = MovieName::parse(Path::new(&rel));
                upsert_layer(
                    tx,
                    item,
                    Some(&name.title),
                    Some(&name.sort_title),
                    name.year,
                    now_ms,
                )?;
            }
        }
        "episode" => {
            if let Some(rel) = oldest_file(tx, "WHERE f.item_id = ?1", item)?
                && let Some(name) = EpisodeName::parse(Path::new(&rel))
            {
                upsert_layer(tx, item, name.title.as_deref(), None, None, now_ms)?;
            }
            let show: Option<i64> = match parent {
                Some(season) => tx
                    .query_row("SELECT parent_id FROM item WHERE id = ?1", [season], |r| {
                        r.get(0)
                    })
                    .optional()?
                    .flatten(),
                None => None,
            };
            if let Some(show) = show {
                write_filename_layers(tx, show, now_ms)?;
            }
        }
        "show" => {
            let oldest = oldest_file(
                tx,
                "JOIN item e ON e.id = f.item_id JOIN item s ON s.id = e.parent_id WHERE s.parent_id = ?1",
                item,
            )?;
            if let Some(name) = oldest.and_then(|rel| EpisodeName::parse(Path::new(&rel))) {
                let show = name.show;
                upsert_layer(
                    tx,
                    item,
                    Some(&show.title),
                    Some(&show.sort_title),
                    show.year,
                    now_ms,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn oldest_file(tx: &Transaction<'_>, from: &str, item: i64) -> rusqlite::Result<Option<String>> {
    tx.query_row(
        &format!("SELECT f.rel_path FROM media_file f {from} ORDER BY f.added_at, f.id LIMIT 1"),
        [item],
        |r| r.get(0),
    )
    .optional()
}

fn upsert_layer(
    tx: &Transaction<'_>,
    item: i64,
    title: Option<&str>,
    sort_title: Option<&str>,
    year: Option<u16>,
    now_ms: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO item_metadata (item_id, source, title, sort_title, year, updated_at) \
         VALUES (?1, 'filename', ?2, ?3, ?4, ?5) \
         ON CONFLICT(item_id, source) DO UPDATE SET title = excluded.title, sort_title = excluded.sort_title, \
         year = excluded.year, updated_at = excluded.updated_at \
         WHERE title IS NOT excluded.title OR sort_title IS NOT excluded.sort_title OR year IS NOT excluded.year",
        params![item, title, sort_title, year, now_ms],
    )?;
    Ok(())
}

/// Marks `item` missing when it has nothing left (a movie or episode without
/// files, a season or show without present children), then its parent the
/// same way.
pub(crate) fn mark_missing_if_empty(
    tx: &Transaction<'_>,
    item: i64,
    now_ms: i64,
) -> rusqlite::Result<()> {
    let mut next = Some(item);
    while let Some(item) = next {
        let changed = tx.execute(
            "UPDATE item SET missing_since = ?1 WHERE id = ?2 AND missing_since IS NULL AND ( \
             (kind IN ('movie', 'episode') AND NOT EXISTS (SELECT 1 FROM media_file WHERE item_id = ?2)) \
             OR (kind IN ('show', 'season') AND NOT EXISTS \
                 (SELECT 1 FROM item c WHERE c.parent_id = ?2 AND c.missing_since IS NULL)))",
            params![now_ms, item],
        )?;
        if changed == 0 {
            break;
        }
        next = tx.query_row("SELECT parent_id FROM item WHERE id = ?1", [item], |r| {
            r.get(0)
        })?;
    }
    Ok(())
}

/// Marks every item with nothing left missing, bottom up: movies and
/// episodes without files, then seasons, then shows without present children.
pub(crate) fn mark_empty_items_missing(
    tx: &Transaction<'_>,
    now_ms: i64,
) -> rusqlite::Result<usize> {
    let mut changed = tx.execute(
        "UPDATE item SET missing_since = ?1 WHERE missing_since IS NULL AND kind IN ('movie', 'episode') \
         AND NOT EXISTS (SELECT 1 FROM media_file f WHERE f.item_id = item.id)",
        [now_ms],
    )?;
    for kind in ["season", "show"] {
        changed += tx.execute(
            "UPDATE item SET missing_since = ?1 WHERE missing_since IS NULL AND kind = ?2 \
             AND NOT EXISTS (SELECT 1 FROM item c WHERE c.parent_id = item.id AND c.missing_since IS NULL)",
            params![now_ms, kind],
        )?;
    }
    Ok(changed)
}

/// Rebuilds every item of `library` for its (new) type: each file is placed
/// again, a file that is no episode in a TV library leaves the catalog (the
/// walk skips it from now on), and the items of the old type are deleted
/// with their watch state and artwork. File rows, streams and probes are
/// kept, so nothing is read again; the next scan renders the new artwork.
pub(crate) fn rebuild(tx: &Transaction<'_>, library: i64, now_ms: i64) -> rusqlite::Result<()> {
    let kind = library_kind(tx, library)?;
    let files: Vec<(i64, String)> = tx
        .prepare(
            "SELECT f.id, f.rel_path FROM media_file f JOIN library_root r ON r.id = f.root_id \
             WHERE r.library_id = ?1 ORDER BY f.added_at, f.id",
        )?
        .query_map([library], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (file, rel) in files {
        match Placement::of(kind, &rel) {
            Some(placement) => {
                let item = place(tx, library, &placement, now_ms)?;
                tx.execute(
                    "UPDATE media_file SET item_id = ?1, parse_version = ?2 WHERE id = ?3",
                    params![item, PARSE_VERSION, file],
                )?;
            }
            None => {
                tx.execute("DELETE FROM media_file WHERE id = ?1", [file])?;
            }
        }
    }
    let stale = match kind {
        LibraryKind::Movies => "('show', 'season', 'episode')",
        LibraryKind::Shows => "('movie')",
    };
    tx.execute(
        &format!("DELETE FROM item WHERE library_id = ?1 AND kind IN {stale}"),
        [library],
    )?;
    let items: Vec<i64> = tx
        .prepare("SELECT id FROM item WHERE library_id = ?1 AND kind IN ('movie', 'episode')")?
        .query_map([library], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for item in items {
        write_filename_layers(tx, item, now_ms)?;
    }
    mark_empty_items_missing(tx, now_ms)?;
    Ok(())
}
