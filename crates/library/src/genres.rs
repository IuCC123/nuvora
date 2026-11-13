//! Genres: one normalized key per genre, whatever the provider called it,
//! and a display name for each key. `item_genre` holds the keys; it is
//! rebuilt from `item_metadata.genres` when [`VERSION`] moves, so changing
//! the rules below only needs a bump.

use rusqlite::{Connection, params};

use crate::ids::ItemId;

/// Bump when [`key`] changes, so existing databases re-normalize on start.
const VERSION: i64 = 1;

/// Genres longer than this are free text, not genres.
const MAX_LEN: usize = 32;

/// The normalized genre for a provider's label, or `None` for labels that
/// are not genres. Lowercase, punctuation as spaces, and synonyms merged:
/// `"Science-Fiction"`, `"Sci-Fi"` and `"science fiction film"` are all
/// `"science fiction"`.
pub(crate) fn key(raw: &str) -> Option<String> {
    let lowered = raw.trim().to_lowercase().replace(['-', '_', '/'], " ");
    let mut words: Vec<&str> = lowered.split_whitespace().collect();
    // Wikidata labels genres by medium: "drama film", "anime television series".
    for suffix in [
        &["television", "series"][..],
        &["tv", "series"],
        &["film"],
        &["series"],
    ] {
        if words.len() > suffix.len() && words.ends_with(suffix) {
            words.truncate(words.len() - suffix.len());
            break;
        }
    }
    let joined = words.join(" ");
    let key = match joined.as_str() {
        "sci fi" | "scifi" | "science fiction" | "sci fi & fantasy" => "science fiction",
        "action & adventure" | "action adventure" => "action",
        "war & politics" => "war",
        "animated" | "animation" => "animation",
        "romantic" => "romance",
        "documentary" | "documentaries" => "documentary",
        "suspense" => "thriller",
        "musical" | "music" => "music",
        other => other,
    };
    (!key.is_empty() && key.len() <= MAX_LEN).then(|| key.to_owned())
}

/// `"science fiction"` -> `"Science Fiction"`, `"slice of life"` -> `"Slice of Life"`.
pub fn display_name(key: &str) -> String {
    key.split(' ')
        .enumerate()
        .map(|(i, word)| {
            if i > 0 && matches!(word, "of" | "and" | "the" | "&" | "in") {
                return word.to_owned();
            }
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Replace one item's genres with the keys of `labels`.
pub(crate) fn index_item(
    conn: &Connection,
    item: ItemId,
    labels: &[String],
) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM item_genre WHERE item_id = ?1")?
        .execute([item.0])?;
    let mut insert =
        conn.prepare_cached("INSERT OR IGNORE INTO item_genre (item_id, genre) VALUES (?1, ?2)")?;
    for genre in labels.iter().filter_map(|l| key(l)) {
        insert.execute(params![item.0, genre])?;
    }
    Ok(())
}

/// Rebuild `item_genre` from the online metadata when it was built by older
/// rules (or never). Idempotent; a no-op once current.
pub(crate) fn reindex_if_stale(conn: &mut Connection) -> rusqlite::Result<()> {
    let built: i64 = conn.query_row("SELECT genre_index FROM catalog WHERE id = 1", [], |r| {
        r.get(0)
    })?;
    if built >= VERSION {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM item_genre", [])?;
    let rows: Vec<(i64, String)> = tx
        .prepare("SELECT item_id, genres FROM item_metadata WHERE source = 'online' AND genres IS NOT NULL")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (item, json) in rows {
        let labels: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
        index_item(&tx, ItemId(item), &labels)?;
    }
    tx.execute(
        "UPDATE catalog SET genre_index = ?1 WHERE id = 1",
        [VERSION],
    )?;
    tx.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_labels_meet_in_one_key() {
        for label in [
            "Science-Fiction",
            "Sci-Fi",
            "science fiction film",
            "Science fiction",
            "Sci-Fi & Fantasy",
        ] {
            assert_eq!(key(label).as_deref(), Some("science fiction"), "{label}");
        }
        assert_eq!(key("Drama").as_deref(), Some("drama"));
        assert_eq!(key("drama film").as_deref(), Some("drama"));
        assert_eq!(key("anime television series").as_deref(), Some("anime"));
        assert_eq!(key("Slice of Life").as_deref(), Some("slice of life"));
        assert_eq!(key("  ").as_deref(), None);
        assert_eq!(
            key("a genre label that goes on far too long to be one").as_deref(),
            None
        );
    }

    #[test]
    fn display_names_read_like_titles() {
        assert_eq!(display_name("science fiction"), "Science Fiction");
        assert_eq!(display_name("slice of life"), "Slice of Life");
        assert_eq!(display_name("drama"), "Drama");
    }
}
