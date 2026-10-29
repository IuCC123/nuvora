//! Subtitles from the internet, for a film or episode whose file has none in
//! the viewer's language, or none this device plays.
//!
//! [Wyzie Subs](https://docs.wyzie.io/subs) searches OpenSubtitles, SubDL,
//! Podnapisi and others at once, by IMDb ID. A free key, from
//! store.wyzie.io/redeem without an account, allows 1,000 requests a day;
//! the admin adds it in Settings. Titles are found by IMDb ID: a film's from
//! Wikidata, a show's from TVmaze. Results that name the same release as the
//! file come first, since they are timed for it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{FileId, Library, QueryError};

const SETTING_KEY: &str = "online_subtitles";
const SEARCH_URL: &str = "https://sub.wyzie.io/search";
const TIMEOUT: Duration = Duration::from_secs(20);
/// Searches are kept this long, so picking a result needs no second search.
const KEEP_SEARCH: Duration = Duration::from_secs(60 * 60);

/// One subtitle file the search found.
#[derive(Debug, Clone, PartialEq)]
pub struct OnlineSubtitle {
    /// Unique within a file's results.
    pub id: String,
    /// ISO 639-1, like `en`.
    pub language: String,
    /// The language's name, like `English`.
    pub label: String,
    /// The release it was made for, like `Interstellar.2014.1080p.BluRay.x264-SPARKS`.
    pub release: Option<String>,
    pub hearing_impaired: bool,
    /// Where it came from, as the service names it.
    pub source: String,
    pub downloads: Option<u64>,
    /// It names the same release as the file (source, resolution and
    /// group), so it is timed for it.
    pub same_release: bool,
    url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OnlineSubtitleError {
    #[error("Add a subtitle search key in Settings › Subtitles to find subtitles online")]
    NoKey,
    #[error("Nuvora doesn't know this title's IMDb number, which the search needs")]
    NoTitle,
    #[error("The subtitle search key isn't valid; check it in Settings › Subtitles")]
    BadKey,
    #[error("Today's subtitle searches are used up; they reset tomorrow")]
    Limit,
    #[error("No such subtitle; search again")]
    NotFound,
    #[error("The subtitle search didn't answer: {0}")]
    Service(String),
    #[error(transparent)]
    Query(#[from] QueryError),
}

impl From<reqwest::Error> for OnlineSubtitleError {
    fn from(e: reqwest::Error) -> Self {
        OnlineSubtitleError::Service(e.without_url().to_string())
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    key: Option<String>,
}

/// What to search for: the title's IMDb ID, the episode, and the file's
/// name to match releases against.
struct Target {
    imdb: String,
    episode: Option<(u32, u32)>,
    file_name: String,
}

impl Library {
    /// Whether a search key is saved. The key itself never leaves the server.
    pub async fn online_subtitles_ready(&self) -> Result<bool, QueryError> {
        Ok(self.saved_subtitle_settings().await?.key.is_some())
    }

    /// `None` or empty removes the key.
    pub async fn set_online_subtitle_key(&self, key: Option<&str>) -> Result<(), QueryError> {
        let key = key
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_owned);
        let json = serde_json::to_string(&Saved { key }).expect("serializes");
        self.set_setting(SETTING_KEY, json).await
    }

    /// Subtitles for `file` in `language` (ISO 639-1), best first: the same
    /// release as the file, then the most downloaded. Machine translations
    /// are left out.
    pub async fn search_online_subtitles(
        &self,
        file: FileId,
        language: &str,
    ) -> Result<Vec<OnlineSubtitle>, OnlineSubtitleError> {
        let language = language.trim().to_ascii_lowercase();
        if let Some(found) = searches().lock().unwrap().get(&(file.0, language.clone()))
            && found.0.elapsed() < KEEP_SEARCH
        {
            return Ok(found.1.clone());
        }
        let key = self
            .saved_subtitle_settings()
            .await?
            .key
            .ok_or(OnlineSubtitleError::NoKey)?;
        let target = self
            .subtitle_target(file)
            .await?
            .ok_or(OnlineSubtitleError::NoTitle)?;
        let mut query = vec![
            ("id", target.imdb.clone()),
            ("language", language.clone()),
            ("format", "srt".to_owned()),
            ("key", key),
        ];
        if let Some((season, episode)) = target.episode {
            query.push(("season", season.to_string()));
            query.push(("episode", episode.to_string()));
        }
        let resp = http().get(SEARCH_URL).query(&query).send().await?;
        let results: Vec<WyzieResult> = match resp.status().as_u16() {
            401 | 403 => return Err(OnlineSubtitleError::BadKey),
            429 | 402 => return Err(OnlineSubtitleError::Limit),
            404 => Vec::new(),
            _ => resp.error_for_status()?.json().await?,
        };
        let found = rank(results, &target.file_name);
        searches()
            .lock()
            .unwrap()
            .insert((file.0, language), (Instant::now(), found.clone()));
        Ok(found)
    }

    /// Result `id` of an earlier search for `file`, as WebVTT.
    pub async fn online_subtitle_vtt(
        &self,
        file: FileId,
        id: &str,
    ) -> Result<String, OnlineSubtitleError> {
        let url = searches()
            .lock()
            .unwrap()
            .iter()
            .filter(|((f, _), _)| *f == file.0)
            .find_map(|(_, (_, found))| found.iter().find(|s| s.id == id).map(|s| s.url.clone()))
            .ok_or(OnlineSubtitleError::NotFound)?;
        let mut url =
            reqwest::Url::parse(&url).map_err(|e| OnlineSubtitleError::Service(e.to_string()))?;
        url.query_pairs_mut().append_pair("to", "vtt");
        let resp = http().get(url).send().await?;
        match resp.status().as_u16() {
            401 | 403 => Err(OnlineSubtitleError::BadKey),
            429 | 402 => Err(OnlineSubtitleError::Limit),
            404 => Err(OnlineSubtitleError::NotFound),
            _ => {
                let text = resp.error_for_status()?.text().await?;
                if text.trim_start().starts_with("WEBVTT") {
                    Ok(text)
                } else {
                    Err(OnlineSubtitleError::Service(
                        "the file wasn't WebVTT".into(),
                    ))
                }
            }
        }
    }

    async fn saved_subtitle_settings(&self) -> Result<Saved, QueryError> {
        Ok(self
            .setting(SETTING_KEY)
            .await?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }

    /// The title's IMDb ID (looked up once per run), and the episode.
    async fn subtitle_target(&self, file: FileId) -> Result<Option<Target>, OnlineSubtitleError> {
        let read = self.inner.db.read(move |c| title_of(c, file)).await;
        let Some(found) = read.map_err(QueryError::from)? else {
            return Ok(None);
        };
        let cached = imdb_ids().lock().unwrap().get(&found.title_item).cloned();
        let imdb = match cached {
            Some(imdb) => Some(imdb),
            None => {
                let imdb = find_imdb(&found).await?;
                if let Some(imdb) = &imdb {
                    imdb_ids()
                        .lock()
                        .unwrap()
                        .insert(found.title_item, imdb.clone());
                }
                imdb
            }
        };
        Ok(imdb.map(|imdb| Target {
            imdb,
            episode: found.episode,
            file_name: found.file_name,
        }))
    }
}

/// The film, or the show an episode belongs to.
struct Title {
    /// The film's or show's item.
    title_item: i64,
    kind: TitleKind,
    title: String,
    year: Option<i64>,
    /// Like `wikidata:Q13417189`, `tvmaze:139`, `anilist:339`.
    providers: Vec<String>,
    /// Season and episode, for an episode.
    episode: Option<(u32, u32)>,
    file_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleKind {
    Film,
    Show,
}

fn title_of(c: &rusqlite::Connection, file: FileId) -> rusqlite::Result<Option<Title>> {
    let Some((item, kind, number, parent, rel)) = c
        .query_row(
            "SELECT i.id, i.kind, i.index_number, i.parent_id, f.rel_path \
             FROM media_file f JOIN item i ON i.id = f.item_id WHERE f.id = ?1",
            [file.0],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let file_name = rel.rsplit('/').next().unwrap_or(&rel).to_owned();
    let (title_item, kind, episode) = match kind.as_str() {
        "movie" => (item, TitleKind::Film, None),
        "episode" => {
            let Some(season_item) = parent else {
                return Ok(None);
            };
            let (season, show) = c.query_row(
                "SELECT index_number, parent_id FROM item WHERE id = ?1",
                [season_item],
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?)),
            )?;
            let (Some(season), Some(show), Some(number)) = (season, show, number) else {
                return Ok(None);
            };
            (show, TitleKind::Show, Some((season as u32, number as u32)))
        }
        _ => return Ok(None),
    };
    let (title, year) = c.query_row(
        "SELECT title, year FROM item_display WHERE id = ?1",
        [title_item],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)),
    )?;
    let providers = c
        .prepare_cached(
            "SELECT provider_id FROM item_metadata WHERE item_id = ?1 AND provider_id IS NOT NULL",
        )?
        .query_map([title_item], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(Title {
        title_item,
        kind,
        title,
        year,
        providers,
        episode,
        file_name,
    }))
}

/// A film's IMDb ID from Wikidata; a show's from TVmaze, by its ID or its name.
async fn find_imdb(title: &Title) -> Result<Option<String>, OnlineSubtitleError> {
    let provider = |prefix: &str| title.providers.iter().find_map(|p| p.strip_prefix(prefix));
    if let Some(qid) = provider("wikidata:") {
        let entity: serde_json::Value = http()
            .get("https://www.wikidata.org/w/api.php")
            .query(&[
                ("action", "wbgetentities"),
                ("ids", qid),
                ("props", "claims"),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let imdb = entity["entities"][qid]["claims"]["P345"][0]["mainsnak"]["datavalue"]["value"]
            .as_str()
            .filter(|id| id.starts_with("tt"));
        if let Some(imdb) = imdb {
            return Ok(Some(imdb.to_owned()));
        }
    }
    if title.kind == TitleKind::Show {
        let show = match provider("tvmaze:") {
            Some(id) => {
                http()
                    .get(format!("https://api.tvmaze.com/shows/{id}"))
                    .send()
                    .await?
            }
            None => {
                http()
                    .get("https://api.tvmaze.com/singlesearch/shows")
                    .query(&[("q", title.title.as_str())])
                    .send()
                    .await?
            }
        };
        if show.status().is_success() {
            let show: serde_json::Value = show.json().await?;
            let year_ok = match (
                title.year,
                show["premiered"]
                    .as_str()
                    .and_then(|d| d.get(..4)?.parse::<i64>().ok()),
            ) {
                (Some(want), Some(got)) => (want - got).abs() <= 1,
                _ => true,
            };
            if let Some(imdb) = show["externals"]["imdb"].as_str().filter(|_| year_ok) {
                return Ok(Some(imdb.to_owned()));
            }
        }
    }
    Ok(None)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WyzieResult {
    id: serde_json::Value,
    url: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    display: Option<String>,
    #[serde(default)]
    release: Option<String>,
    #[serde(default)]
    releases: Vec<String>,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    is_hearing_impaired: bool,
    #[serde(default)]
    source: String,
    #[serde(default)]
    download_count: Option<u64>,
    #[serde(default)]
    ai: bool,
}

/// Machine translations out; the same release first, then hearing-impaired
/// last, then the most downloaded.
fn rank(results: Vec<WyzieResult>, file_name: &str) -> Vec<OnlineSubtitle> {
    let file = release_marks(file_name);
    let mut found: Vec<OnlineSubtitle> = results
        .into_iter()
        .filter(|r| !r.ai)
        .map(|r| {
            let names: Vec<&str> = r
                .release
                .iter()
                .chain(&r.releases)
                .chain(&r.file_name)
                .map(String::as_str)
                .collect();
            let same_release = !file.is_empty() && names.iter().any(|n| release_marks(n) == file);
            OnlineSubtitle {
                id: match &r.id {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                label: r
                    .display
                    .clone()
                    .unwrap_or_else(|| r.language.to_uppercase()),
                language: r.language,
                release: r.release.or(r.file_name),
                hearing_impaired: r.is_hearing_impaired,
                source: r.source,
                downloads: r.download_count,
                same_release,
                url: r.url,
            }
        })
        .collect();
    found.sort_by_key(|s| {
        (
            !s.same_release,
            s.hearing_impaired,
            std::cmp::Reverse(s.downloads.unwrap_or(0)),
        )
    });
    let mut seen = std::collections::HashSet::new();
    found.retain(|s| seen.insert(s.id.clone()));
    found
}

/// What decides a subtitle's timing in a release name: where it was ripped
/// from (cuts differ), the resolution, and the group. Empty when the name
/// says none of it.
fn release_marks(name: &str) -> Vec<String> {
    let lower = name.to_ascii_lowercase();
    let stem = lower
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() <= 4)
        .map_or(lower.as_str(), |(s, _)| s);
    let words: Vec<&str> = stem
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut marks = Vec::new();
    if let Some(source) = words.iter().find_map(|w| match *w {
        "bluray" | "bdrip" | "brrip" | "remux" | "bdremux" => Some("bluray"),
        "webdl" | "webrip" | "web" => Some("web"),
        "hdtv" => Some("hdtv"),
        "dvdrip" | "dvd" => Some("dvd"),
        _ => None,
    }) {
        marks.push(source.to_owned());
    }
    if let Some(resolution) = words
        .iter()
        .find(|w| matches!(**w, "2160p" | "1080p" | "720p" | "480p"))
    {
        marks.push((*resolution).to_owned());
    }
    // The group is the last word after a hyphen: `...x264-SPARKS`.
    if let Some((_, group)) = stem.rsplit_once('-')
        && !group.is_empty()
        && group.chars().all(|c| c.is_ascii_alphanumeric())
    {
        marks.push(group.to_owned());
    }
    marks
}

/// Whether the service takes `key`: a search for a film everyone has
/// (Interstellar) answers.
pub async fn check_online_subtitle_key(key: &str) -> Result<(), OnlineSubtitleError> {
    let resp = http()
        .get(SEARCH_URL)
        .query(&[
            ("id", "tt0816692"),
            ("language", "en"),
            ("format", "srt"),
            ("key", key),
        ])
        .send()
        .await?;
    match resp.status().as_u16() {
        401 | 403 => Err(OnlineSubtitleError::BadKey),
        429 | 402 => Err(OnlineSubtitleError::Limit),
        200 | 404 => Ok(()),
        status => Err(OnlineSubtitleError::Service(format!(
            "it answered {status}"
        ))),
    }
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!(
                "Nuvora/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/IuCC123/nuvora)"
            ))
            .timeout(TIMEOUT)
            .build()
            .expect("HTTP client builds")
    })
}

type Searches = Mutex<HashMap<(i64, String), (Instant, Vec<OnlineSubtitle>)>>;

fn searches() -> &'static Searches {
    static SEARCHES: OnceLock<Searches> = OnceLock::new();
    SEARCHES.get_or_init(Mutex::default)
}

fn imdb_ids() -> &'static Mutex<HashMap<i64, String>> {
    static IDS: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();
    IDS.get_or_init(Mutex::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: &str, release: &str, hi: bool, downloads: u64, ai: bool) -> WyzieResult {
        WyzieResult {
            id: serde_json::Value::String(id.into()),
            url: format!("https://sub.wyzie.io/c/{id}?tok=x"),
            language: "en".into(),
            display: Some("English".into()),
            release: Some(release.into()),
            releases: vec![],
            file_name: None,
            is_hearing_impaired: hi,
            source: "charlie".into(),
            download_count: Some(downloads),
            ai,
        }
    }

    #[test]
    fn the_same_release_comes_first_then_downloads_and_machine_translations_go() {
        let file = "Interstellar.2014.PROPER.2160p.BluRay.REMUX.HEVC.DTS-HD.MA.5.1-FGT.mkv";
        let found = rank(
            vec![
                result(
                    "popular",
                    "Interstellar.2014.1080p.WEB-DL.x264-RARBG",
                    false,
                    90_000,
                    false,
                ),
                result(
                    "same",
                    "Interstellar 2014 2160p BluRay REMUX HEVC-FGT",
                    false,
                    300,
                    false,
                ),
                result(
                    "deaf",
                    "Interstellar.2014.1080p.BluRay.x264-SPARKS",
                    true,
                    50_000,
                    false,
                ),
                result(
                    "bluray",
                    "Interstellar.2014.1080p.BluRay.x264-SPARKS",
                    false,
                    40_000,
                    false,
                ),
                result(
                    "robot",
                    "Interstellar.2014.2160p.BluRay.REMUX-FGT",
                    false,
                    1,
                    true,
                ),
            ],
            file,
        );
        let order: Vec<_> = found
            .iter()
            .map(|s| (s.id.as_str(), s.same_release))
            .collect();
        assert_eq!(
            order,
            [
                ("same", true),
                ("popular", false),
                ("bluray", false),
                ("deaf", false)
            ]
        );
    }

    #[test]
    fn release_names_reduce_to_source_resolution_and_group() {
        assert_eq!(
            release_marks("Heat.1995.1080p.BluRay.x264-AMIABLE.mkv"),
            ["bluray", "1080p", "amiable"]
        );
        assert_eq!(
            release_marks("Heat 1995 1080p WEB-DL DD5.1 H264-FLUX"),
            ["web", "1080p", "flux"]
        );
        assert_eq!(
            release_marks("[Kanavid] Serial Experiments Lain - 05 [BD][1080p][AAC].mp4"),
            ["1080p"]
        );
        assert!(release_marks("movie").is_empty());
    }
}
