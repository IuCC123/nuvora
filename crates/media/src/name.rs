//! Filename knowledge: which paths are movies, which are extras or artwork, and
//! what title/year a movie path names. Pure functions over relative paths.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
        let mut key = String::with_capacity(self.title.len() + 5);
        for c in self.title.chars() {
            if c == '&' {
                key.push_str("and");
            } else if let Some(folded) = fold_diacritic(c) {
                key.push_str(folded);
            } else if c.is_alphanumeric() {
                key.extend(c.to_lowercase());
            }
        }
        key.push('|');
        if let Some(year) = self.year {
            key.push_str(&year.to_string());
        }
        IdentityKey(key)
    }
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
    let tokens = tokens(stem);
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
    ];

    #[test]
    fn parses_titles_and_years() {
        for (path, title, year) in CASES {
            let n = MovieName::parse(Path::new(path));
            assert_eq!((n.title.as_str(), n.year), (*title, *year), "{path}");
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
