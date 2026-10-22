//! Everything Nuvora knows about a media file that does not depend on state:
//! what the file is (by name), what is inside it (ffprobe), what its artwork
//! looks like at the sizes the UI uses, whether a device can play it, and its
//! subtitles and fonts as files a player can use.
//!
//! No database, no HTTP, no global state. Each public function takes a path or
//! bytes and returns a domain value, so each is tested against real files.
//! ffprobe's JSON shape and the image codecs stay private to this crate.

mod artwork;
mod name;
mod playback;
mod probe;
mod subtitles;
#[cfg(test)]
mod test_fixtures;
mod tools;

pub use artwork::{
    ArtworkError, ArtworkRole, ArtworkSource, Frame, ImageSize, PIPELINE_VERSION, Placeholder,
    RenderedArtwork,
};
pub use name::{
    EpisodeName, IdentityKey, MovieName, PARSE_VERSION, PathClass, ShowName, classify_path,
    fold_for_sort, is_skipped_dir,
};
pub use playback::{AudioSupport, DeviceProfile, Incompatibility, PlaybackDecision, VideoSupport};
pub use probe::{
    AudioCodec, AudioStream, ColorInfo, Container, HdrFormat, MediaInfo, PROBE_VERSION, ProbeError,
    SubtitleCodec, SubtitleStream, VideoCodec, VideoStream, probe,
};
pub use subtitles::{
    ExtractError, FontAttachment, SubtitleText, dump_attachment, extract_subtitle,
    font_attachments, font_mime_type,
};
pub use tools::{MediaTools, ToolsError};
