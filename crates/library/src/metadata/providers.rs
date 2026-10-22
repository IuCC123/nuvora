//! Online metadata providers. Each turns a title (and year, when known) into
//! a [`Found`], or `None` when nothing matches confidently. A wrong match is
//! worse than none, so every provider checks the candidate's titles against
//! the query with [`matches`] before accepting it.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

const USER_AGENT: &str = concat!(
    "Nuvora/",
    env!("CARGO_PKG_VERSION"),
    " (self-hosted media server)"
);

/// What a provider knows about one movie or show.
#[derive(Debug, Clone, Default)]
pub(crate) struct Found {
    /// `"anilist:153406"`, `"tvmaze:41428"`, `"tmdb:movie:10378"`.
    pub provider_id: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    /// 0..10.
    pub rating: Option<f32>,
    pub studio: Option<String>,
    pub external_url: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub credits: Vec<FoundCredit>,
}

#[derive(Debug, Clone)]
pub(crate) struct FoundCredit {
    /// `"cast"` or `"voice"`.
    pub kind: &'static str,
    pub name: String,
    pub character: Option<String>,
    /// For voice actors: which dub, e.g. `"Japanese"`.
    pub language: Option<String>,
    pub image_url: Option<String>,
    pub character_image_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("rate limited")]
    RateLimited,
}

pub(crate) fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("reqwest client builds with static config")
}

/// Lowercase alphanumeric words; ordinals and "season" are normalized away so
/// "Tower of God 2nd Season" and "Tower of God 2" compare equal.
fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in s
        .to_lowercase()
        .replace('&', " and ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let w = raw
            .strip_suffix("st")
            .or_else(|| raw.strip_suffix("nd"))
            .or_else(|| raw.strip_suffix("rd"))
            .or_else(|| raw.strip_suffix("th"))
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(raw);
        if w == "season" || w == "the" {
            continue;
        }
        out.push(w.to_string());
    }
    out
}

/// Share of `query`'s words found in `candidate`.
fn containment(query: &[String], candidate: &[String]) -> f32 {
    if query.is_empty() {
        return 0.0;
    }
    let hits = query.iter().filter(|w| candidate.contains(w)).count();
    hits as f32 / query.len() as f32
}

/// A candidate matches when one of its titles contains every word of the
/// query (or all but one, for long queries), and the query covers at least
/// half of that title, so "Friends" never matches "Friends of the Forest".
pub(crate) fn matches(query: &str, titles: &[&str]) -> bool {
    let q = words(query);
    if q.is_empty() {
        return false;
    }
    let need = if q.len() >= 5 {
        (q.len() - 1) as f32 / q.len() as f32
    } else {
        1.0
    };
    titles.iter().any(|t| {
        let c = words(t);
        !c.is_empty() && containment(&q, &c) >= need && containment(&c, &q) >= 0.5
    })
}

/// The searches to try for a show title, most specific first:
/// "Tower of God 2 - Ouji no Kikan" → itself, "Tower of God 2", "Ouji no Kikan".
pub(crate) fn title_variants(title: &str) -> Vec<String> {
    let mut out = vec![title.trim().to_string()];
    if let Some((head, tail)) = title.split_once(" - ") {
        for part in [head, tail] {
            let part = part.trim();
            if words(part).len() >= 2 && !out.iter().any(|o| o == part) {
                out.push(part.to_string());
            }
        }
    }
    out
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    // AniList appends "(Source: …)" and notes after a blank line; keep the synopsis.
    let mut kept: Vec<&str> = Vec::new();
    for para in out.split("\n\n") {
        let p = para.trim();
        if p.starts_with("(Source") || p.starts_with("Note:") || p.starts_with("Notes:") {
            break;
        }
        if !p.is_empty() {
            kept.push(p);
        }
    }
    kept.join("\n\n").trim().to_string()
}

// ---------------------------------------------------------------- AniList

const ANILIST_QUERY: &str = r#"
query ($search: String, $format: [MediaFormat]) {
  Page(perPage: 5) {
    media(search: $search, type: ANIME, format_in: $format, sort: SEARCH_MATCH) {
      id
      siteUrl
      title { romaji english native }
      synonyms
      format
      seasonYear
      startDate { year }
      averageScore
      genres
      description(asHtml: false)
      coverImage { extraLarge large }
      studios(isMain: true) { nodes { name } }
      characters(sort: [ROLE, RELEVANCE], perPage: 12) {
        edges {
          role
          node { name { full } image { large } }
          voiceActors { name { full } image { large } languageV2 }
        }
      }
    }
  }
}"#;

#[derive(Deserialize)]
struct AniPage {
    data: Option<AniData>,
}
#[derive(Deserialize)]
struct AniData {
    #[serde(rename = "Page")]
    page: AniMediaList,
}
#[derive(Deserialize)]
struct AniMediaList {
    media: Vec<AniMedia>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AniMedia {
    id: u64,
    site_url: Option<String>,
    title: AniTitle,
    #[serde(default)]
    synonyms: Vec<String>,
    season_year: Option<u16>,
    start_date: Option<AniDate>,
    average_score: Option<f32>,
    #[serde(default)]
    genres: Vec<String>,
    description: Option<String>,
    cover_image: Option<AniCover>,
    studios: Option<AniStudios>,
    characters: Option<AniCharacters>,
}
#[derive(Deserialize)]
struct AniTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}
#[derive(Deserialize)]
struct AniDate {
    year: Option<u16>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AniCover {
    extra_large: Option<String>,
    large: Option<String>,
}
#[derive(Deserialize)]
struct AniStudios {
    nodes: Vec<AniName>,
}
#[derive(Deserialize)]
struct AniName {
    name: String,
}
#[derive(Deserialize)]
struct AniCharacters {
    edges: Vec<AniEdge>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AniEdge {
    node: AniPerson,
    #[serde(default)]
    voice_actors: Vec<AniPerson>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AniPerson {
    name: AniFullName,
    image: Option<AniImage>,
    language_v2: Option<String>,
}
#[derive(Deserialize)]
struct AniFullName {
    full: Option<String>,
}
#[derive(Deserialize)]
struct AniImage {
    large: Option<String>,
}

/// AniList allows 30 requests a minute while degraded (90 normally). Callers
/// space requests with [`ANILIST_SPACING`].
pub(crate) const ANILIST_SPACING: Duration = Duration::from_millis(2_100);

pub(crate) async fn anilist(
    http: &reqwest::Client,
    search: &str,
    movie: bool,
    year: Option<u16>,
) -> Result<Option<Found>, ProviderError> {
    let format = if movie {
        json!(["MOVIE"])
    } else {
        json!(["TV", "TV_SHORT", "ONA", "OVA", "SPECIAL"])
    };
    let resp = http
        .post("https://graphql.anilist.co")
        .json(
            &json!({ "query": ANILIST_QUERY, "variables": { "search": search, "format": format } }),
        )
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ProviderError::RateLimited);
    }
    let page: AniPage = resp.error_for_status()?.json().await?;
    let Some(data) = page.data else {
        return Ok(None);
    };
    for m in data.page.media {
        let mut titles: Vec<&str> = [&m.title.english, &m.title.romaji, &m.title.native]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        titles.extend(m.synonyms.iter().map(String::as_str));
        if !matches(search, &titles) {
            continue;
        }
        let found_year = m.season_year.or(m.start_date.as_ref().and_then(|d| d.year));
        if movie
            && let (Some(want), Some(got)) = (year, found_year)
            && want.abs_diff(got) > 1
        {
            continue;
        }
        return Ok(Some(anilist_found(m, found_year)));
    }
    Ok(None)
}

fn anilist_found(m: AniMedia, year: Option<u16>) -> Found {
    let title = m
        .title
        .english
        .clone()
        .or(m.title.romaji.clone())
        .unwrap_or_default();
    let original_title = m.title.romaji.clone().filter(|r| *r != title);
    let mut credits = Vec::new();
    for edge in m.characters.map(|c| c.edges).unwrap_or_default() {
        let character = edge.node.name.full.clone();
        let character_image = edge.node.image.as_ref().and_then(|i| i.large.clone());
        // One request per language returns the same actor for every alias, so
        // fetch all dubs and pick the two people actually look for.
        for language in ["Japanese", "English"] {
            if let Some(va) = edge
                .voice_actors
                .iter()
                .find(|v| v.language_v2.as_deref() == Some(language))
                && let Some(name) = va.name.full.clone()
            {
                credits.push(FoundCredit {
                    kind: "voice",
                    name,
                    character: character.clone(),
                    language: Some(language.to_string()),
                    image_url: va.image.as_ref().and_then(|i| i.large.clone()),
                    character_image_url: character_image.clone(),
                });
            }
        }
    }
    Found {
        provider_id: format!("anilist:{}", m.id),
        title,
        original_title,
        year,
        overview: m
            .description
            .as_deref()
            .map(strip_html)
            .filter(|d| !d.is_empty()),
        genres: m.genres,
        rating: m.average_score.map(|s| s / 10.0),
        studio: m
            .studios
            .and_then(|s| s.nodes.into_iter().next())
            .map(|n| n.name),
        external_url: m.site_url,
        poster_url: m.cover_image.and_then(|c| c.extra_large.or(c.large)),
        backdrop_url: None,
        credits,
    }
}

// ----------------------------------------------------------------- TVmaze

#[derive(Deserialize)]
struct TvShow {
    id: u64,
    url: Option<String>,
    name: String,
    premiered: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    rating: Option<TvRating>,
    network: Option<TvNetwork>,
    #[serde(rename = "webChannel")]
    web_channel: Option<TvNetwork>,
    image: Option<TvImage>,
    #[serde(rename = "_embedded")]
    embedded: Option<TvEmbedded>,
}
#[derive(Deserialize)]
struct TvRating {
    average: Option<f32>,
}
#[derive(Deserialize)]
struct TvNetwork {
    name: String,
}
#[derive(Deserialize)]
struct TvImage {
    original: Option<String>,
    medium: Option<String>,
}
#[derive(Deserialize)]
struct TvEmbedded {
    #[serde(default)]
    cast: Vec<TvCast>,
}
#[derive(Deserialize)]
struct TvCast {
    person: TvPerson,
    character: Option<TvPerson>,
    #[serde(default)]
    voice: bool,
}
#[derive(Deserialize)]
struct TvPerson {
    name: String,
    image: Option<TvImage>,
}

/// TVmaze allows 20 requests per 10 seconds.
pub(crate) const TVMAZE_SPACING: Duration = Duration::from_millis(600);

pub(crate) async fn tvmaze(
    http: &reqwest::Client,
    search: &str,
) -> Result<Option<Found>, ProviderError> {
    let resp = http
        .get("https://api.tvmaze.com/singlesearch/shows")
        .query(&[("q", search), ("embed", "cast")])
        .send()
        .await?;
    match resp.status() {
        reqwest::StatusCode::NOT_FOUND => return Ok(None),
        reqwest::StatusCode::TOO_MANY_REQUESTS => return Err(ProviderError::RateLimited),
        _ => {}
    }
    let show: TvShow = resp.error_for_status()?.json().await?;
    if !matches(search, &[&show.name]) {
        return Ok(None);
    }
    let credits = show
        .embedded
        .map(|e| e.cast)
        .unwrap_or_default()
        .into_iter()
        .take(15)
        .map(|c| FoundCredit {
            kind: if c.voice { "voice" } else { "cast" },
            name: c.person.name,
            character: c.character.as_ref().map(|ch| ch.name.clone()),
            language: None,
            image_url: c.person.image.and_then(|i| i.medium.or(i.original)),
            character_image_url: c
                .character
                .and_then(|ch| ch.image)
                .and_then(|i| i.medium.or(i.original)),
        })
        .collect();
    Ok(Some(Found {
        provider_id: format!("tvmaze:{}", show.id),
        title: show.name,
        original_title: None,
        year: show
            .premiered
            .as_deref()
            .and_then(|p| p.get(..4))
            .and_then(|y| y.parse().ok()),
        overview: show
            .summary
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.is_empty()),
        genres: show.genres,
        rating: show.rating.and_then(|r| r.average),
        studio: show.network.or(show.web_channel).map(|n| n.name),
        external_url: show.url,
        poster_url: show.image.and_then(|i| i.original.or(i.medium)),
        backdrop_url: None,
        credits,
    }))
}

// ------------------------------------------------------------------- TMDB

#[derive(Deserialize)]
struct TmdbSearch {
    results: Vec<TmdbHit>,
}
#[derive(Deserialize)]
struct TmdbHit {
    id: u64,
    title: Option<String>,
    original_title: Option<String>,
    release_date: Option<String>,
}
#[derive(Deserialize)]
struct TmdbMovie {
    id: u64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    tagline: Option<String>,
    #[serde(default)]
    genres: Vec<TmdbGenre>,
    vote_average: Option<f32>,
    vote_count: Option<u32>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    production_companies: Vec<TmdbGenre>,
    credits: Option<TmdbCredits>,
}
#[derive(Deserialize)]
struct TmdbGenre {
    name: String,
}
#[derive(Deserialize)]
struct TmdbCredits {
    #[serde(default)]
    cast: Vec<TmdbCast>,
}
#[derive(Deserialize)]
struct TmdbCast {
    name: String,
    character: Option<String>,
    profile_path: Option<String>,
}

/// TMDB allows about 40 requests per second; a short gap is plenty.
pub(crate) const TMDB_SPACING: Duration = Duration::from_millis(150);

/// A TMDB "API Read Access Token" (long JWT) goes in a Bearer header; a v3
/// API key goes in the query string.
fn tmdb(http: &reqwest::Client, key: &str, url: &str) -> reqwest::RequestBuilder {
    let req = http.get(url);
    if key.len() > 40 {
        req.bearer_auth(key)
    } else {
        req.query(&[("api_key", key)])
    }
}

pub(crate) async fn tmdb_movie(
    http: &reqwest::Client,
    key: &str,
    search: &str,
    year: Option<u16>,
) -> Result<Option<(Found, Option<String>)>, ProviderError> {
    let mut req =
        tmdb(http, key, "https://api.themoviedb.org/3/search/movie").query(&[("query", search)]);
    if let Some(y) = year {
        req = req.query(&[("year", y.to_string())]);
    }
    let resp = req.send().await?;
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ProviderError::RateLimited);
    }
    let hits: TmdbSearch = resp.error_for_status()?.json().await?;
    let Some(hit) = hits.results.into_iter().find(|h| {
        let titles: Vec<&str> = [&h.title, &h.original_title]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        let year_ok = match (
            year,
            h.release_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse::<u16>().ok()),
        ) {
            (Some(want), Some(got)) => want.abs_diff(got) <= 1,
            _ => true,
        };
        year_ok && matches(search, &titles)
    }) else {
        return Ok(None);
    };
    tokio::time::sleep(TMDB_SPACING).await;
    let movie: TmdbMovie = tmdb(
        http,
        key,
        &format!("https://api.themoviedb.org/3/movie/{}", hit.id),
    )
    .query(&[("append_to_response", "credits")])
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;
    let img = |size: &str, path: &Option<String>| {
        path.as_ref()
            .map(|p| format!("https://image.tmdb.org/t/p/{size}{p}"))
    };
    let credits = movie
        .credits
        .map(|c| c.cast)
        .unwrap_or_default()
        .into_iter()
        .take(15)
        .map(|c| FoundCredit {
            kind: "cast",
            name: c.name,
            character: c.character.filter(|s| !s.is_empty()),
            language: None,
            image_url: img("w185", &c.profile_path),
            character_image_url: None,
        })
        .collect();
    let found = Found {
        provider_id: format!("tmdb:movie:{}", movie.id),
        original_title: movie.original_title.filter(|o| *o != movie.title),
        title: movie.title,
        year: movie
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse().ok()),
        overview: movie.overview.filter(|o| !o.is_empty()),
        genres: movie.genres.into_iter().map(|g| g.name).collect(),
        rating: movie
            .vote_average
            .filter(|_| movie.vote_count.unwrap_or(0) >= 10),
        studio: movie
            .production_companies
            .into_iter()
            .next()
            .map(|c| c.name),
        external_url: Some(format!("https://www.themoviedb.org/movie/{}", movie.id)),
        poster_url: img("w780", &movie.poster_path),
        backdrop_url: img("w1280", &movie.backdrop_path),
        credits,
    };
    Ok(Some((found, movie.tagline.filter(|t| !t.is_empty()))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_match_only_when_confident() {
        assert!(matches("Tower of God 2", &["Tower of God Season 2"]));
        assert!(matches(
            "Tower of God 2",
            &["Kami no Tou: Tower of God 2nd Season"]
        ));
        assert!(matches("Big Buck Bunny", &["Big Buck Bunny"]));
        assert!(!matches("Friends", &["Friends of the Forest"]));
        assert!(!matches("Tower of God 2", &["Tower of God"]));
        assert!(!matches("Hero", &["My Hero Academia"]));
        assert!(matches("The Office", &["The Office"]));
    }

    #[test]
    fn show_titles_split_into_searches() {
        assert_eq!(
            title_variants("Tower of God 2 - Ouji no Kikan"),
            vec![
                "Tower of God 2 - Ouji no Kikan",
                "Tower of God 2",
                "Ouji no Kikan"
            ]
        );
        assert_eq!(title_variants("Breaking Bad"), vec!["Breaking Bad"]);
    }

    #[test]
    fn html_and_source_notes_are_stripped() {
        assert_eq!(
            strip_html("The second season.<br><br>\n\n(Source: Crunchyroll)"),
            "The second season."
        );
        assert_eq!(strip_html("<p>A &amp; B</p>"), "A & B");
    }
}
