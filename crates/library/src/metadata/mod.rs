//! The metadata job: after each scan, look up shows and movies that have no
//! online metadata yet, and store what AniList, TVmaze or TMDB know about
//! them as the `online` layer: title, year, synopsis, genres, rating, studio,
//! poster, backdrop, cast and voice actors.
//!
//! One worker, one item at a time, with each provider's own spacing, so a big
//! first scan never trips a rate limit. Every lookup is recorded in
//! `metadata_lookup`; an item is not looked up again until the admin asks
//! (Refresh metadata) or a new provider becomes available (a TMDB key).

pub(crate) mod providers;
pub(crate) mod read;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use nuvora_media::{ArtworkRole, ArtworkSource};
use rusqlite::{OptionalExtension, params};
use tokio::sync::{mpsc, watch};

use crate::db::Db;
use crate::ids::{ArtworkId, ItemId};
use crate::scan::{self, ScanState, ScanStatus};
use crate::store::ArtworkStore;
use providers::{Found, ProviderError};

pub(crate) const TMDB_KEY: &str = "tmdb_api_key";

#[derive(Clone)]
pub(crate) struct MetadataHandle {
    tx: mpsc::Sender<()>,
}

impl MetadataHandle {
    /// Runs after every finished scan, and whenever [`MetadataHandle::request`] is called.
    /// Starts the lookup job, unless `enabled` is false: then requests are
    /// accepted and nothing runs.
    pub(crate) fn spawn(
        db: Db,
        store: Arc<ArtworkStore>,
        mut scans: watch::Receiver<ScanStatus>,
        enabled: bool,
    ) -> MetadataHandle {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        if !enabled {
            return MetadataHandle { tx };
        }
        let kick = tx.clone();
        tokio::spawn(async move {
            let mut last_finished = scans.borrow().last.as_ref().map(|s| s.finished_at);
            while scans.changed().await.is_ok() {
                let status = scans.borrow().clone();
                let finished = status.last.as_ref().map(|s| s.finished_at);
                if matches!(status.state, ScanState::Idle) && finished != last_finished {
                    last_finished = finished;
                    let _ = kick.try_send(());
                }
            }
        });
        tokio::spawn(async move {
            let http = providers::client();
            while rx.recv().await.is_some() {
                if let Err(e) = run(&db, &store, &http).await {
                    tracing::warn!("metadata lookup stopped: {e}");
                }
            }
        });
        MetadataHandle { tx }
    }

    pub(crate) fn request(&self) {
        let _ = self.tx.try_send(());
    }
}

struct Pending {
    item: ItemId,
    kind: String,
    title: String,
    year: Option<u16>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

async fn run(
    db: &Db,
    store: &Arc<ArtworkStore>,
    http: &reqwest::Client,
) -> Result<(), crate::db::DbError> {
    let tmdb_key: Option<String> = db
        .read(|c| {
            c.query_row(
                "SELECT value FROM setting WHERE key = ?1",
                [TMDB_KEY],
                |r| r.get(0),
            )
            .optional()
        })
        .await?
        .filter(|k: &String| !k.trim().is_empty());
    let pending: Vec<Pending> = db
        .read(|c| {
            c.prepare(
                // Display title from the filename layer only: a previous match
                // must not feed the next search.
                "SELECT i.id, i.kind, f.title, f.year FROM item i \
                 JOIN item_metadata f ON f.item_id = i.id AND f.source = 'filename' \
                 LEFT JOIN metadata_lookup l ON l.item_id = i.id \
                 WHERE i.kind IN ('movie', 'show') AND i.missing_since IS NULL AND l.item_id IS NULL \
                 ORDER BY i.added_at DESC",
            )?
            .query_map([], |r| {
                Ok(Pending {
                    item: ItemId(r.get(0)?),
                    kind: r.get(1)?,
                    title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    year: r.get(3)?,
                })
            })?
            .collect()
        })
        .await?;
    if pending.is_empty() {
        return Ok(());
    }
    tracing::info!(items = pending.len(), "looking up online metadata");
    for p in pending {
        let outcome = match lookup(http, &p, tmdb_key.as_deref()).await {
            Ok(Some((found, tagline))) => {
                let art = download_all(http, store, &found).await;
                let item = p.item;
                let written = db
                    .write(move |tx| store_found(tx, item, &found, tagline.as_deref(), &art))
                    .await;
                match written {
                    Ok(()) => "matched",
                    Err(e) => {
                        tracing::warn!(item = item.0, "metadata write failed: {e}");
                        "error"
                    }
                }
            }
            Ok(None) => "no_match",
            Err(ProviderError::RateLimited) => {
                tracing::info!("metadata provider rate limit hit; resuming next run");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
            Err(e) => {
                tracing::warn!(item = p.item.0, title = %p.title, "metadata lookup failed: {e}");
                "error"
            }
        };
        let item = p.item;
        db.write(move |tx| {
            tx.execute(
                "INSERT INTO metadata_lookup (item_id, looked_at, outcome) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(item_id) DO UPDATE SET looked_at = excluded.looked_at, outcome = excluded.outcome",
                params![item.0, now_ms(), outcome],
            )
            .map(|_| ())
        })
        .await?;
    }
    Ok(())
}

/// Shows: AniList, then TVmaze, over each title variant. Movies: TMDB when a
/// key is set, then AniList's anime films (strict: title and year must agree),
/// then Wikidata and Wikipedia, which need no key.
async fn lookup(
    http: &reqwest::Client,
    p: &Pending,
    tmdb_key: Option<&str>,
) -> Result<Option<(Found, Option<String>)>, ProviderError> {
    let variants = providers::title_variants(&p.title);
    if p.kind == "show" {
        for v in &variants {
            tokio::time::sleep(providers::ANILIST_SPACING).await;
            if let Some(f) = providers::anilist(http, v, false, p.year).await? {
                return Ok(Some((f, None)));
            }
        }
        for v in &variants {
            tokio::time::sleep(providers::TVMAZE_SPACING).await;
            if let Some(f) = providers::tvmaze(http, v).await? {
                return Ok(Some((f, None)));
            }
        }
        return Ok(None);
    }
    if let Some(key) = tmdb_key {
        tokio::time::sleep(providers::TMDB_SPACING).await;
        if let Some(hit) = providers::tmdb_movie(http, key, &p.title, p.year).await? {
            return Ok(Some(hit));
        }
    }
    tokio::time::sleep(providers::ANILIST_SPACING).await;
    if let Some(f) = providers::anilist(http, &p.title, true, p.year).await? {
        return Ok(Some((f, None)));
    }
    tokio::time::sleep(providers::WIKI_SPACING).await;
    Ok(providers::wikidata_movie(http, &p.title, p.year)
        .await?
        .map(|f| (f, None)))
}

/// A rendered image ready to reference from the database.
#[derive(Clone)]
struct Stored {
    id: ArtworkId,
    url: String,
    width: u32,
    height: u32,
    placeholder: Vec<u8>,
}

struct Art {
    poster: Option<Stored>,
    backdrop: Option<Stored>,
    /// Parallel to `found.credits`: (person image, character image).
    credits: Vec<(Option<Stored>, Option<Stored>)>,
}

async fn download_all(http: &reqwest::Client, store: &Arc<ArtworkStore>, found: &Found) -> Art {
    let poster = match &found.poster_url {
        Some(u) => fetch(http, store, u, ArtworkRole::Poster).await,
        None => None,
    };
    let backdrop = match &found.backdrop_url {
        Some(u) => fetch(http, store, u, ArtworkRole::Backdrop).await,
        None => None,
    };
    let jobs: Vec<(Option<String>, Option<String>)> = found
        .credits
        .iter()
        .map(|c| (c.image_url.clone(), c.character_image_url.clone()))
        .collect();
    let (http, store) = (http.clone(), Arc::clone(store));
    let credits = futures::stream::iter(jobs)
        .map(move |(person_url, character_url)| {
            let (http, store) = (http.clone(), Arc::clone(&store));
            async move {
                let person = match person_url {
                    Some(u) => fetch(&http, &store, &u, ArtworkRole::Poster).await,
                    None => None,
                };
                let character = match character_url {
                    Some(u) => fetch(&http, &store, &u, ArtworkRole::Poster).await,
                    None => None,
                };
                (person, character)
            }
        })
        .buffered(6)
        .collect()
        .await;
    Art {
        poster,
        backdrop,
        credits,
    }
}

async fn fetch(
    http: &reqwest::Client,
    store: &Arc<ArtworkStore>,
    url: &str,
    role: ArtworkRole,
) -> Option<Stored> {
    let bytes = match http
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(r) => r.bytes().await.ok()?,
        Err(e) => {
            tracing::debug!(url, "image download failed: {e}");
            return None;
        }
    };
    let store = Arc::clone(store);
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        let (id, art) = scan::render_and_store(&store, role, ArtworkSource::File(&bytes)).ok()?;
        Some(Stored {
            id,
            url,
            width: art.width,
            height: art.height,
            placeholder: art.placeholder.0,
        })
    })
    .await
    .ok()
    .flatten()
}

fn store_found(
    tx: &rusqlite::Transaction<'_>,
    item: ItemId,
    found: &Found,
    tagline: Option<&str>,
    art: &Art,
) -> rusqlite::Result<()> {
    let genres = serde_json::to_string(&found.genres).unwrap_or_else(|_| "[]".into());
    tx.execute(
        "INSERT INTO item_metadata (item_id, source, title, original_title, year, overview, tagline, \
         provider_id, updated_at, genres, rating, studio, external_url) \
         VALUES (?1, 'online', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
         ON CONFLICT(item_id, source) DO UPDATE SET title = excluded.title, original_title = excluded.original_title, \
         year = excluded.year, overview = excluded.overview, tagline = excluded.tagline, provider_id = excluded.provider_id, \
         updated_at = excluded.updated_at, genres = excluded.genres, rating = excluded.rating, studio = excluded.studio, \
         external_url = excluded.external_url",
        params![
            item.0,
            found.title,
            found.original_title,
            found.year,
            found.overview,
            tagline,
            found.provider_id,
            now_ms(),
            genres,
            found.rating,
            found.studio,
            found.external_url
        ],
    )?;
    for (role, stored) in [("poster", &art.poster), ("backdrop", &art.backdrop)] {
        match stored {
            Some(s) => {
                tx.execute(
                    "INSERT INTO artwork (item_id, role, source, source_sig, hash, width, height, placeholder) \
                     VALUES (?1, ?2, 'online', ?3, ?4, ?5, ?6, ?7) \
                     ON CONFLICT(item_id, role, source) DO UPDATE SET source_sig = excluded.source_sig, hash = excluded.hash, \
                     width = excluded.width, height = excluded.height, placeholder = excluded.placeholder",
                    params![item.0, role, s.url, s.id.0.to_vec(), s.width, s.height, s.placeholder],
                )?;
            }
            None => {
                tx.execute(
                    "DELETE FROM artwork WHERE item_id = ?1 AND role = ?2 AND source = 'online'",
                    params![item.0, role],
                )?;
            }
        }
    }
    tx.execute("DELETE FROM item_credit WHERE item_id = ?1", [item.0])?;
    let mut insert = tx.prepare_cached(
        "INSERT INTO item_credit (item_id, ord, kind, name, character, language, image, image_aspect, image_ph, \
         character_image, character_aspect, character_ph) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for (ord, (credit, (person, character))) in found.credits.iter().zip(&art.credits).enumerate() {
        let aspect = |s: &Option<Stored>| {
            s.as_ref()
                .map(|s| f64::from(s.width) / f64::from(s.height.max(1)))
        };
        insert.execute(params![
            item.0,
            ord as i64,
            credit.kind,
            credit.name,
            credit.character,
            credit.language,
            person.as_ref().map(|s| s.id.0.to_vec()),
            aspect(person),
            person.as_ref().map(|s| s.placeholder.clone()),
            character.as_ref().map(|s| s.id.0.to_vec()),
            aspect(character),
            character.as_ref().map(|s| s.placeholder.clone()),
        ])?;
    }
    scan::write::bump_rev(tx)
}
