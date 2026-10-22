//! Everything Nuvora knows about a media file that does not depend on state:
//! what the file is (by name), what is inside it (ffprobe), what its artwork
//! looks like at the sizes the UI uses, whether a device can play it, and its
//! subtitles and fonts as files a player can use.
//!
//! No database, no HTTP, no global state. Each public function takes a path or
//! bytes and returns a domain value, so each is tested against real files.
//! ffprobe's JSON shape and the image codecs stay private to this crate.

mod artwork;
mod hls;
mod name;
mod playback;
mod probe;
mod sidecar;
mod subtitles;
#[cfg(test)]
mod test_fixtures;
mod tools;

pub use artwork::{
    ArtworkError, ArtworkRole, ArtworkSource, Frame, ImageSize, PIPELINE_VERSION, Placeholder,
    RenderedArtwork,
};
pub use hls::{HlsError, INIT_SEGMENT, Run, SEGMENT_SECONDS, Segments, keyframes, landing};
pub use name::{
    EpisodeName, IdentityKey, MovieName, PARSE_VERSION, PathClass, ShowName, classify_path,
    fold_for_sort, is_skipped_dir,
};
pub use playback::{
    AudioOutput, AudioPlan, AudioSupport, DeviceProfile, Incompatibility, PlaybackDecision,
    StreamPlan, VideoPlan, VideoSupport,
};
pub use probe::{
    AudioCodec, AudioStream, ColorInfo, Container, HdrFormat, MediaInfo, PROBE_VERSION, ProbeError,
    SubtitleCodec, SubtitleStream, VideoCodec, VideoStream, probe,
};
pub use sidecar::{SIDECAR_EXTENSIONS, SidecarName, sidecar_codec, sidecar_name};
pub use subtitles::{
    ExtractError, FontAttachment, SubtitleText, dump_attachment, extract_subtitle,
    font_attachments, font_mime_type,
};
pub use tools::{MediaTools, ToolsError};
