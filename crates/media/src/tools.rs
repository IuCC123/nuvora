use std::path::PathBuf;
use std::time::Duration;

/// Resolved ffprobe/ffmpeg executables. Constructed once at startup; every
/// subprocess call in this crate goes through it, so timeouts and binary paths
/// have one owner.
#[derive(Debug, Clone)]
pub struct MediaTools {
    pub(crate) ffprobe: PathBuf,
    pub(crate) ffmpeg: PathBuf,
    /// Kill ffprobe after this long. NAS shares can stall on a bad sector; one
    /// stuck file must not stall the scan.
    pub(crate) probe_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolsError {
    #[error("{tool} not found (looked for {looked_for}); install FFmpeg or set its path in nuvora.toml")]
    NotFound { tool: &'static str, looked_for: PathBuf },
    #[error("{tool} at {path} did not run: {source}")]
    Broken { tool: &'static str, path: PathBuf, source: std::io::Error },
}

impl MediaTools {
    /// Find the binaries (explicit path, else `PATH`) and run `-version` on each.
    /// Fails fast at startup with a message a user can act on.
    pub async fn locate(ffprobe: Option<PathBuf>, ffmpeg: Option<PathBuf>) -> Result<Self, ToolsError> {
        let _ = (ffprobe, ffmpeg);
        todo!("resolve each binary, run `-version`, keep probe_timeout = 30s")
    }
}
