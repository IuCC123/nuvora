//! Torznab, the search API that Jackett, Prowlarr and many trackers speak:
//! an RSS feed of releases, each with a `.torrent` link or a magnet link and
//! `torznab:attr` extras (seeders, size, info hash).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Torznab's Movies and TV categories; their sub-categories (HD, UHD,
/// Anime, ...) are included.
const MOVIES_AND_TV: &str = "2000,5000";
const TIMEOUT: Duration = Duration::from_secs(20);

/// A Torznab feed the admin added: Jackett's "Torznab Feed" URL, or a
/// Prowlarr indexer's `/<id>/api` URL, and its API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Indexer {
    pub id: u32,
    pub name: String,
    pub url: String,
    pub api_key: String,
}

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub title: String,
    /// The tracker it came from (Jackett's "all" feed names each), else the
    /// indexer's name.
    pub indexer: String,
    /// `magnet:` or an `http(s):` link to a `.torrent` (which may redirect to
    /// a magnet link).
    pub link: String,
    pub size: Option<u64>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    /// Unix seconds.
    pub published: Option<i64>,
    pub details_url: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TorznabError {
    #[error("Couldn't reach it: {0}")]
    Unreachable(String),
    #[error("It answered {0}")]
    Status(u16),
    /// Torznab's own `<error>`: a wrong API key is code 100.
    #[error("It said: {0}")]
    Refused(String),
    #[error("Its answer isn't a Torznab feed")]
    NotTorznab,
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Nuvora/", env!("CARGO_PKG_VERSION")))
        .timeout(TIMEOUT)
        .build()
        .expect("TLS backend initializes")
}

/// Movie and TV releases matching `query`, in the indexer's order.
pub async fn search(
    http: &reqwest::Client,
    indexer: &Indexer,
    query: &str,
) -> Result<Vec<Release>, TorznabError> {
    let xml = get(
        http,
        indexer,
        &[
            ("t", "search"),
            ("q", query),
            ("cat", MOVIES_AND_TV),
            ("extended", "1"),
        ],
    )
    .await?;
    parse_feed(&xml, &indexer.name)
}

/// Asks for the indexer's capabilities: proves the URL and key work.
pub async fn check(http: &reqwest::Client, indexer: &Indexer) -> Result<(), TorznabError> {
    let xml = get(http, indexer, &[("t", "caps")]).await?;
    let doc = roxmltree::Document::parse(&xml).map_err(|_| TorznabError::NotTorznab)?;
    let root = doc.root_element();
    match root.tag_name().name() {
        "caps" => Ok(()),
        "error" => Err(refusal(root)),
        _ => Err(TorznabError::NotTorznab),
    }
}

async fn get(
    http: &reqwest::Client,
    indexer: &Indexer,
    params: &[(&str, &str)],
) -> Result<String, TorznabError> {
    let mut url =
        reqwest::Url::parse(&indexer.url).map_err(|e| TorznabError::Unreachable(e.to_string()))?;
    url.query_pairs_mut()
        .extend_pairs(params)
        .append_pair("apikey", &indexer.api_key);
    // `without_url`: the URL carries the API key, and errors reach logs and the UI.
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| TorznabError::Unreachable(e.without_url().to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| TorznabError::Unreachable(e.without_url().to_string()))?;
    // Jackett answers a bad key with a 4xx that still carries a Torznab error.
    if !status.is_success() {
        return Err(roxmltree::Document::parse(&body)
            .ok()
            .filter(|d| d.root_element().has_tag_name("error"))
            .map_or(TorznabError::Status(status.as_u16()), |d| {
                refusal(d.root_element())
            }));
    }
    Ok(body)
}

fn refusal(error: roxmltree::Node) -> TorznabError {
    let description = error.attribute("description").unwrap_or("an unknown error");
    TorznabError::Refused(description.to_owned())
}

fn parse_feed(xml: &str, indexer: &str) -> Result<Vec<Release>, TorznabError> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| TorznabError::NotTorznab)?;
    let root = doc.root_element();
    if root.has_tag_name("error") {
        return Err(refusal(root));
    }
    if !root.has_tag_name("rss") {
        return Err(TorznabError::NotTorznab);
    }
    Ok(root
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|item| release(item, indexer))
        .collect())
}

/// `None` for an item with no title or nothing to download.
fn release(item: roxmltree::Node, indexer: &str) -> Option<Release> {
    let child = |name: &str| {
        item.children()
            .find(|n| n.tag_name().name() == name)
            .and_then(|n| n.text())
            .map(str::trim)
            .filter(|t| !t.is_empty())
    };
    let attr = |name: &str| {
        item.children()
            .filter(|n| n.tag_name().name() == "attr")
            .find(|n| n.attribute("name") == Some(name))
            .and_then(|n| n.attribute("value"))
    };
    let enclosure = item.children().find(|n| n.tag_name().name() == "enclosure");
    let title = child("title")?.to_owned();
    let link = attr("magneturl")
        .filter(|m| m.starts_with("magnet:"))
        .or_else(|| enclosure.and_then(|e| e.attribute("url")))
        .or_else(|| child("link"))
        .filter(|l| is_downloadable(l))?
        .to_owned();
    let number = |s: Option<&str>| s.and_then(|s| s.trim().parse::<u64>().ok());
    let size = number(child("size"))
        .or_else(|| number(attr("size")))
        .or_else(|| number(enclosure.and_then(|e| e.attribute("length"))))
        .filter(|&s| s > 0);
    let seeders = number(attr("seeders")).map(|s| s.min(u32::MAX.into()) as u32);
    // Torznab's `peers` counts seeders too.
    let leechers = number(attr("leechers"))
        .map(|l| l.min(u32::MAX.into()) as u32)
        .or_else(|| {
            let peers = number(attr("peers"))?.min(u32::MAX.into()) as u32;
            Some(peers.saturating_sub(seeders.unwrap_or(0)))
        });
    let tracker = item
        .children()
        .find(|n| matches!(n.tag_name().name(), "jackettindexer" | "indexer"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty());
    Some(Release {
        title,
        indexer: tracker.unwrap_or(indexer).to_owned(),
        link,
        size,
        seeders,
        leechers,
        published: child("pubDate").and_then(|d| {
            chrono::DateTime::parse_from_rfc2822(d)
                .ok()
                .map(|d| d.timestamp())
        }),
        details_url: child("comments")
            .filter(|c| c.starts_with("http"))
            .map(str::to_owned),
    })
}

pub fn is_downloadable(link: &str) -> bool {
    link.starts_with("magnet:?") || link.starts_with("http://") || link.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:torznab="http://torznab.com/schemas/2015/feed">
  <channel>
    <title>Jackett</title>
    <item>
      <title>Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP</title>
      <guid>https://tracker.example/t/1</guid>
      <jackettindexer id="example">Example Tracker</jackettindexer>
      <comments>https://tracker.example/t/1</comments>
      <pubDate>Mon, 26 Oct 2026 10:00:00 +0000</pubDate>
      <size>8589934592</size>
      <link>http://jackett.local/dl/example/?jackett_apikey=k&amp;path=abc</link>
      <enclosure url="http://jackett.local/dl/example/?jackett_apikey=k&amp;path=abc" length="8589934592" type="application/x-bittorrent" />
      <torznab:attr name="seeders" value="42" />
      <torznab:attr name="peers" value="50" />
      <torznab:attr name="magneturl" value="magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567" />
    </item>
    <item>
      <title>Night of the Living Dead (1968) 720p</title>
      <link>https://tracker.example/t/2.torrent</link>
      <torznab:attr name="seeders" value="3" />
    </item>
    <item>
      <title>No link at all</title>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn feed_items_become_releases() {
        let releases = parse_feed(FEED, "Jackett").unwrap();
        assert_eq!(
            releases,
            vec![
                Release {
                    title: "Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP".into(),
                    indexer: "Example Tracker".into(),
                    link: "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567".into(),
                    size: Some(8_589_934_592),
                    seeders: Some(42),
                    leechers: Some(8),
                    published: Some(1_793_008_800),
                    details_url: Some("https://tracker.example/t/1".into()),
                },
                Release {
                    title: "Night of the Living Dead (1968) 720p".into(),
                    indexer: "Jackett".into(),
                    link: "https://tracker.example/t/2.torrent".into(),
                    size: None,
                    seeders: Some(3),
                    leechers: None,
                    published: None,
                    details_url: None,
                },
            ]
        );
    }

    #[test]
    fn torznab_errors_are_refusals() {
        let error = r#"<?xml version="1.0"?><error code="100" description="Invalid API Key" />"#;
        assert_eq!(
            parse_feed(error, "x"),
            Err(TorznabError::Refused("Invalid API Key".into()))
        );
        assert_eq!(
            parse_feed("<html>login</html>", "x"),
            Err(TorznabError::NotTorznab)
        );
    }
}
