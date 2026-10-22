//! Filename knowledge: which paths are movies, which are extras or artwork,
//! what title/year a movie path names, and which show, season and episode an
//! episode path names. Pure functions over relative paths.

use std::path::Path;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

/// Bumped whenever a change in this file can change what a stored path
/// parses to. The scanner re-derives every file stored with an older version,
/// so a parser fix reaches files that were already cataloged.
pub const PARSE_VERSION: u32 = 1;

/// What a path under a library root is, decided by name alone (no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathClass {
    /// A main video file (by extension, not an extra).
    Video,
    /// A sample, trailer, featurette, or anything inside an extras folder
    /// (`Extras/`, `Featurettes/`, `Behind The Scenes/`, `Deleted Scenes/`,
    /// `Trailers/`, `Samples/`) or with a `-sample`/`-trailer`/`.sample` suffix.
    /// Ignored in Step 1; Step 2 may attach extras to items.
    Extra,
    /// A local artwork candidate. `stem` is the lowercase file stem
    /// (`poster`, `folder`, `fanart`, `backdrop`, `the matrix (1999)-poster`, ...).
    Image { stem: String },
    /// Anything else (`.nfo`, `.srt`, `.DS_Store`, ...). Skipped.
    Other,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "webm", "avi", "ts", "m2ts", "mts", "mpg", "mpeg", "wmv", "flv",
    "ogv", "ogm", "3gp", "vob", "divx",
];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Folder names (casefolded) whose contents are extras, per the Plex/Jellyfin convention.
const EXTRAS_FOLDERS: &[&str] = &[
    "extras",
    "extra",
    "featurettes",
    "behind the scenes",
    "deleted scenes",
    "trailers",
    "samples",
    "sample",
    "interviews",
    "scenes",
    "shorts",
    "bonus",
];
const EXTRA_MARKERS: &[&str] = &["sample", "trailer"];
const SYSTEM_FOLDERS: &[&str] = &[
    "@eadir",
    "#recycle",
    "lost+found",
    "$recycle.bin",
    "system volume information",
];

/// Quality, source, codec and tag tokens (casefolded). A token in this table
/// ends the title when it is not the first token.
const RELEASE_TOKENS: &[&str] = &[
    "4k",
    "uhd",
    "8k",
    "bluray",
    "blu-ray",
    "bdrip",
    "brrip",
    "bdremux",
    "remux",
    "web-dl",
    "webdl",
    "webrip",
    "hdtv",
    "dvdrip",
    "hdrip",
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "avc",
    "av1",
    "vp9",
    "xvid",
    "divx",
    "aac",
    "ac3",
    "dts",
    "truehd",
    "atmos",
    "ddp",
    "eac3",
    "flac",
    "hdr",
    "hdr10",
    "hdr10+",
    "dv",
    "dovi",
    "sdr",
    "10bit",
    "8bit",
    "repack",
    "remastered",
    "unrated",
    "imax",
    "amzn",
    "nf",
    "dsnp",
    "atvp",
    "hmax",
];

/// Classify one path relative to its library root.
pub fn classify_path(rel: &Path) -> PathClass {
    let components: Vec<String> = rel
        .iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect();
    let Some((name, folders)) = components.split_last() else {
        return PathClass::Other;
    };
    if is_hidden(name) || folders.iter().any(|f| is_hidden(f)) {
        return PathClass::Other;
    }
    if folders.iter().any(|f| is_extras_folder(f)) {
        return PathClass::Extra;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext.to_ascii_lowercase()),
        _ => return PathClass::Other,
    };
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        if is_extra_stem(stem) {
            PathClass::Extra
        } else {
            PathClass::Video
        }
    } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        PathClass::Image {
            stem: stem.to_lowercase(),
        }
    } else {
        PathClass::Other
    }
}

/// True for a directory the walk should not descend: hidden or system folders
/// (nothing useful inside) and extras folders (contents are never main videos).
pub fn is_skipped_dir(rel: &Path) -> bool {
    rel.file_name().is_some_and(|n| {
        let n = n.to_string_lossy();
        is_hidden(&n) || is_extras_folder(&n)
    })
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.') || SYSTEM_FOLDERS.contains(&name.to_lowercase().as_str())
}

fn is_extras_folder(name: &str) -> bool {
    EXTRAS_FOLDERS.contains(&name.to_lowercase().as_str())
}

/// Title and year parsed from a movie's path.
///
/// Rules, in order:
/// 1. If the parent folder looks like `Title (Year)` (optionally `[...]` tags),
///    use it: folder-per-movie is the convention Plex users follow.
/// 2. Else parse the file stem: split on `.`/`_`/space, take tokens up to the
///    first year in 1900..=current+1 that is not the first token (so `1917.mkv`
///    and `2001 A Space Odyssey (1968)` work), drop quality/source/codec and
///    bitrate tokens.
/// 3. No year found: the whole cleaned stem is the title, year = None.
/// 4. A one-word CamelCase title is split into words (`BigBuckBunny`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieName {
    pub title: String,
    /// Title with a leading article (`The`, `A`, `An`) moved to the end for sorting.
    pub sort_title: String,
    pub year: Option<u16>,
}

/// Grouping key for "these files are the same movie", stored as
/// `item.identity_key`. Only constructible by [`MovieName::identity_key`], so
/// every writer normalizes identically. Built from the filename alone and never
/// replaced by a provider id: a later re-probe must re-derive the same key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityKey(String);

impl IdentityKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl MovieName {
    pub fn parse(rel: &Path) -> MovieName {
        let folder = rel
            .parent()
            .and_then(Path::file_name)
            .map(|f| f.to_string_lossy());
        if let Some((title, year)) = folder.as_deref().and_then(folder_title_year) {
            return MovieName::new(title, Some(year));
        }
        let stem = rel
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let (title, year) = stem_title_year(&stem);
        MovieName::new(title, year)
    }

    fn new(title: String, year: Option<u16>) -> MovieName {
        let sort_title = sort_title(&title);
        MovieName {
            title,
            sort_title,
            year,
        }
    }

    /// `"<casefolded alnum title>|<year or ''>"`. Diacritics folded, `&` -> `and`.
    pub fn identity_key(&self) -> IdentityKey {
        IdentityKey(title_key(&self.title, self.year))
    }
}

/// `title` lowercased with diacritics folded (`Amélie` -> `amelie`), for
/// sorting and for grouping by first letter: `Ñ` files under N, not after Z.
pub fn fold_for_sort(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars() {
        match fold_diacritic(c) {
            Some(folded) => out.push_str(folded),
            None => out.extend(c.to_lowercase()),
        }
    }
    out
}

fn title_key(title: &str, year: Option<u16>) -> String {
    let mut key = String::with_capacity(title.len() + 5);
    for c in title.chars() {
        if c == '&' {
            key.push_str("and");
        } else if let Some(folded) = fold_diacritic(c) {
            key.push_str(folded);
        } else if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        }
    }
    key.push('|');
    if let Some(year) = year {
        key.push_str(&year.to_string());
    }
    key
}

/// Show, season and episode parsed from an episode's path. `None` from
/// [`EpisodeName::parse`] means the name carries no episode number.
///
/// Before matching, every `[...]` segment (release group, resolution, CRC)
/// and every `(...)` segment without a year is dropped. Episode patterns, in
/// priority order:
/// 1. `S01E02`, `s1e2`, multi-episode `S01E02E03` / `S01E02-E03` (the first
///    episode counts).
/// 2. `1x02`.
/// 3. `Episode 2`, `Ep 2` or `E02` inside a `Season N` folder.
/// 4. The anime absolute number: the last ` - 02` (or `- 02v2`) with 1 to 4
///    digits, at the end of the name or before release tokens.
/// 5. `Ep02` or `Episode 02` anywhere.
///
/// Without a season in the token, the season comes from a `Season N`/`S1`
/// folder, else 1. `S00` or a `Specials` folder is season 0.
///
/// The show is the first folder under the library root that is not a season
/// folder, when there is one, so episodes named slightly differently group
/// together. Otherwise it is the text before the episode token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeName {
    pub show: ShowName,
    pub season: u32,
    pub episode: u32,
    /// The text after the episode token (`S01E02 - The Title`), if any.
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowName {
    pub title: String,
    /// Title with a leading article moved to the end, for sorting.
    pub sort_title: String,
    pub year: Option<u16>,
}

impl EpisodeName {
    pub fn parse(rel: &Path) -> Option<EpisodeName> {
        let parts: Vec<String> = rel
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .collect();
        let (file, folders) = parts.split_last()?;
        let stem = file
            .rsplit_once('.')
            .map_or(file.as_str(), |(stem, _)| stem);
        let folder_season = folders.last().and_then(|f| season_folder(f));
        let show_folder = folders.iter().find(|f| season_folder(f).is_none());

        let clean = strip_tags(stem);
        let found = find_episode(&clean, folder_season.is_some())?;
        let season = found.season.or(folder_season).unwrap_or(1);
        let show = match show_folder {
            Some(folder) => show_name(&strip_tags(folder)),
            None => show_name(&clean[..found.start]),
        }?;
        Some(EpisodeName {
            show,
            season,
            episode: found.episode,
            title: episode_title(&clean[found.end..]),
        })
    }

    /// `"show:<casefolded alnum title>|<year or ''>"`, unique within a library.
    pub fn show_key(&self) -> IdentityKey {
        IdentityKey(format!(
            "show:{}",
            title_key(&self.show.title, self.show.year)
        ))
    }

    /// The show key plus `|s<season>`.
    pub fn season_key(&self) -> IdentityKey {
        IdentityKey(format!("{}|s{}", self.show_key().0, self.season))
    }

    /// The season key plus `|e<episode>`.
    pub fn episode_key(&self) -> IdentityKey {
        IdentityKey(format!("{}|e{}", self.season_key().0, self.episode))
    }
}

/// Where the episode token sits in the cleaned stem, and what it says.
struct Found {
    start: usize,
    end: usize,
    season: Option<u32>,
    episode: u32,
}

static SXXEXX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^a-z0-9])(s(\d{1,3})[ ._-]?e(\d{1,4})(?:[ ._]?-?[ ._]?e\d{1,4})*)")
        .expect("valid")
});
static CROSS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^a-z0-9])((\d{1,2})x(\d{2,3}))(?:[^a-z0-9]|$)").expect("valid")
});
static IN_SEASON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^a-z0-9])((?:episode|ep|e)[ ._]?(\d{1,4}))(?:[^a-z0-9]|$)")
        .expect("valid")
});
static ABSOLUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s-\s*((\d{1,4})(?:v\d{1,2})?)(?:\s|$)").expect("valid"));
static EPISODE_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^a-z0-9])((?:episode|ep)[ ._]?(\d{1,4}))(?:[^a-z0-9]|$)").expect("valid")
});

fn find_episode(stem: &str, in_season_folder: bool) -> Option<Found> {
    let numbered = |re: &Regex, season_group: Option<usize>, episode_group: usize| {
        re.captures_iter(stem).find_map(|c| {
            let token = c.get(1)?;
            Some(Found {
                start: token.start(),
                end: token.end(),
                season: season_group.and_then(|g| c.get(g)?.as_str().parse().ok()),
                episode: c.get(episode_group)?.as_str().parse().ok()?,
            })
        })
    };
    numbered(&SXXEXX, Some(2), 3)
        .or_else(|| numbered(&CROSS, Some(2), 3))
        .or_else(|| {
            in_season_folder
                .then(|| numbered(&IN_SEASON, None, 2))
                .flatten()
        })
        .or_else(|| absolute_number(stem))
        .or_else(|| numbered(&EPISODE_WORD, None, 2))
}

/// The last ` - NN` whose remainder is empty or release tokens only. A
/// four-digit number that reads as a year is a year, not an episode.
fn absolute_number(stem: &str) -> Option<Found> {
    let last = ABSOLUTE.captures_iter(stem).filter_map(|c| {
        let token = c.get(1)?;
        let digits = c.get(2)?.as_str();
        let rest = &stem[token.end()..];
        let only_tags = tokens(rest).iter().all(|t| is_release_token(t));
        let year = digits.len() == 4 && year_of(digits).is_some();
        let episode = digits.parse().ok()?;
        (only_tags && !year).then(|| Found {
            start: c.get(0).map_or(token.start(), |m| m.start()),
            end: token.end(),
            season: None,
            episode,
        })
    });
    last.last()
}

/// `Season 1`, `Season 01`, `Series 2`, `S1`, `S01` give the number;
/// `Specials` (and `Season 0`) give 0.
fn season_folder(name: &str) -> Option<u32> {
    static SEASON: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(?:season|series|s)[ ._-]*(\d{1,3})\s*$").expect("valid")
    });
    if name.trim().eq_ignore_ascii_case("specials") || name.trim().eq_ignore_ascii_case("special") {
        return Some(0);
    }
    SEASON.captures(name)?.get(1)?.as_str().parse().ok()
}

/// Drops every `[...]` segment and every `(...)` segment that holds no year,
/// then collapses whitespace.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(['[', '(']) {
        out.push_str(&rest[..open]);
        let close = if rest[open..].starts_with('[') {
            ']'
        } else {
            ')'
        };
        let Some(len) = rest[open..].find(close) else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let inner = &rest[open + 1..open + len];
        if close == ')' && year_of(inner.trim()).is_some() {
            out.push_str(&rest[open..=open + len]);
        }
        out.push(' ');
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A show title and year from a folder name or the text before the episode
/// token: scene dots become spaces, the title ends at the first year or
/// release token, and inner hyphens are kept (`Tower of God 2 - Ouji no
/// Kikan`). `None` when nothing is left.
fn show_name(text: &str) -> Option<ShowName> {
    let text = scene_spaces(text);
    let mut title_words = Vec::new();
    let mut year = None;
    for (i, word) in text.split(' ').enumerate() {
        let bare = word.trim_matches(|c| c == '(' || c == ')');
        if i > 0
            && let Some(y) = year_of(bare)
        {
            year = Some(y);
            break;
        }
        if i > 0 && is_release_token(bare) {
            break;
        }
        title_words.push(word);
    }
    let title = title_words
        .join(" ")
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '.' | ':'))
        .to_owned();
    if title.is_empty() {
        return None;
    }
    Some(ShowName {
        sort_title: sort_title(&title),
        title,
        year,
    })
}

/// The text after the episode token, without separators and release tokens.
fn episode_title(text: &str) -> Option<String> {
    let text = scene_spaces(text);
    let words: Vec<&str> = text
        .split(' ')
        .take_while(|w| !is_release_token(w))
        .collect();
    let title = words
        .join(" ")
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '.' | ':'))
        .to_owned();
    (!title.is_empty()).then_some(title)
}

/// `The.Office.US` reads as `The Office US`: dots and underscores are word
/// breaks unless the text already has spaces (then only underscores are).
fn scene_spaces(text: &str) -> String {
    let spaced = if text.contains(' ') {
        text.replace('_', " ")
    } else {
        text.replace(['.', '_'], " ")
    };
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Title (Year)` or `Title [Year]`, with anything after the year ignored.
fn folder_title_year(folder: &str) -> Option<(String, u16)> {
    let mut search = 0;
    while let Some(open) = folder[search..].find(['(', '[']) {
        let start = search + open;
        let closed = matches!(folder.get(start + 5..start + 6), Some(")") | Some("]"));
        let year = folder.get(start + 1..start + 5).and_then(year_of);
        if let (true, Some(year)) = (closed, year) {
            let title = folder[..start].trim().trim_end_matches('-').trim();
            if !title.is_empty() {
                return Some((title.to_owned(), year));
            }
        }
        search = start + 1;
    }
    None
}

fn stem_title_year(stem: &str) -> (String, Option<u16>) {
    let stem = strip_bracket_tags(stem);
    let tokens = tokens(&stem);
    let end = tokens
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, t)| year_of(t).is_some() || is_release_token(t));
    match end {
        Some((i, _)) => {
            let year = tokens[i..].iter().find_map(|t| year_of(t));
            (title_of(&tokens[..i]), year)
        }
        None if tokens.is_empty() => (stem.trim().to_owned(), None),
        None => (title_of(&tokens), None),
    }
}

/// Drops `[...]` segments that hold no year (`[Group]`, `[1080p]`, CRCs);
/// `[2001]` stays, since a movie's year is often written that way.
fn strip_bracket_tags(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut rest = stem;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let Some(len) = rest[open..].find(']') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &rest[open + 1..open + len];
        if year_of(inner.trim()).is_some() {
            out.push_str(&rest[open..=open + len]);
        } else {
            out.push(' ');
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out
}

/// A one-word title written in CamelCase (`BigBuckBunny`) is split into its
/// words. Only when every word is a capital and two or more lowercase letters,
/// so `iPhone`, `McDonald` and `WALL-E` stay whole.
fn title_of(tokens: &[&str]) -> String {
    let [word] = tokens else {
        return tokens.join(" ");
    };
    let starts: Vec<usize> = word
        .char_indices()
        .filter(|(_, c)| c.is_uppercase())
        .map(|(i, _)| i)
        .collect();
    let ends = starts.iter().skip(1).copied().chain([word.len()]);
    let humps: Vec<&str> = starts.iter().zip(ends).map(|(&a, b)| &word[a..b]).collect();
    let camel = starts.first() == Some(&0)
        && humps.len() >= 2
        && humps.iter().all(|hump| {
            let rest = hump.chars().skip(1);
            rest.clone().count() >= 2 && rest.clone().all(char::is_lowercase)
        });
    if camel {
        humps.join(" ")
    } else {
        (*word).to_owned()
    }
}

/// Split a stem into words. Brackets are separators (so `[2001]` yields `2001`)
/// and a bare dash is dropped (so `Alien (1979) - 4K` yields `Alien 1979 4K`).
fn tokens(stem: &str) -> Vec<&str> {
    stem.split(|c: char| {
        c.is_whitespace() || matches!(c, '.' | '_' | '(' | ')' | '[' | ']' | '{' | '}')
    })
    .filter(|t| !t.is_empty() && !t.chars().all(|c| matches!(c, '-' | '–' | '—')))
    .collect()
}

/// A sample or trailer marker counts only after the title has ended: as the
/// last token, after a year or release token, as a `-suffix`, or alone. A
/// `Sample` inside the title (`Sintel.Sample.2010...`) is part of the title.
fn is_extra_stem(stem: &str) -> bool {
    let tokens = tokens(stem);
    let mut title_ended = false;
    for (i, token) in tokens.iter().enumerate() {
        let parts: Vec<&str> = token.split('-').collect();
        let last_part = parts
            .last()
            .map(|p| p.to_ascii_lowercase())
            .unwrap_or_default();
        if EXTRA_MARKERS.contains(&last_part.as_str()) {
            let is_last = i + 1 == tokens.len();
            if is_last || title_ended || parts.len() > 1 {
                return true;
            }
        }
        if i > 0 && (year_of(token).is_some() || is_release_token(token)) {
            title_ended = true;
        }
    }
    false
}

fn year_of(token: &str) -> Option<u16> {
    if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: u16 = token.parse().ok()?;
    (1900..=current_year() + 1).contains(&year).then_some(year)
}

fn current_year() -> u16 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Mean Gregorian year; exact enough for a plausibility bound on a filename.
    1970 + (secs / 31_556_952) as u16
}

fn is_release_token(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    std::iter::once(lower.as_str())
        .chain(lower.split('-'))
        .any(|part| RELEASE_TOKENS.contains(&part) || is_resolution(part) || is_bitrate(part))
}

/// `512kb`, `1024kbps`, `800k`.
fn is_bitrate(part: &str) -> bool {
    let Some(digits) = ["kbps", "kb", "k"]
        .iter()
        .find_map(|unit| part.strip_suffix(unit))
    else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `480p`, `720p`, `1080i`, `2160p`.
fn is_resolution(part: &str) -> bool {
    let Some(digits) = part.strip_suffix(['p', 'i']) else {
        return false;
    };
    (3..=4).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

fn sort_title(title: &str) -> String {
    for article in ["The ", "A ", "An "] {
        let Some((head, rest)) = title.get(..article.len()).zip(title.get(article.len()..)) else {
            continue;
        };
        if !rest.is_empty() && head.eq_ignore_ascii_case(article) {
            return format!("{}, {}", rest.trim_start(), head.trim_end());
        }
    }
    title.to_owned()
}

const DIACRITIC_FOLDS: &[(&str, &str)] = &[
    ("ÀÁÂÃÄÅàáâãäåĀāĂăĄą", "a"),
    ("Ææ", "ae"),
    ("ÇçĆćĈĉĊċČč", "c"),
    ("ÐðĎďĐđ", "d"),
    ("ÈÉÊËèéêëĒēĔĕĖėĘęĚě", "e"),
    ("ĜĝĞğĠġĢģ", "g"),
    ("ĤĥĦħ", "h"),
    ("ÌÍÎÏìíîïĨĩĪīĬĭĮįİı", "i"),
    ("Ĳĳ", "ij"),
    ("Ĵĵ", "j"),
    ("Ķķĸ", "k"),
    ("ĹĺĻļĽľĿŀŁł", "l"),
    ("ÑñŃńŅņŇňŉŊŋ", "n"),
    ("ÒÓÔÕÖØòóôõöøŌōŎŏŐő", "o"),
    ("Œœ", "oe"),
    ("ŔŕŖŗŘř", "r"),
    ("ŚśŜŝŞşŠšſ", "s"),
    ("ß", "ss"),
    ("ŢţŤťŦŧ", "t"),
    ("Þþ", "th"),
    ("ÙÚÛÜùúûüŨũŪūŬŭŮůŰűŲų", "u"),
    ("Ŵŵ", "w"),
    ("ÝýÿŶŷŸ", "y"),
    ("ŹźŻżŽž", "z"),
];

fn fold_diacritic(c: char) -> Option<&'static str> {
    if c.is_ascii() {
        return None;
    }
    DIACRITIC_FOLDS
        .iter()
        .find(|(chars, _)| chars.contains(c))
        .map(|(_, to)| *to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (relative path, expected title, expected year). The table is the spec;
    /// every bug report about a misparsed name becomes a row here.
    const CASES: &[(&str, &str, Option<u16>)] = &[
        (
            "The Matrix (1999)/The Matrix (1999).mkv",
            "The Matrix",
            Some(1999),
        ),
        (
            "The.Matrix.1999.1080p.BluRay.x264-GRP.mkv",
            "The Matrix",
            Some(1999),
        ),
        (
            "Blade Runner 2049 (2017)/Blade.Runner.2049.2160p.UHD.mkv",
            "Blade Runner 2049",
            Some(2017),
        ),
        ("1917 (2019)/1917.mkv", "1917", Some(2019)),
        (
            "2001.A.Space.Odyssey.1968.mkv",
            "2001 A Space Odyssey",
            Some(1968),
        ),
        ("Movies/Amélie [2001] [1080p].mp4", "Amélie", Some(2001)),
        ("Heat.mkv", "Heat", None),
        ("Alien (1979)/Alien (1979) - 4K.mkv", "Alien", Some(1979)),
        (
            "Big Buck Test (2008)/Big Buck Test (2008).mp4",
            "Big Buck Test",
            Some(2008),
        ),
        (
            "Sintel Sample (2010)/Sintel.Sample.2010.1080p.BluRay.x264-GRP.mkv",
            "Sintel Sample",
            Some(2010),
        ),
        (
            "Elephants Dream (2006)/Elephants.Dream.2006.mp4",
            "Elephants Dream",
            Some(2006),
        ),
        (
            "Cosmos Laundromat (2015)/Cosmos Laundromat (2015) - 2160p.mp4",
            "Cosmos Laundromat",
            Some(2015),
        ),
        (
            "Loose Files/The.Spring.2019.720p.WEB-DL.mp4",
            "The Spring",
            Some(2019),
        ),
        (
            "Tears of Steel (2012)/Tears of Steel (2012).webm",
            "Tears of Steel",
            Some(2012),
        ),
        ("Blade Runner 2049.mkv", "Blade Runner 2049", None),
        ("Spider-Man.2002.1080p.mkv", "Spider-Man", Some(2002)),
        ("Heat.1080p.BluRay.x264.mkv", "Heat", None),
        ("BigBuckBunny_512kb.mp4", "Big Buck Bunny", None),
        ("ElephantsDream.1024kbps.mp4", "Elephants Dream", None),
        ("Sintel.2010.800k.mp4", "Sintel", Some(2010)),
        ("iPhone.mkv", "iPhone", None),
        ("McDonald.mkv", "McDonald", None),
        ("WALL-E.2008.mkv", "WALL-E", Some(2008)),
        ("WALL-E.mkv", "WALL-E", None),
        ("The.Matrix.1999.1080p.mkv", "The Matrix", Some(1999)),
        (
            "[Group] Some Movie (2020) [1080p].mkv",
            "Some Movie",
            Some(2020),
        ),
        (
            "[Ñ] Tower of God 2 - Ouji no Kikan - 01 [1080p] [080DD9EA].mkv",
            "Tower of God 2 Ouji no Kikan 01",
            None,
        ),
    ];

    #[test]
    fn parses_titles_and_years() {
        for (path, title, year) in CASES {
            let n = MovieName::parse(Path::new(path));
            assert_eq!((n.title.as_str(), n.year), (*title, *year), "{path}");
        }
    }

    const TOWER: &str = "Tower of God 2 - Ouji no Kikan";

    /// (relative path, show, show year, season, episode, episode title).
    type EpisodeRow = (
        &'static str,
        &'static str,
        Option<u16>,
        u32,
        u32,
        Option<&'static str>,
    );

    /// Every pattern of the episode spec has a row; every misparsed episode
    /// name reported later becomes one.
    const EPISODES: &[EpisodeRow] = &[
        (
            "[Ñ] Tower of God 2 - Ouji no Kikan [1080p]/[Ñ] Tower of God 2 - Ouji no Kikan - 01 [1080p] [080DD9EA].mkv",
            TOWER,
            None,
            1,
            1,
            None,
        ),
        (
            "[Ñ] Tower of God 2 - Ouji no Kikan - 13 [1080p] [B8B1B2BC].mkv",
            TOWER,
            None,
            1,
            13,
            None,
        ),
        (
            "[SubsPlease] Frieren - 02v2 (1080p) [ABCD1234].mkv",
            "Frieren",
            None,
            1,
            2,
            None,
        ),
        (
            "Frieren/Season 2/Frieren - 03.mkv",
            "Frieren",
            None,
            2,
            3,
            None,
        ),
        (
            "One Piece - 1089 [1080p].mkv",
            "One Piece",
            None,
            1,
            1089,
            None,
        ),
        (
            "The.Office.US.S02E03.720p.HDTV.x264-GRP.mkv",
            "The Office US",
            None,
            2,
            3,
            None,
        ),
        (
            "Breaking.Bad.S01E02.Cats.in.the.Bag.1080p.BluRay.x264.mkv",
            "Breaking Bad",
            None,
            1,
            2,
            Some("Cats in the Bag"),
        ),
        ("Show Name s1e2.mkv", "Show Name", None, 1, 2, None),
        (
            "Show Name - S01E02 - The Title.mkv",
            "Show Name",
            None,
            1,
            2,
            Some("The Title"),
        ),
        (
            "Show Name - S01E02E03 - Double.mkv",
            "Show Name",
            None,
            1,
            2,
            Some("Double"),
        ),
        ("Show.Name.S01E02-E03.mkv", "Show Name", None, 1, 2, None),
        (
            "Show Name - 1x02 - The Title.mkv",
            "Show Name",
            None,
            1,
            2,
            Some("The Title"),
        ),
        (
            "Show Name/Season 2/Episode 5.mkv",
            "Show Name",
            None,
            2,
            5,
            None,
        ),
        (
            "Show Name/Season 02/E07 - The Title.mkv",
            "Show Name",
            None,
            2,
            7,
            Some("The Title"),
        ),
        (
            "Show Name (2019)/Season 1/Show Name - S01E04.mkv",
            "Show Name",
            Some(2019),
            1,
            4,
            None,
        ),
        (
            "Doctor Who (2005)/Season 1/doctor.who.2005.s01e01.rose.mkv",
            "Doctor Who",
            Some(2005),
            1,
            1,
            Some("rose"),
        ),
        (
            "Show Name/S3/Show Name 3x04.mkv",
            "Show Name",
            None,
            3,
            4,
            None,
        ),
        (
            "Show Name/Specials/Show Name - S00E01 - Pilot.mkv",
            "Show Name",
            None,
            0,
            1,
            Some("Pilot"),
        ),
        (
            "Show Name/Specials/Episode 2.mkv",
            "Show Name",
            None,
            0,
            2,
            None,
        ),
        (
            "Show Name/Show Name S00E05.mkv",
            "Show Name",
            None,
            0,
            5,
            None,
        ),
        ("Show Name Ep02.mkv", "Show Name", None, 1, 2, None),
        (
            "Show Name Episode 12 - Finale.mkv",
            "Show Name",
            None,
            1,
            12,
            Some("Finale"),
        ),
        (
            "Mr. Robot/Season 1/Mr. Robot - S01E01 - eps1.0.mkv",
            "Mr. Robot",
            None,
            1,
            1,
            Some("eps1.0"),
        ),
    ];

    #[test]
    fn parses_episodes() {
        for (path, show, year, season, episode, title) in EPISODES {
            let n = EpisodeName::parse(Path::new(path)).unwrap_or_else(|| panic!("{path}"));
            assert_eq!(
                (
                    n.show.title.as_str(),
                    n.show.year,
                    n.season,
                    n.episode,
                    n.title.as_deref()
                ),
                (*show, *year, *season, *episode, *title),
                "{path}"
            );
        }
    }

    #[test]
    fn the_owners_release_is_one_show_with_thirteen_episodes() {
        let crcs = [
            "080DD9EA", "C6F9D246", "D982D152", "48791899", "297EA1D8", "5537BCD7", "F6F792C3",
            "BEC05FB0", "AD306291", "D725D91F", "29C0F0E6", "DB5520A3", "B8B1B2BC",
        ];
        let folder = "[Ñ] Tower of God 2 - Ouji no Kikan [1080p]";
        let parsed: Vec<EpisodeName> = crcs
            .iter()
            .enumerate()
            .map(|(i, crc)| {
                let path = format!(
                    "{folder}/[Ñ] Tower of God 2 - Ouji no Kikan - {:02} [1080p] [{crc}].mkv",
                    i + 1
                );
                EpisodeName::parse(Path::new(&path)).unwrap()
            })
            .collect();
        for (i, n) in parsed.iter().enumerate() {
            assert_eq!(
                (n.show.title.as_str(), n.season, n.episode),
                (TOWER, 1, i as u32 + 1)
            );
            assert_eq!(n.show_key(), parsed[0].show_key());
        }
        assert_eq!(
            parsed[0].show_key().as_str(),
            "show:towerofgod2oujinokikan|"
        );
        assert_eq!(
            parsed[4].episode_key().as_str(),
            "show:towerofgod2oujinokikan||s1|e5"
        );
    }

    #[test]
    fn names_without_an_episode_number_are_not_episodes() {
        for path in [
            "Heat (1995)/Heat (1995).mkv",
            "BigBuckBunny_512kb.mp4",
            "Show - 2019.mkv",
            "Movie.1920x1080.mkv",
            "The.Matrix.1999.1080p.BluRay.x264-GRP.mkv",
            "Season 1/S01E02.mkv",
        ] {
            assert_eq!(EpisodeName::parse(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn sort_title_moves_the_article_to_the_end() {
        let cases = [
            ("The Matrix", "Matrix, The"),
            ("A Few Good Men", "Few Good Men, A"),
            ("Heat", "Heat"),
            ("Alien", "Alien"),
        ];
        for (title, want) in cases {
            assert_eq!(MovieName::new(title.to_owned(), None).sort_title, want);
        }
    }

    #[test]
    fn versions_of_one_movie_share_an_identity_key() {
        let a = MovieName::parse(Path::new("Alien (1979)/Alien (1979) - 1080p.mkv"));
        let b = MovieName::parse(Path::new("Alien.1979.2160p.UHD.HDR.mkv"));
        assert_eq!(a.identity_key(), b.identity_key());
        assert_eq!(a.identity_key().as_str(), "alien|1979");
    }

    #[test]
    fn identity_key_folds_case_diacritics_and_punctuation() {
        let key = |t: &str, y: Option<u16>| {
            MovieName::new(t.to_owned(), y)
                .identity_key()
                .as_str()
                .to_owned()
        };
        assert_eq!(key("Amélie", Some(2001)), "amelie|2001");
        assert_eq!(key("Fast & Furious", None), "fastandfurious|");
        assert_eq!(key("Spider-Man", Some(2002)), key("Spider Man", Some(2002)));
        assert_eq!(key("WALL·E", Some(2008)), "walle|2008");
    }

    const CLASSES: &[(&str, &str)] = &[
        ("The Matrix (1999)/The Matrix (1999).mkv", "video"),
        ("The Matrix (1999)/Extras/Making Of.mkv", "extra"),
        ("The Matrix (1999)/the.matrix.sample.mkv", "extra"),
        ("The Matrix (1999)/The Matrix-trailer.mp4", "extra"),
        ("The Matrix (1999)/poster.jpg", "image"),
        ("The Matrix (1999)/movie.nfo", "other"),
        (".DS_Store", "other"),
        (
            "Sintel Sample (2010)/Sintel.Sample.2010.1080p.BluRay.x264-GRP.mkv",
            "video",
        ),
        (
            "Elephants Dream (2006)/Elephants.Dream.2006.sample.mp4",
            "extra",
        ),
        ("Elephants Dream (2006)/Elephants.Dream.2006.mp4", "video"),
        (
            "Cosmos Laundromat (2015)/Cosmos Laundromat (2015) - 2160p.mp4",
            "video",
        ),
        ("Loose Files/The.Spring.2019.720p.WEB-DL.mp4", "video"),
        ("Tears of Steel (2012)/Tears of Steel (2012).webm", "video"),
        ("Sample Movie (2010)/Sample Movie (2010).mkv", "video"),
        ("sample.mkv", "extra"),
        ("Movie.2010.1080p.sample.x264.mkv", "extra"),
        ("@eaDir/Movie (2010).mkv", "other"),
        ("Movie (2010)/.hidden.mkv", "other"),
        ("Movie (2010)/Movie (2010)-poster.jpg", "image"),
        ("Movie (2010)/Movie (2010).srt", "other"),
    ];

    #[test]
    fn classifies_paths() {
        for (path, want) in CLASSES {
            let got = match classify_path(Path::new(path)) {
                PathClass::Video => "video",
                PathClass::Extra => "extra",
                PathClass::Image { .. } => "image",
                PathClass::Other => "other",
            };
            assert_eq!(got, *want, "{path}");
        }
    }

    #[test]
    fn image_stems_are_lowercased() {
        assert_eq!(
            classify_path(Path::new("Movie (2010)/Movie (2010)-Poster.JPG")),
            PathClass::Image {
                stem: "movie (2010)-poster".into()
            }
        );
    }

    #[test]
    fn skipped_dirs() {
        assert!(is_skipped_dir(Path::new("The Matrix (1999)/Extras")));
        assert!(is_skipped_dir(Path::new("Behind The Scenes")));
        assert!(is_skipped_dir(Path::new("@eaDir")));
        assert!(is_skipped_dir(Path::new(".git")));
        assert!(!is_skipped_dir(Path::new("The Matrix (1999)")));
        assert!(!is_skipped_dir(Path::new("Loose Files")));
    }
}
