//! The read side of online metadata: what the movie and show pages show
//! beyond title, year and synopsis.

use nuvora_media::{ArtworkRole, Placeholder};
use rusqlite::{Connection, OptionalExtension};

use crate::ids::{ArtworkId, ItemId};
use crate::views::Artwork;

/// Genres, rating, studio, the provider's page, and cast. Empty until the
/// metadata job has matched the item.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    /// 0..10.
    pub rating: Option<f32>,
    pub studio: Option<String>,
    /// e.g. `https://anilist.co/anime/153406`.
    pub external_url: Option<String>,
    /// `"AniList"`, `"TVmaze"` or `"TMDB"`.
    pub source: Option<&'static str>,
    pub credits: Vec<Credit>,
}

#[derive(Debug, Clone)]
pub struct Credit {
    /// `"cast"` or `"voice"`.
    pub kind: String,
    pub name: String,
    pub character: Option<String>,
    /// For voice actors: which dub.
    pub language: Option<String>,
    pub image: Option<Artwork>,
    pub character_image: Option<Artwork>,
}

fn art(hash: Option<Vec<u8>>, aspect: Option<f64>, ph: Option<Vec<u8>>) -> Option<Artwork> {
    let hash: [u8; 16] = hash?.try_into().ok()?;
    let aspect = aspect.unwrap_or(2.0 / 3.0);
    Some(Artwork {
        id: ArtworkId(hash),
        role: ArtworkRole::Poster,
        width: (aspect * 1000.0).round() as u32,
        height: 1000,
        placeholder: Placeholder(ph.unwrap_or_default()),
    })
}

pub(crate) fn extras(conn: &Connection, item: ItemId) -> rusqlite::Result<Extras> {
    let head = conn
        .prepare_cached(
            "SELECT original_title, tagline, genres, rating, studio, external_url, provider_id \
             FROM item_metadata WHERE item_id = ?1 AND source = 'online'",
        )?
        .query_row([item.0], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })
        .optional()?;
    let Some((original_title, tagline, genres, rating, studio, external_url, provider)) = head
    else {
        return Ok(Extras::default());
    };
    let credits = conn
        .prepare_cached(
            "SELECT kind, name, character, language, image, image_aspect, image_ph, \
             character_image, character_aspect, character_ph FROM item_credit WHERE item_id = ?1 ORDER BY ord",
        )?
        .query_map([item.0], |r| {
            Ok(Credit {
                kind: r.get(0)?,
                name: r.get(1)?,
                character: r.get(2)?,
                language: r.get(3)?,
                image: art(r.get(4)?, r.get(5)?, r.get(6)?),
                character_image: art(r.get(7)?, r.get(8)?, r.get(9)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let source = match provider.as_deref().and_then(|p| p.split(':').next()) {
        Some("anilist") => Some("AniList"),
        Some("tvmaze") => Some("TVmaze"),
        Some("tmdb") => Some("TMDB"),
        _ => None,
    };
    Ok(Extras {
        original_title,
        tagline,
        genres: genres
            .and_then(|g| serde_json::from_str(&g).ok())
            .unwrap_or_default(),
        rating: rating.map(|r| r as f32),
        studio,
        external_url,
        source,
        credits,
    })
}
