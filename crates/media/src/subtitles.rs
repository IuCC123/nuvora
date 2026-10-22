//! Text subtitles and font attachments, pulled out of a media file with
//! ffmpeg. Each call writes one output file; caching and concurrency are the
//! caller's job.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

use crate::{MediaTools, ProbeError, SubtitleCodec};

/// The text formats a subtitle stream can be extracted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleText {
    WebVtt,
    Ass,
}

impl SubtitleText {
    pub fn extension(self) -> &'static str {
        match self {
            SubtitleText::WebVtt => "vtt",
            SubtitleText::Ass => "ass",
        }
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            SubtitleText::WebVtt => "text/vtt; charset=utf-8",
            SubtitleText::Ass => "text/x-ssa; charset=utf-8",
        }
    }

    fn muxer(self) -> &'static str {
        match self {
            SubtitleText::WebVtt => "webvtt",
            SubtitleText::Ass => "ass",
        }
    }
}

/// A font attached to a Matroska file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontAttachment {
    pub index: u32,
    pub file_name: String,
    /// Normalized to `font/*`.
    pub mime_type: &'static str,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("ffmpeg timed out after {0:?}")]
    Timeout(Duration),
    #[error("ffmpeg failed: {stderr}")]
    Failed { stderr: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Writes stream `index` of `path` (a subtitle stream of `codec`) to `out` as
/// `text`. An ASS stream extracted as ASS is copied, so the script keeps every
/// style, position and comment line. `charset` names a text file's encoding
/// when it isn't UTF-8 (`"CP1252"`).
pub async fn extract_subtitle(
    tools: &MediaTools,
    path: &Path,
    index: u32,
    codec: &SubtitleCodec,
    text: SubtitleText,
    out: &Path,
    charset: Option<&str>,
) -> Result<(), ExtractError> {
    let encoder = match (codec, text) {
        (SubtitleCodec::Ass, SubtitleText::Ass) => "copy",
        (_, text) => text.muxer(),
    };
    let mut command = Command::new(&tools.ffmpeg);
    command.args(["-nostdin", "-v", "error", "-y"]);
    if let Some(charset) = charset {
        command.args(["-sub_charenc", charset]);
    }
    command
        .arg("-i")
        .arg(path)
        .args(["-map", &format!("0:{index}"), "-c:s", encoder])
        .args(["-f", text.muxer()])
        .arg(out);
    let timeout = tools.extract_timeout;
    let output = run(command, timeout)
        .await?
        .ok_or(ExtractError::Timeout(timeout))?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(())
}

/// The file's font attachments, in stream order. Attachments that are not
/// fonts (cover art, NFOs) are left out.
pub async fn font_attachments(
    tools: &MediaTools,
    path: &Path,
) -> Result<Vec<FontAttachment>, ProbeError> {
    let mut command = Command::new(&tools.ffprobe);
    command
        .args(["-v", "error", "-print_format", "json", "-show_streams"])
        .args(["-select_streams", "t"])
        .arg(path);
    let timeout = tools.probe_timeout;
    let output = run(command, timeout)
        .await?
        .ok_or(ProbeError::Timeout(timeout))?;
    if !output.status.success() {
        return Err(ProbeError::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_fonts(&output.stdout)
}

/// Writes attachment stream `index` of `path` to `out`.
pub async fn dump_attachment(
    tools: &MediaTools,
    path: &Path,
    index: u32,
    out: &Path,
) -> Result<(), ExtractError> {
    let mut command = Command::new(&tools.ffmpeg);
    command
        .args(["-nostdin", "-v", "error", "-y"])
        .arg(format!("-dump_attachment:{index}"))
        .arg(out)
        .arg("-i")
        .arg(path);
    let timeout = tools.extract_timeout;
    let output = run(command, timeout)
        .await?
        .ok_or(ExtractError::Timeout(timeout))?;
    // ffmpeg dumps while opening the input, then exits 1 because no output
    // file was named; the dumped file is the result.
    match tokio::fs::metadata(out).await {
        Ok(meta) if meta.len() > 0 => Ok(()),
        _ => Err(failed(&output)),
    }
}

/// The font type from the file's first bytes, or `None` when they are not a
/// font's.
pub fn font_mime_type(head: &[u8]) -> Option<&'static str> {
    match head.get(..4)? {
        [0, 1, 0, 0] | b"true" => Some("font/ttf"),
        b"OTTO" => Some("font/otf"),
        b"ttcf" => Some("font/collection"),
        b"wOFF" => Some("font/woff"),
        b"wOF2" => Some("font/woff2"),
        _ => None,
    }
}

/// `None` when `command` outlived `timeout`; it is killed then.
async fn run(
    mut command: Command,
    timeout: Duration,
) -> std::io::Result<Option<std::process::Output>> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(output) => output.map(Some),
        Err(_) => Ok(None),
    }
}

fn failed(output: &std::process::Output) -> ExtractError {
    ExtractError::Failed {
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
}

/// Muxers label fonts inconsistently (`application/x-truetype-font`,
/// `font/ttf`, none at all), so the file name's extension decides when the
/// MIME type does not.
fn parse_fonts(json: &[u8]) -> Result<Vec<FontAttachment>, ProbeError> {
    #[derive(Deserialize)]
    struct Output {
        #[serde(default)]
        streams: Vec<Stream>,
    }
    #[derive(Deserialize)]
    struct Stream {
        index: u32,
        codec_type: Option<String>,
        extradata_size: Option<u64>,
        #[serde(default)]
        tags: std::collections::HashMap<String, String>,
    }
    let raw: Output =
        serde_json::from_slice(json).map_err(|e| ProbeError::Unparseable(e.to_string()))?;
    Ok(raw
        .streams
        .into_iter()
        .filter(|s| s.codec_type.as_deref() == Some("attachment"))
        .filter_map(|s| {
            let file_name = s.tags.get("filename").cloned().unwrap_or_default();
            let mime = s.tags.get("mimetype").map(|m| m.to_lowercase());
            let mime_type = mime
                .as_deref()
                .and_then(font_mime_from_label)
                .or_else(|| font_mime_from_name(&file_name))?;
            Some(FontAttachment {
                index: s.index,
                file_name,
                mime_type,
                size_bytes: s.extradata_size,
            })
        })
        .collect())
}

fn font_mime_from_label(mime: &str) -> Option<&'static str> {
    match mime {
        "font/ttf"
        | "font/sfnt"
        | "application/x-truetype-font"
        | "application/x-font-ttf"
        | "application/font-sfnt" => Some("font/ttf"),
        "font/otf"
        | "application/vnd.ms-opentype"
        | "application/x-font-otf"
        | "application/x-font-opentype" => Some("font/otf"),
        "font/collection" => Some("font/collection"),
        "font/woff" | "application/font-woff" => Some("font/woff"),
        "font/woff2" => Some("font/woff2"),
        _ => None,
    }
}

fn font_mime_from_name(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "ttf" => Some("font/ttf"),
        "otf" => Some("font/otf"),
        "ttc" | "otc" => Some("font/collection"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_are_kept_by_label_or_extension() {
        let json = br#"{"streams":[
            {"index":3,"codec_type":"attachment","extradata_size":933972,
             "tags":{"filename":"SitkaB.ttc","mimetype":"font/collection"}},
            {"index":4,"codec_type":"attachment","extradata_size":10,
             "tags":{"filename":"Arial.TTF","mimetype":"application/octet-stream"}},
            {"index":5,"codec_type":"attachment",
             "tags":{"filename":"x","mimetype":"application/x-truetype-font"}},
            {"index":6,"codec_type":"attachment","tags":{"filename":"cover.jpg","mimetype":"image/jpeg"}},
            {"index":7,"codec_type":"subtitle"}
        ]}"#;
        let fonts = parse_fonts(json).unwrap();
        let summary: Vec<_> = fonts
            .iter()
            .map(|f| (f.index, f.file_name.as_str(), f.mime_type, f.size_bytes))
            .collect();
        assert_eq!(
            summary,
            [
                (3, "SitkaB.ttc", "font/collection", Some(933972)),
                (4, "Arial.TTF", "font/ttf", Some(10)),
                (5, "x", "font/ttf", None),
            ]
        );
    }

    #[test]
    fn font_types_are_sniffed_from_magic_bytes() {
        assert_eq!(font_mime_type(b"ttcf\0\x02"), Some("font/collection"));
        assert_eq!(font_mime_type(&[0, 1, 0, 0, 0, 9]), Some("font/ttf"));
        assert_eq!(font_mime_type(b"OTTO"), Some("font/otf"));
        assert_eq!(font_mime_type(b"wOF2...."), Some("font/woff2"));
        assert_eq!(font_mime_type(b"\x89PNG"), None);
        assert_eq!(font_mime_type(b"tt"), None);
    }
}
