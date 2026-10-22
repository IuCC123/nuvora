//! Filename knowledge: which paths are movies, which are extras or artwork, and
//! what title/year a movie path names. Pure functions over relative paths.

use std::path::Path;

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

/// Classify one path relative to its library root.
pub fn classify_path(rel: &Path) -> PathClass {
    let _ = rel;
    todo!("extension table + extras folder/suffix rules; hidden files (`.`-prefixed, `@eaDir`, `#recycle`) => Other")
}

/// Title and year parsed from a movie's path.
///
/// Rules, in order:
/// 1. If the parent folder looks like `Title (Year)` (optionally `[...]` tags),
///    use it: folder-per-movie is the convention Plex users follow.
/// 2. Else parse the file stem: split on `.`/`_`/space, take tokens up to the
///    first year in 1900..=current+1 that is not the first token (so `1917.mkv`
///    and `2001 A Space Odyssey (1968)` work), drop quality/source/codec tokens.
/// 3. No year found: the whole cleaned stem is the title, year = None.
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
        let _ = rel;
        todo!("rules 1-3 above")
    }

    /// `"<casefolded alnum title>|<year or ''>"`. Diacritics folded, `&` -> `and`.
    pub fn identity_key(&self) -> IdentityKey {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (relative path, expected title, expected year). The table is the spec;
    /// every bug report about a misparsed name becomes a row here.
    const CASES: &[(&str, &str, Option<u16>)] = &[
        ("The Matrix (1999)/The Matrix (1999).mkv", "The Matrix", Some(1999)),
        ("The.Matrix.1999.1080p.BluRay.x264-GRP.mkv", "The Matrix", Some(1999)),
        ("Blade Runner 2049 (2017)/Blade.Runner.2049.2160p.UHD.mkv", "Blade Runner 2049", Some(2017)),
        ("1917 (2019)/1917.mkv", "1917", Some(2019)),
        ("2001.A.Space.Odyssey.1968.mkv", "2001 A Space Odyssey", Some(1968)),
        ("Movies/Amélie [2001] [1080p].mp4", "Amélie", Some(2001)),
        ("Heat.mkv", "Heat", None),
        ("Alien (1979)/Alien (1979) - 4K.mkv", "Alien", Some(1979)),
    ];

    #[test]
    #[ignore = "sketch: enable with the implementation"]
    fn parses_titles_and_years() {
        for (path, title, year) in CASES {
            let n = MovieName::parse(Path::new(path));
            assert_eq!((n.title.as_str(), n.year), (*title, *year), "{path}");
        }
    }

    #[test]
    #[ignore = "sketch: enable with the implementation"]
    fn versions_of_one_movie_share_an_identity_key() {
        let a = MovieName::parse(Path::new("Alien (1979)/Alien (1979) - 1080p.mkv"));
        let b = MovieName::parse(Path::new("Alien.1979.2160p.UHD.HDR.mkv"));
        assert_eq!(a.identity_key(), b.identity_key());
    }

    const CLASSES: &[(&str, &str)] = &[
        ("The Matrix (1999)/The Matrix (1999).mkv", "video"),
        ("The Matrix (1999)/Extras/Making Of.mkv", "extra"),
        ("The Matrix (1999)/the.matrix.sample.mkv", "extra"),
        ("The Matrix (1999)/The Matrix-trailer.mp4", "extra"),
        ("The Matrix (1999)/poster.jpg", "image"),
        ("The Matrix (1999)/movie.nfo", "other"),
        (".DS_Store", "other"),
    ];

    #[test]
    #[ignore = "sketch: enable with the implementation"]
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
}
