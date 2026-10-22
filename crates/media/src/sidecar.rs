//! Subtitle files next to a video: `Movie (2010).en.srt`,
//! `Movie (2010).English.forced.srt`, `Subs/2_English.srt`. What the name
//! says about the track (language, forced, for the hard of hearing) is read
//! here; which video a file belongs to is the scanner's call.

use crate::SubtitleCodec;

/// Extensions read as subtitle files.
pub const SIDECAR_EXTENSIONS: [&str; 4] = ["srt", "ass", "ssa", "vtt"];

/// What a subtitle file's name says about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SidecarName {
    /// ISO 639-2/B when the name has a language (`eng`, `spa`, `fre`), or
    /// a BCP 47 tag the name spelled out (`pt-BR`).
    pub language: Option<String>,
    /// Words the name has that are not a language or a flag (`"Full"`).
    pub title: Option<String>,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub default: bool,
}

/// The codec a subtitle file's extension stands for.
pub fn sidecar_codec(extension: &str) -> Option<SubtitleCodec> {
    match extension.to_ascii_lowercase().as_str() {
        "srt" => Some(SubtitleCodec::Srt),
        "ass" | "ssa" => Some(SubtitleCodec::Ass),
        "vtt" => Some(SubtitleCodec::WebVtt),
        _ => None,
    }
}

/// Reads the part of a subtitle file's stem that isn't the video's: `".en"`,
/// `".eng.forced"`, `".English.SDH"`, `"2_English"`, `".pt-BR"`.
pub fn sidecar_name(rest: &str) -> SidecarName {
    let mut name = SidecarName::default();
    let mut title = Vec::new();
    // YTS-style `Subs/2_English.srt`: a leading track number.
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '_');
    for word in rest
        .split(['.', '_', ' ', '[', ']', '(', ')'])
        .filter(|w| !w.is_empty())
    {
        let lower = word.to_lowercase();
        match lower.as_str() {
            "forced" | "foreign" => name.forced = true,
            "sdh" | "cc" | "hi" | "hoh" => name.hearing_impaired = true,
            "default" => name.default = true,
            _ if name.language.is_none() && language(&lower).is_some() => {
                name.language = language(&lower);
            }
            _ => title.push(word),
        }
    }
    if !title.is_empty() {
        name.title = Some(title.join(" "));
    }
    name
}

/// A language code or English (or native) language name, as ISO 639-2/B;
/// a region tag (`pt-br`) stays a BCP 47 tag.
fn language(word: &str) -> Option<String> {
    if let Some((lang, region)) = word.split_once('-')
        && lang.len() == 2
        && region.len() == 2
        && NAMES.iter().any(|(_, two, _)| *two == lang)
    {
        return Some(format!("{lang}-{}", region.to_uppercase()));
    }
    NAMES
        .iter()
        .find(|(names, two, three)| {
            *two == word
                || three.split('|').any(|t| t == word)
                || names.split('|').any(|n| n == word)
        })
        .map(|(_, _, three)| three.split('|').next().unwrap_or(three).to_owned())
}

/// (names, ISO 639-1, ISO 639-2/B|T): the languages subtitle files are named in.
const NAMES: &[(&str, &str, &str)] = &[
    ("english", "en", "eng"),
    ("spanish|espanol|español|castellano|latino", "es", "spa"),
    ("french|francais|français", "fr", "fre|fra"),
    ("german|deutsch", "de", "ger|deu"),
    ("italian|italiano", "it", "ita"),
    ("portuguese|portugues|português|brazilian", "pt", "por"),
    ("dutch|nederlands", "nl", "dut|nld"),
    ("japanese", "ja", "jpn"),
    ("chinese", "zh", "chi|zho"),
    ("korean", "ko", "kor"),
    ("russian", "ru", "rus"),
    ("polish|polski", "pl", "pol"),
    ("swedish|svenska", "sv", "swe"),
    ("norwegian|norsk", "no", "nor"),
    ("danish|dansk", "da", "dan"),
    ("finnish|suomi", "fi", "fin"),
    ("turkish", "tr", "tur"),
    ("arabic", "ar", "ara"),
    ("hebrew", "he", "heb"),
    ("greek", "el", "gre|ell"),
    ("czech", "cs", "cze|ces"),
    ("hungarian", "hu", "hun"),
    ("romanian", "ro", "rum|ron"),
    ("thai", "th", "tha"),
    ("vietnamese", "vi", "vie"),
    ("indonesian", "id", "ind"),
    ("malay", "ms", "may|msa"),
    ("hindi", "", "hin"),
    ("ukrainian", "uk", "ukr"),
    ("croatian", "hr", "hrv"),
    ("serbian", "sr", "srp"),
    ("bulgarian", "bg", "bul"),
    ("slovak", "sk", "slo|slk"),
    ("slovenian", "sl", "slv"),
    ("catalan", "ca", "cat"),
    ("persian|farsi", "fa", "per|fas"),
    ("tagalog|filipino", "tl", "tgl"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(rest: &str) -> Option<String> {
        sidecar_name(rest).language
    }

    #[test]
    fn languages_by_code_or_name() {
        assert_eq!(lang(".en").as_deref(), Some("eng"));
        assert_eq!(lang(".eng").as_deref(), Some("eng"));
        assert_eq!(lang(".English").as_deref(), Some("eng"));
        assert_eq!(lang(".fra").as_deref(), Some("fre"));
        assert_eq!(lang(".Español").as_deref(), Some("spa"));
        assert_eq!(lang(".pt-BR").as_deref(), Some("pt-BR"));
        assert_eq!(lang("2_English").as_deref(), Some("eng"));
        assert_eq!(lang(""), None);
        assert_eq!(lang(".1080p"), None);
    }

    #[test]
    fn flags_and_leftover_words() {
        let n = sidecar_name(".English.SDH");
        assert!(n.hearing_impaired && !n.forced);
        let n = sidecar_name(".spa.forced.default");
        assert_eq!(n.language.as_deref(), Some("spa"));
        assert!(n.forced && n.default);
        let n = sidecar_name(".en.Full Subs");
        assert_eq!(n.title.as_deref(), Some("Full Subs"));
        assert_eq!(sidecar_codec("SSA"), Some(SubtitleCodec::Ass));
        assert_eq!(sidecar_codec("sub"), None);
    }
}
