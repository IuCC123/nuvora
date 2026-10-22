//! Search across every library: movies and shows by any of their titles,
//! episodes by theirs, and people (actors, voice actors) by their name or
//! a character they play.
//!
//! Matching is word-prefix, case- and accent-blind: every word typed must
//! start a word of the title, so "tow god" finds "Tower of God" and
//! "ouji" finds it by its file name too. A household library is a few
//! thousand titles, so this reads them all and ranks in memory.

use std::collections::HashMap;

use nuvora_media::{ArtworkRole, fold_for_sort};
use rusqlite::Connection;

use crate::ids::{ItemId, UserId};
use crate::metadata::read::art;
use crate::views::{self, Artwork, Card};
use crate::{ItemKind, Library, QueryError, shows};

const TITLES: usize = 40;
const EPISODES: usize = 20;
const PEOPLE: usize = 12;
const ROLES: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    /// Movies and shows, best match first.
    pub titles: Vec<Card>,
    pub episodes: Vec<EpisodeHit>,
    pub people: Vec<PersonHit>,
}

#[derive(Debug, Clone)]
pub struct EpisodeHit {
    pub id: ItemId,
    pub show: ItemId,
    pub show_title: String,
    pub season: u32,
    pub episode: u32,
    pub title: String,
    /// The show's poster.
    pub poster: Option<Artwork>,
}

#[derive(Debug, Clone)]
pub struct PersonHit {
    pub name: String,
    pub image: Option<Artwork>,
    /// What they are in, billing order within each title.
    pub roles: Vec<PersonRole>,
}

#[derive(Debug, Clone)]
pub struct PersonRole {
    pub card: Card,
    pub character: Option<String>,
}

impl Library {
    /// Everything matching `query` for `user`. An empty query (after folding)
    /// finds nothing.
    pub async fn search(&self, query: String, user: UserId) -> Result<SearchResults, QueryError> {
        Ok(self.inner.db.read(move |c| search(c, &query, user)).await?)
    }
}

/// Lowercased, accents folded, split on anything that isn't a letter or digit.
fn words(text: &str) -> Vec<String> {
    fold_for_sort(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// How well `title` matches the query words `q`; `None` when it doesn't.
/// Whole title > starts with the query > every word a prefix > squashed substring.
fn score(q: &[String], title: &str) -> Option<u32> {
    let t = words(title);
    if t.is_empty() {
        return None;
    }
    let (qj, tj) = (q.join(" "), t.join(" "));
    let shorter = 100u32.saturating_sub(tj.len() as u32).min(99);
    if tj == qj {
        return Some(1000);
    }
    if tj.starts_with(&qj) {
        return Some(800 + shorter);
    }
    if q.iter()
        .all(|w| t.iter().any(|tw| tw.starts_with(w.as_str())))
    {
        return Some(600 + shorter);
    }
    if q.concat().len() >= 3 && t.concat().contains(&q.concat()) {
        return Some(300 + shorter);
    }
    None
}

fn search(conn: &Connection, query: &str, user: UserId) -> rusqlite::Result<SearchResults> {
    let q = words(query);
    if q.is_empty() {
        return Ok(SearchResults::default());
    }
    let mut results = SearchResults {
        titles: titles(conn, &q, user)?,
        episodes: episodes(conn, &q)?,
        people: people(conn, &q, user)?,
    };
    results.people.retain(|p| !p.roles.is_empty());
    Ok(results)
}

/// Present movies and shows, matched on every title they have: shown,
/// original, and each metadata layer's (file name, online, user).
fn titles(conn: &Connection, q: &[String], user: UserId) -> rusqlite::Result<Vec<Card>> {
    let mut stmt = conn.prepare_cached(
        "SELECT d.id, d.sort_title, d.title, m.title, m.original_title FROM item_display d \
         LEFT JOIN item_metadata m ON m.item_id = d.id \
         WHERE d.kind IN ('movie', 'show') AND d.missing_since IS NULL",
    )?;
    let mut best: HashMap<i64, (u32, String)> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let sort: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
        for col in 2..=4 {
            let Some(title) = r.get::<_, Option<String>>(col)? else {
                continue;
            };
            if let Some(s) = score(q, &title) {
                let entry = best.entry(id).or_insert((0, sort.clone()));
                entry.0 = entry.0.max(s);
            }
        }
    }
    let mut ranked: Vec<(i64, u32, String)> = best
        .into_iter()
        .map(|(id, (s, sort))| (id, s, sort))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
    let mut cards = Vec::new();
    for (id, _, _) in ranked.into_iter().take(TITLES) {
        cards.extend(card(conn, ItemId(id), user)?);
    }
    Ok(cards)
}

/// A movie or show card with `user`'s watch state, as the rows show it.
fn card(conn: &Connection, id: ItemId, user: UserId) -> rusqlite::Result<Option<Card>> {
    let Some(mut card) = views::card_by_id(conn, id, user)? else {
        return Ok(None);
    };
    if card.kind == ItemKind::Show {
        shows::fill_show_card(conn, &mut card, user)?;
    }
    Ok(Some(card))
}

/// Present episodes whose own title matches, with their show's title and poster.
fn episodes(conn: &Connection, q: &[String]) -> rusqlite::Result<Vec<EpisodeHit>> {
    let mut stmt = conn.prepare_cached(
        "SELECT e.id, e.title, e.index_number, se.index_number, sh.id, sh.title, \
         p.hash, p.width, p.height, p.placeholder \
         FROM item_display e JOIN item se ON se.id = e.parent_id JOIN item_display sh ON sh.id = se.parent_id \
         LEFT JOIN item_artwork p ON p.item_id = sh.id AND p.role = 'poster' \
         WHERE e.kind = 'episode' AND e.missing_since IS NULL AND e.title IS NOT NULL",
    )?;
    let mut hits: Vec<(u32, EpisodeHit)> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let title: String = r.get(1)?;
        let Some(s) = score(q, &title) else {
            continue;
        };
        hits.push((
            s,
            EpisodeHit {
                id: ItemId(r.get(0)?),
                title,
                episode: r.get::<_, Option<u32>>(2)?.unwrap_or(0),
                season: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                show: ItemId(r.get(4)?),
                show_title: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                poster: views::artwork(r, 6, ArtworkRole::Poster)?,
            },
        ));
    }
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.show_title.cmp(&b.1.show_title))
            .then_with(|| (a.1.season, a.1.episode).cmp(&(b.1.season, b.1.episode)))
    });
    Ok(hits.into_iter().take(EPISODES).map(|(_, h)| h).collect())
}

/// Actors and voice actors by name or by a character's, each with what they are in.
fn people(conn: &Connection, q: &[String], user: UserId) -> rusqlite::Result<Vec<PersonHit>> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.name, c.character, c.item_id, c.image, c.image_aspect, c.image_ph FROM item_credit c \
         JOIN item i ON i.id = c.item_id WHERE i.missing_since IS NULL ORDER BY c.item_id, c.ord",
    )?;
    struct Found {
        score: u32,
        image: Option<Artwork>,
        roles: Vec<(ItemId, Option<String>)>,
    }
    let mut found: HashMap<String, Found> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let name: String = r.get(0)?;
        let character: Option<String> = r.get(1)?;
        // A character's name finds whoever plays or voices them, below a match on the person.
        let by_character = character
            .as_deref()
            .and_then(|c| score(q, c))
            .map(|s| s.saturating_sub(100));
        let Some(s) = score(q, &name).max(by_character) else {
            continue;
        };
        let person = found.entry(name).or_insert(Found {
            score: s,
            image: None,
            roles: Vec::new(),
        });
        if person.image.is_none() {
            person.image = art(r.get(3)?, r.get(4)?, r.get(5)?);
        }
        let item = ItemId(r.get(2)?);
        person.score = person.score.max(s);
        if person.roles.len() < ROLES && !person.roles.iter().any(|(i, _)| *i == item) {
            person.roles.push((item, character));
        }
    }
    let mut ranked: Vec<(String, Found)> = found.into_iter().collect();
    ranked.sort_by(|a, b| b.1.score.cmp(&a.1.score).then_with(|| a.0.cmp(&b.0)));
    let mut people = Vec::new();
    for (name, person) in ranked.into_iter().take(PEOPLE) {
        let mut roles = Vec::new();
        for (item, character) in person.roles {
            if let Some(card) = card(conn, item, user)? {
                roles.push(PersonRole { card, character });
            }
        }
        people.push(PersonHit {
            name,
            image: person.image,
            roles,
        });
    }
    Ok(people)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> Vec<String> {
        words(text)
    }

    #[test]
    fn word_prefixes_match_accent_and_case_blind() {
        assert_eq!(score(&q("tower of god"), "Tower of God"), Some(1000));
        assert!(score(&q("tow god"), "Tower of God").is_some());
        assert!(score(&q("tower"), "Tower of God Season 2").unwrap() >= 800);
        assert!(score(&q("pokemon"), "Pokémon: The First Movie").is_some());
        assert!(score(&q("AMELIE"), "Amélie").is_some());
        assert!(score(&q("laundro"), "Cosmos Laundromat").is_some());
        assert_eq!(score(&q("god tower x"), "Tower of God"), None);
        assert_eq!(score(&q("ow"), "Tower of God"), None);
    }

    #[test]
    fn closer_matches_rank_higher() {
        let exact = score(&q("sintel"), "Sintel").unwrap();
        let starts = score(&q("sintel"), "Sintel Sample").unwrap();
        let word = score(&q("sample"), "Sintel Sample").unwrap();
        assert!(exact > starts && starts > word, "{exact} {starts} {word}");
    }
}
