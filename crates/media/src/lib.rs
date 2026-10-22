//! Everything Nuvora knows about a media file that does not depend on state:
//! what the file is (by name), what is inside it (ffprobe), what its artwork
//! looks like at the sizes the UI uses, and whether a device can play it.
//!
//! No database, no HTTP, no global state. Each public function takes a path or
//! bytes and returns a domain value, so each is tested against real files.
//! ffprobe's JSON shape and the image codecs stay private to this crate.

mod artwork;
mod name;
mod playback;
mod probe;
mod tools;

pub use artwork::{
    ArtworkError, ArtworkRole, ArtworkSource, Frame, ImageSize, PIPELINE_VERSION, Placeholder,
    RenderedArtwork,
};
pub use name::{IdentityKey, MovieName, PathClass, classify_path};
pub use playback::{AudioSupport, DeviceProfile, Incompatibility, PlaybackDecision, VideoSupport};
pub use probe::{
    AudioCodec, AudioStream, ColorInfo, Container, HdrFormat, MediaInfo, PROBE_VERSION, ProbeError,
    SubtitleCodec, SubtitleStream, VideoCodec, VideoStream,
};
pub use tools::{MediaTools, ToolsError};
