//! Subtitle files, attached to the video they belong to.
//!
//! A subtitle file belongs to the video in its folder whose name it starts
//! with (`Movie (2010).en.srt` → `Movie (2010).mkv`), the longest such name
//! winning. A `Subs/` or `Subtitles/` folder counts as its parent folder,
//! and `Subs/<video name>/2_English.srt` belongs to that video. Failing
//! those, a folder with a single video gets every subtitle file in it.

use std::collections::HashMap;

use nuvora_media::{SidecarName, sidecar_name};

use super::walk::DiskFile;
use crate::FileId;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SidecarRow {
    pub file: FileId,
    pub rel: String,
    pub size: u64,
    pub mtime_ns: i64,
    /// `srt`, `ass` or `vtt`.
    pub format: &'static str,
    pub name: SidecarName,
}

const SUBS_FOLDERS: [&str; 3] = ["subs", "subtitles", "sub"];

/// Sidecar rows for `subtitles`, given the videos (`id`, `rel_path`) of the
/// same root. Files that belong to no video are left out.
pub(crate) fn assign(videos: &[(FileId, String)], subtitles: &[DiskFile]) -> Vec<SidecarRow> {
    let mut by_folder: HashMap<&str, Vec<(FileId, &str)>> = HashMap::new();
    for (id, rel) in videos {
        let (folder, name) = split(rel);
        by_folder.entry(folder).or_default().push((*id, stem(name)));
    }
    let mut rows = Vec::new();
    for sub in subtitles {
        let (folder, name) = split(&sub.rel.0);
        let Some((stem_part, ext)) = name.rsplit_once('.') else {
            continue;
        };
        let format = match ext.to_ascii_lowercase().as_str() {
            "srt" => "srt",
            "ass" | "ssa" => "ass",
            "vtt" => "vtt",
            _ => continue,
        };
        let Some((file, rest, by_name)) = owner(&by_folder, folder, stem_part) else {
            continue;
        };
        let mut name = sidecar_name(rest);
        if !by_name {
            // The words are another release's name, not a title.
            name.title = None;
        }
        rows.push(SidecarRow {
            file,
            rel: sub.rel.0.clone(),
            size: sub.size,
            mtime_ns: sub.mtime_ns,
            format,
            name,
        });
    }
    rows.sort_by(|a, b| (a.file.0, &a.rel).cmp(&(b.file.0, &b.rel)));
    rows
}

/// The video a subtitle file in `folder` named `stem` belongs to, the part
/// of the stem after the video's name, and whether the name decided it.
fn owner<'a>(
    by_folder: &HashMap<&str, Vec<(FileId, &str)>>,
    folder: &str,
    stem: &'a str,
) -> Option<(FileId, &'a str, bool)> {
    let (parent, last) = split(folder);
    let last = last.to_lowercase();
    let mut folders = vec![folder];
    if SUBS_FOLDERS.contains(&last.as_str()) {
        folders.push(parent);
    }
    // `Subs/<video name>/...`
    let (grand, parent_name) = split(parent);
    if SUBS_FOLDERS.contains(&parent_name.to_lowercase().as_str())
        && let Some((id, _)) = by_folder
            .get(grand)
            .into_iter()
            .flatten()
            .find(|(_, v)| v.eq_ignore_ascii_case(split(folder).1))
    {
        return Some((*id, stem, true));
    }
    let lower = stem.to_lowercase();
    let by_prefix = folders
        .iter()
        .flat_map(|f| by_folder.get(f).into_iter().flatten())
        .filter(|(_, v)| {
            let v = v.to_lowercase();
            lower.starts_with(&v)
                && lower[v.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| matches!(c, '.' | '_' | '-' | ' '))
        })
        .max_by_key(|(_, v)| v.len());
    if let Some((id, v)) = by_prefix {
        return Some((*id, &stem[v.len()..], true));
    }
    let mut only = folders
        .iter()
        .find_map(|f| by_folder.get(f).filter(|videos| !videos.is_empty()))?
        .iter();
    match (only.next(), only.next()) {
        (Some((id, _)), None) => Some((*id, stem, false)),
        _ => None,
    }
}

/// `"a/b/c.srt"` → `("a/b", "c.srt")`; `"c.srt"` → `("", "c.srt")`.
fn split(rel: &str) -> (&str, &str) {
    rel.rsplit_once('/').unwrap_or(("", rel))
}

fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::walk::RelPath;

    fn disk(rel: &str) -> DiskFile {
        DiskFile {
            rel: RelPath(rel.to_owned()),
            size: 1,
            mtime_ns: 1,
        }
    }

    fn assigned(videos: &[(i64, &str)], subs: &[&str]) -> Vec<(i64, String, Option<String>)> {
        let videos: Vec<(FileId, String)> = videos
            .iter()
            .map(|(id, rel)| (FileId(*id), (*rel).to_owned()))
            .collect();
        let subs: Vec<DiskFile> = subs.iter().map(|s| disk(s)).collect();
        assign(&videos, &subs)
            .into_iter()
            .map(|r| (r.file.0, r.rel, r.name.language))
            .collect()
    }

    #[test]
    fn subtitles_find_their_video() {
        let got = assigned(
            &[
                (1, "Shows/S01/Show - S01E01.mkv"),
                (2, "Shows/S01/Show - S01E02.mkv"),
                (3, "Movie (2010)/Movie (2010).mp4"),
                (4, "Other (2011)/Other.2011.1080p.mkv"),
                (5, "Yts (2012)/Yts (2012).mp4"),
                (6, "Episodes/Ep.S01E03.mkv"),
                (7, "Episodes/Ep.S01E04.mkv"),
            ],
            &[
                "Shows/S01/Show - S01E01.en.srt",
                "Shows/S01/Show - S01E02.spa.forced.ass",
                "Shows/S01/Unrelated.srt",
                "Movie (2010)/Movie (2010).srt",
                "Movie (2010)/Subs/Movie (2010).fr.srt",
                "Other (2011)/english.srt",
                "Yts (2012)/Subs/2_English.srt",
                "Episodes/Subs/Ep.S01E04/3_Spanish.srt",
            ],
        );
        assert_eq!(
            got,
            [
                (
                    1,
                    "Shows/S01/Show - S01E01.en.srt".into(),
                    Some("eng".into())
                ),
                (
                    2,
                    "Shows/S01/Show - S01E02.spa.forced.ass".into(),
                    Some("spa".into())
                ),
                (3, "Movie (2010)/Movie (2010).srt".into(), None),
                (
                    3,
                    "Movie (2010)/Subs/Movie (2010).fr.srt".into(),
                    Some("fre".into())
                ),
                (4, "Other (2011)/english.srt".into(), Some("eng".into())),
                (
                    5,
                    "Yts (2012)/Subs/2_English.srt".into(),
                    Some("eng".into())
                ),
                (
                    7,
                    "Episodes/Subs/Ep.S01E04/3_Spanish.srt".into(),
                    Some("spa".into())
                ),
            ]
        );
    }
}
