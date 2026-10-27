//! What a release's name says it holds: the movie or show, whether it is a
//! movie, some episodes or whole seasons, its audio and subtitle languages,
//! and the picture. Release names are indexer titles, not file paths: dots
//! or spaces, tracker tags in brackets, sometimes a translated title first
//! (`Оппенгеймер / Oppenheimer (2023)`). Pure functions over the name.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::MovieName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseName {
    /// The movie or show.
    pub title: String,
    pub year: Option<u16>,
    pub content: Content,
    /// ISO 639-1 codes of the audio the name names, in order. Empty when it
    /// names none, which usually means the original language only.
    pub audio: Vec<&'static str>,
    /// ISO 639-1 codes of the subtitles the name names; `"mul"` for "many".
    pub subtitles: Vec<&'static str>,
    /// More than one audio track (`Dual Audio`, `MULTi`).
    pub dual_audio: bool,
    /// A dubbed soundtrack in place of the original.
    pub dubbed: bool,
    /// Subtitles burned into the picture (`HC`, `KORSUB`): they can't be
    /// turned off.
    pub hardcoded_subs: bool,
    pub tags: Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    Movie,
    /// Episodes `first..=last`. `season` is `None` for episodes numbered
    /// across the whole show, as anime often is (`Show - 05`).
    Episodes {
        season: Option<u32>,
        first: u32,
        last: u32,
    },
    /// Whole seasons `first..=last`.
    Seasons {
        first: u32,
        last: u32,
    },
    /// Every season.
    Complete,
}

impl Content {
    pub fn is_show(self) -> bool {
        self != Content::Movie
    }
}

/// What the name says about the picture.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tags {
    /// 2160, 1080, 720 or 480.
    pub resolution: Option<u16>,
    pub hdr: Option<Hdr>,
    pub codec: Option<Codec>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hdr {
    Hdr,
    DolbyVision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Remux,
    BluRay,
    Web,
    Hdtv,
    Dvd,
    /// Filmed in a cinema: poor picture and sound.
    Cam,
}

/// Language words and tracker codes, after the title only: `French` in
/// `The French Dispatch` is the title's.
const LANGUAGES: &[(&str, &str)] = &[
    ("english", "en"),
    ("eng", "en"),
    ("russian", "ru"),
    ("rus", "ru"),
    ("ukrainian", "uk"),
    ("ukr", "uk"),
    ("italian", "it"),
    ("ita", "it"),
    ("french", "fr"),
    ("truefrench", "fr"),
    ("vff", "fr"),
    ("vfq", "fr"),
    ("vfi", "fr"),
    ("german", "de"),
    ("ger", "de"),
    ("deutsch", "de"),
    ("spanish", "es"),
    ("spa", "es"),
    ("esp", "es"),
    ("castellano", "es"),
    ("latino", "es"),
    ("portuguese", "pt"),
    ("por", "pt"),
    ("dublado", "pt"),
    ("polish", "pl"),
    ("pol", "pl"),
    ("lektor", "pl"),
    ("dutch", "nl"),
    ("swedish", "sv"),
    ("swe", "sv"),
    ("danish", "da"),
    ("norwegian", "no"),
    ("finnish", "fi"),
    ("czech", "cs"),
    ("hungarian", "hu"),
    ("romanian", "ro"),
    ("greek", "el"),
    ("turkish", "tr"),
    ("tur", "tr"),
    ("arabic", "ar"),
    ("ara", "ar"),
    ("persian", "fa"),
    ("hebrew", "he"),
    ("hindi", "hi"),
    ("hin", "hi"),
    ("tamil", "ta"),
    ("tam", "ta"),
    ("telugu", "te"),
    ("tel", "te"),
    ("malayalam", "ml"),
    ("kannada", "kn"),
    ("bengali", "bn"),
    ("japanese", "ja"),
    ("jap", "ja"),
    ("jpn", "ja"),
    ("korean", "ko"),
    ("kor", "ko"),
    ("chinese", "zh"),
    ("chi", "zh"),
    ("mandarin", "zh"),
    ("cantonese", "zh"),
    ("thai", "th"),
    ("vietnamese", "vi"),
    ("indonesian", "id"),
];

/// Words that make the language next to them a subtitle language.
const SUBTITLE_WORDS: &[&str] = &[
    "sub",
    "subs",
    "subbed",
    "subtitle",
    "subtitles",
    "subtitled",
];

/// The Russian trackers' voice-over kinds: dubbing, multi-voice, dual-voice,
/// single-voice. A sure sign of a Russian soundtrack.
const RUSSIAN_VOICE_OVERS: &[&str] = &["d", "mvo", "dvo", "avo", "vo"];

/// `S01E02`, and the start of `S01E02E03` and `S01E01-E10`.
static EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bS(\d{1,2}) ?E(\d{1,4})").expect("regex"));
static MORE_EPISODES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?: ?- ?E?| ?E)(\d{1,4})\b").expect("regex"));
static CROSS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})x(\d{2,3})(?:-(\d{2,3}))?\b").expect("regex"));
/// `S01-S10`, `S01-10`, `S01 - S10`, but not `S2 - 03` (an anime episode).
static SEASON_RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bS(\d{1,2})(?:-S?| ?(?:-|to|thru|&) ?S)(\d{1,2})\b").expect("regex")
});
/// `Seasons 1-9`, `Season 1 to 3`.
static SEASON_RANGE_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bSeasons? ?(\d{1,2}) ?(?:-|to|&|and|thru) ?(\d{1,2})\b").expect("regex")
});
static SEASON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:S|Season ?)(\d{1,2})\b").expect("regex"));
static EPISODE_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Episodes?|Ep) ?(\d{1,4})(?: ?- ?(\d{1,4}))?\b").expect("regex")
});
/// Anime: `Show - 05`, `Show - 01 ~ 12`, `Show - 05v2`.
static DASH_EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^| )- (\d{1,4})(?:v\d)?(?: ?[-~] ?(\d{1,4})(?:v\d)?)?(?: |$)").expect("regex")
});
/// Anime batches without a dash: `Show 1-13 END`, `Show (01-28)`, `Show 01~26`.
static BARE_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^| |\()(\d{1,3}) ?[-~] ?(\d{1,3})(?:\)| |$)").expect("regex"));
/// Words that make `Harry Potter 1-8` a box of movies, not episodes.
static MOVIE_SET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:collection|movies|films|trilogy|quadrilogy|saga|anthology)\b")
        .expect("regex")
});
static COMPLETE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:complete (?:series|collection|seasons|box ?set)|all seasons)\b")
        .expect("regex")
});
static LEADING_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s*(?:\[[^\]]*\]|【[^】]*】|\([^)]*\)))+").expect("regex"));

impl ReleaseName {
    pub fn parse(name: &str) -> ReleaseName {
        let scripts = script_languages(name);
        let base = LEADING_TAGS.replace(latin_part(name), "");
        let text: String = base
            .chars()
            .map(|c| if c == '.' || c == '_' { ' ' } else { c })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let found = content(&text);
        let (title_end, content) = match found {
            Some((start, content)) => (start, content),
            None => (movie_title_end(&text), Content::Movie),
        };
        let title_part = text[..title_end].trim_end_matches([' ', '-', '(', '[']);
        let named = MovieName::parse(Path::new(&format!("{title_part}.mkv")));
        let tail = &text[title_end..];
        let mut release = ReleaseName {
            title: named.title,
            year: named.year.or_else(|| year_in(tail)),
            content,
            audio: Vec::new(),
            subtitles: Vec::new(),
            dual_audio: false,
            dubbed: false,
            hardcoded_subs: false,
            tags: Tags::parse(name),
        };
        release.read_languages(tail, &scripts);
        release
    }

    fn read_languages(&mut self, tail: &str, scripts: &[&'static str]) {
        let tokens: Vec<String> = words(&tail.to_lowercase());
        let is_sub = |i: usize| -> bool {
            [i.checked_sub(1), Some(i + 1)]
                .into_iter()
                .flatten()
                .filter_map(|j| tokens.get(j))
                .any(|t| SUBTITLE_WORDS.contains(&t.as_str()))
        };
        let russian =
            scripts.contains(&"ru") || tokens.iter().any(|t| t == "rus" || t == "russian");
        for (i, token) in tokens.iter().enumerate() {
            let token = token.as_str();
            match token {
                "dual" if tokens.get(i + 1).is_some_and(|t| t == "audio") => self.dual_audio = true,
                "multi" | "multiaudio" if !is_sub(i) => self.dual_audio = true,
                "multi" | "multisub" | "multisubs" | "multiple" => {
                    if token != "multiple" || is_sub(i) {
                        push(&mut self.subtitles, "mul");
                    }
                }
                "dub" | "dubbed" | "dubbing" | "dublado" => self.dubbed = true,
                "hc" | "hardsub" | "hardsubs" | "hardcoded" => self.hardcoded_subs = true,
                "esub" | "esubs" | "engsub" | "engsubs" => push(&mut self.subtitles, "en"),
                "korsub" | "korsubs" => {
                    self.hardcoded_subs = true;
                    push(&mut self.subtitles, "ko");
                }
                "vostfr" | "subfrench" => push(&mut self.subtitles, "fr"),
                _ if russian && RUSSIAN_VOICE_OVERS.contains(&token) => {
                    self.dubbed |= token == "d";
                    push(&mut self.audio, "ru");
                }
                _ => {
                    if let Some(&(_, code)) = LANGUAGES.iter().find(|(word, _)| *word == token) {
                        if is_sub(i) {
                            push(&mut self.subtitles, code);
                        } else {
                            push(&mut self.audio, code);
                        }
                        if matches!(token, "dublado" | "lektor") {
                            self.dubbed = true;
                        }
                    }
                }
            }
        }
        // A title in another script names the release for that language's
        // audience, and its soundtrack with it.
        for &code in scripts {
            if !(code == "ru" && self.audio.contains(&"uk")) {
                push(&mut self.audio, code);
            }
        }
        if self.audio.len() > 1 {
            self.dual_audio = true;
        }
    }
}

fn push(list: &mut Vec<&'static str>, code: &'static str) {
    if !list.contains(&code) {
        list.push(code);
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `Оппенгеймер / Oppenheimer (2023) ...`: from the first title written in
/// Latin letters only, so the title is the one the rest of the world uses.
fn latin_part(name: &str) -> &str {
    let mut start = 0;
    for part in name.split(" / ") {
        let latin = part.chars().any(|c| c.is_ascii_alphabetic())
            && !part.chars().any(|c| c.is_alphabetic() && !c.is_ascii());
        if latin {
            return &name[start..];
        }
        start += part.len() + " / ".len();
    }
    name
}

/// Languages a title's script points at: Cyrillic, Japanese kana, Hangul,
/// Han, Arabic, Devanagari, Thai, Hebrew, Greek.
fn script_languages(name: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let has =
        |range: std::ops::RangeInclusive<u32>| name.chars().any(|c| range.contains(&(c as u32)));
    if has(0x0400..=0x04FF) {
        found.push("ru");
    }
    let kana = has(0x3040..=0x30FF);
    if kana {
        found.push("ja");
    }
    if has(0xAC00..=0xD7AF) {
        found.push("ko");
    }
    if !kana && has(0x4E00..=0x9FFF) {
        found.push("zh");
    }
    for (range, code) in [
        (0x0600..=0x06FF, "ar"),
        (0x0900..=0x097F, "hi"),
        (0x0E00..=0x0E7F, "th"),
        (0x0590..=0x05FF, "he"),
        (0x0370..=0x03FF, "el"),
    ] {
        if has(range) {
            found.push(code);
        }
    }
    found
}

/// Where the episode or season marker starts, and what it says.
fn content(text: &str) -> Option<(usize, Content)> {
    if let Some(m) = EPISODE.captures(text) {
        let whole = m.get(0).expect("match");
        let season = m[1].parse().ok();
        let first: u32 = m[2].parse().ok()?;
        let mut last = first;
        let mut rest = &text[whole.end()..];
        while let Some(more) = MORE_EPISODES.captures(rest) {
            last = more[1].parse().unwrap_or(last).max(last);
            rest = &rest[more.get(0).expect("match").end()..];
        }
        return Some((
            whole.start(),
            Content::Episodes {
                season,
                first,
                last,
            },
        ));
    }
    if let Some(m) = CROSS.captures(text) {
        let first: u32 = m[2].parse().ok()?;
        let last = m
            .get(3)
            .and_then(|l| l.as_str().parse().ok())
            .unwrap_or(first);
        let season = m[1].parse().ok();
        return Some((
            m.get(0)?.start(),
            Content::Episodes {
                season,
                first,
                last,
            },
        ));
    }
    for pattern in [&*SEASON_RANGE, &*SEASON_RANGE_WORDS] {
        if let Some(m) = pattern.captures(text) {
            let (first, last): (u32, u32) = (m[1].parse().ok()?, m[2].parse().ok()?);
            if first < last {
                return Some((m.get(0)?.start(), Content::Seasons { first, last }));
            }
        }
    }
    if let Some(m) = SEASON.captures(text) {
        let season: u32 = m[1].parse().ok()?;
        let start = m.get(0)?.start();
        let after = &text[m.get(0)?.end()..];
        // `Show S2 - 03`, `Show Season 1 Episode 5`: an episode of that season.
        if let Some(e) = DASH_EPISODE
            .captures(after)
            .or_else(|| EPISODE_WORD.captures(after))
        {
            let first: u32 = e[1].parse().ok()?;
            let last = e
                .get(2)
                .and_then(|l| l.as_str().parse().ok())
                .unwrap_or(first);
            return Some((
                start,
                Content::Episodes {
                    season: Some(season),
                    first,
                    last,
                },
            ));
        }
        return Some((
            start,
            Content::Seasons {
                first: season,
                last: season,
            },
        ));
    }
    if let Some(m) = COMPLETE.find(text) {
        return Some((m.start(), Content::Complete));
    }
    for pattern in [&*DASH_EPISODE, &*EPISODE_WORD] {
        if let Some(e) = pattern.captures(text) {
            let first: u32 = e[1].parse().ok()?;
            // `Movie - 2019`: a year, not episode 2019.
            if (1900..=2100).contains(&first) {
                continue;
            }
            let last = e
                .get(2)
                .and_then(|l| l.as_str().parse().ok())
                .unwrap_or(first);
            return Some((
                e.get(0)?.start(),
                Content::Episodes {
                    season: None,
                    first,
                    last,
                },
            ));
        }
    }
    // A short unpadded range (`Rocky 1-6`) is more likely a set of movies.
    if let Some(e) = BARE_RANGE.captures(text)
        && !MOVIE_SET.is_match(text)
    {
        let start = e.get(0)?.start();
        let (first, last): (u32, u32) = (e[1].parse().ok()?, e[2].parse().ok()?);
        let padded = e[1].len() > 1 && e[1].starts_with('0');
        if start > 0 && first < last && (last >= 10 || padded) {
            return Some((
                start,
                Content::Episodes {
                    season: None,
                    first,
                    last,
                },
            ));
        }
    }
    None
}

/// A movie's title ends at its year (a year that has come, and not the first
/// word, so `1917` and `Blade Runner 2049 (2017)` work), or without one at
/// the first quality, source or language word.
fn movie_title_end(text: &str) -> usize {
    let lower = text.to_lowercase();
    let latest = crate::name::current_year() + 1;
    let mut starts = Vec::new();
    let mut offset = 0;
    for word in lower.split(' ') {
        starts.push((offset, word.trim_matches(|c: char| !c.is_alphanumeric())));
        offset += word.len() + 1;
    }
    let year = starts.iter().skip(1).find(|(_, token)| {
        token.len() == 4
            && token
                .parse::<u16>()
                .is_ok_and(|y| (1900..=latest).contains(&y))
    });
    let release = || {
        starts.iter().skip(1).find(|(_, token)| {
            is_resolution(token)
                || crate::name::RELEASE_TOKENS.contains(token)
                || matches!(
                    *token,
                    "web" | "bdrip" | "hdrip" | "dvdscr" | "cam" | "hdcam"
                )
                || LANGUAGES.iter().any(|(w, _)| w == token && w.len() > 3)
        })
    };
    year.or_else(release)
        .map_or(text.len(), |(start, _)| *start)
}

fn is_resolution(token: &str) -> bool {
    token
        .strip_suffix(['p', 'i'])
        .is_some_and(|n| (3..=4).contains(&n.len()) && n.chars().all(|c| c.is_ascii_digit()))
}

fn year_in(text: &str) -> Option<u16> {
    words(text)
        .iter()
        .find_map(|w| w.parse::<u16>().ok().filter(|y| (1900..=2100).contains(y)))
}

impl Tags {
    pub fn parse(title: &str) -> Tags {
        let lower = title
            .to_lowercase()
            .replace("h.264", "h264")
            .replace("h.265", "h265");
        let tokens = words(&lower);
        let has = |list: &[&str]| tokens.iter().any(|t| list.contains(&t.as_str()));
        let resolution = if has(&["2160p", "4k", "uhd"]) {
            Some(2160)
        } else if has(&["1080p", "1080i"]) {
            Some(1080)
        } else if has(&["720p"]) {
            Some(720)
        } else if has(&["480p", "576p"]) {
            Some(480)
        } else {
            None
        };
        let hdr = if has(&["dv", "dovi"]) || lower.contains("dolby vision") {
            Some(Hdr::DolbyVision)
        } else if has(&["hdr", "hdr10", "hdr10+"]) {
            Some(Hdr::Hdr)
        } else {
            None
        };
        let codec = if has(&["x265", "h265", "hevc"]) {
            Some(Codec::Hevc)
        } else if has(&["x264", "h264", "avc"]) {
            Some(Codec::H264)
        } else if has(&["av1"]) {
            Some(Codec::Av1)
        } else {
            None
        };
        let source = if has(&["remux", "bdremux"]) {
            Some(Source::Remux)
        } else if has(&[
            "cam", "camrip", "hdcam", "ts", "hdts", "telesync", "tc", "telecine",
        ]) {
            Some(Source::Cam)
        } else if has(&["bluray", "bdrip", "brrip"]) || lower.contains("blu-ray") {
            Some(Source::BluRay)
        } else if has(&["web", "webrip", "webdl"]) {
            Some(Source::Web)
        } else if has(&["hdtv"]) {
            Some(Source::Hdtv)
        } else if has(&["dvdrip", "dvd", "dvd5", "dvd9"]) {
            Some(Source::Dvd)
        } else {
            None
        };
        Tags {
            resolution,
            hdr,
            codec,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (name, title, year, content, audio, subtitles)
    #[allow(clippy::type_complexity)]
    const CASES: &[(&str, &str, Option<u16>, Content, &[&str], &[&str])] = &[
        (
            "Oppenheimer.2023.1080p.BluRay.x264-GRP",
            "Oppenheimer",
            Some(2023),
            Content::Movie,
            &[],
            &[],
        ),
        (
            "Оппенгеймер / Oppenheimer (2023) BDRip-IMAX | D | Лицензия",
            "Oppenheimer",
            Some(2023),
            Content::Movie,
            &["ru"],
            &[],
        ),
        (
            "The French Dispatch 2021 1080p BluRay",
            "The French Dispatch",
            Some(2021),
            Content::Movie,
            &[],
            &[],
        ),
        (
            "1917 (2019) 2160p UHD BluRay",
            "1917",
            Some(2019),
            Content::Movie,
            &[],
            &[],
        ),
        (
            "Film.2020.iTALiAN.BDRip.x264",
            "Film",
            Some(2020),
            Content::Movie,
            &["it"],
            &[],
        ),
        (
            "Film 2021 VOSTFR 1080p WEB",
            "Film",
            Some(2021),
            Content::Movie,
            &[],
            &["fr"],
        ),
        (
            "Movie Name (2019) [Hindi + English] Dual Audio 1080p",
            "Movie Name",
            Some(2019),
            Content::Movie,
            &["hi", "en"],
            &[],
        ),
        (
            "Movie 2022 1080p WEB ENG SUB",
            "Movie",
            Some(2022),
            Content::Movie,
            &[],
            &["en"],
        ),
        (
            "Movie 2022 HDCAM KORSUB",
            "Movie",
            Some(2022),
            Content::Movie,
            &[],
            &["ko"],
        ),
        (
            "Breaking.Bad.S05E14.Ozymandias.1080p.WEB-DL.DD5.1.H.264",
            "Breaking Bad",
            None,
            Content::Episodes {
                season: Some(5),
                first: 14,
                last: 14,
            },
            &[],
            &[],
        ),
        (
            "Show.Name.S02E01E02.720p.HDTV",
            "Show Name",
            None,
            Content::Episodes {
                season: Some(2),
                first: 1,
                last: 2,
            },
            &[],
            &[],
        ),
        (
            "Show Name S03E01-E10 1080p",
            "Show Name",
            None,
            Content::Episodes {
                season: Some(3),
                first: 1,
                last: 10,
            },
            &[],
            &[],
        ),
        (
            "Show Name 3x07 HDTV",
            "Show Name",
            None,
            Content::Episodes {
                season: Some(3),
                first: 7,
                last: 7,
            },
            &[],
            &[],
        ),
        (
            "Friends.S01-S10.COMPLETE.720p.BluRay.x264",
            "Friends",
            None,
            Content::Seasons { first: 1, last: 10 },
            &[],
            &[],
        ),
        (
            "The Office (US) Seasons 1-9 1080p",
            "The Office US",
            None,
            Content::Seasons { first: 1, last: 9 },
            &[],
            &[],
        ),
        (
            "High School DxD S01 1080p BluRay Dual Audio x265",
            "High School DxD",
            None,
            Content::Seasons { first: 1, last: 1 },
            &[],
            &[],
        ),
        (
            "Seinfeld Complete Series 1080p WEB",
            "Seinfeld",
            None,
            Content::Complete,
            &[],
            &[],
        ),
        (
            "[SubsPlease] High School DxD Hero - 05 (1080p) [ABCDEF12]",
            "High School DxD Hero",
            None,
            Content::Episodes {
                season: None,
                first: 5,
                last: 5,
            },
            &[],
            &[],
        ),
        (
            "[Erai-raws] Some Anime - 01 ~ 12 [1080p][Multiple Subtitle]",
            "Some Anime",
            None,
            Content::Episodes {
                season: None,
                first: 1,
                last: 12,
            },
            &[],
            &["mul"],
        ),
        (
            "【推しの子】 Oshi no Ko S2 - 03 [1080p]",
            "Oshi no Ko",
            None,
            Content::Episodes {
                season: Some(2),
                first: 3,
                last: 3,
            },
            &["ja"],
            &[],
        ),
        (
            "Squid Game S01 1080p Korean English Dubbed",
            "Squid Game",
            None,
            Content::Seasons { first: 1, last: 1 },
            &["ko", "en"],
            &[],
        ),
        (
            "Show Name Season 1 Episode 5 720p",
            "Show Name",
            None,
            Content::Episodes {
                season: Some(1),
                first: 5,
                last: 5,
            },
            &[],
            &[],
        ),
        (
            "Serial Experiments Lain 1-13 END MP4",
            "Serial Experiments Lain",
            None,
            Content::Episodes {
                season: None,
                first: 1,
                last: 13,
            },
            &[],
            &[],
        ),
        (
            "[SubsPlease] Sousou no Frieren (01-28) (1080p) [Batch]",
            "Sousou no Frieren",
            None,
            Content::Episodes {
                season: None,
                first: 1,
                last: 28,
            },
            &[],
            &[],
        ),
        (
            "Cowboy Bebop 01~26 BD 1080p",
            "Cowboy Bebop",
            None,
            Content::Episodes {
                season: None,
                first: 1,
                last: 26,
            },
            &[],
            &[],
        ),
    ];

    #[test]
    fn movie_sets_stay_movies() {
        for name in [
            "Rocky 1-6 1080p BluRay",
            "Harry Potter 1-8 Collection 1080p",
            "Fast and Furious 1-10 Movies 1080p",
            "2 Fast 2 Furious 2003 1080p",
            "Oppenheimer.2023.1080p.BluRay.DD5.1.x264-GalaxyRG",
        ] {
            assert_eq!(ReleaseName::parse(name).content, Content::Movie, "{name}");
        }
    }

    #[test]
    fn release_names_say_what_they_hold() {
        for &(name, title, year, content, audio, subtitles) in CASES {
            let r = ReleaseName::parse(name);
            assert_eq!(
                (
                    r.title.as_str(),
                    r.year,
                    r.content,
                    r.audio.as_slice(),
                    r.subtitles.as_slice()
                ),
                (title, year, content, audio, subtitles),
                "{name}"
            );
        }
    }

    #[test]
    fn dubs_dual_audio_and_burned_in_subtitles() {
        let russian =
            ReleaseName::parse("Оппенгеймер / Oppenheimer (2023) BDRip-IMAX | D | Лицензия");
        assert!(russian.dubbed);
        let dual = ReleaseName::parse("High School DxD S01 1080p BluRay Dual Audio x265");
        assert!(dual.dual_audio && !dual.dubbed);
        assert!(ReleaseName::parse("Movie.2019.MULTi.1080p.BluRay").dual_audio);
        assert!(ReleaseName::parse("Squid Game S01 1080p Korean English Dubbed").dubbed);
        assert!(ReleaseName::parse("Movie 2022 HDCAM KORSUB").hardcoded_subs);
        assert!(!ReleaseName::parse("Oppenheimer.2023.1080p.BluRay.x264-GRP").dual_audio);
    }

    #[test]
    fn release_names_tell_the_picture() {
        assert_eq!(
            Tags::parse("Dune.Part.Two.2024.2160p.UHD.BluRay.REMUX.DV.HDR.HEVC.TrueHD-GRP"),
            Tags {
                resolution: Some(2160),
                hdr: Some(Hdr::DolbyVision),
                codec: Some(Codec::Hevc),
                source: Some(Source::Remux),
            }
        );
        assert_eq!(
            Tags::parse("Some Movie (2023) [1080p] [WEB-DL] [H.264]"),
            Tags {
                resolution: Some(1080),
                hdr: None,
                codec: Some(Codec::H264),
                source: Some(Source::Web),
            }
        );
        assert_eq!(
            Tags::parse("New.Movie.2026.HDCAM.x264").source,
            Some(Source::Cam)
        );
        assert_eq!(Tags::parse("Plain Title"), Tags::default());
    }
}
