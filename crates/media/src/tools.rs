use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

/// Resolved ffprobe/ffmpeg executables. Constructed once at startup; every
/// subprocess call in this crate goes through it, so timeouts and binary paths
/// have one owner.
#[derive(Debug, Clone)]
pub struct MediaTools {
    pub(crate) ffprobe: PathBuf,
    pub(crate) ffmpeg: PathBuf,
    ffprobe_version: String,
    ffmpeg_version: String,
    /// Kill ffprobe after this long. NAS shares can stall on a bad sector; one
    /// stuck file must not stall the scan.
    pub(crate) probe_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolsError {
    #[error(
        "{tool} not found (looked for {looked_for}); install FFmpeg or set its path in nuvora.toml"
    )]
    NotFound {
        tool: &'static str,
        looked_for: PathBuf,
    },
    #[error("{tool} at {path} did not run: {source}")]
    Broken {
        tool: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl MediaTools {
    /// Find the binaries (explicit path, else `PATH`) and run `-version` on each.
    /// Fails fast at startup with a message a user can act on.
    pub async fn locate(
        ffprobe: Option<PathBuf>,
        ffmpeg: Option<PathBuf>,
    ) -> Result<Self, ToolsError> {
        let (ffprobe, ffprobe_version) =
            check("ffprobe", ffprobe.unwrap_or_else(|| "ffprobe".into())).await?;
        let (ffmpeg, ffmpeg_version) =
            check("ffmpeg", ffmpeg.unwrap_or_else(|| "ffmpeg".into())).await?;
        Ok(MediaTools {
            ffprobe,
            ffmpeg,
            ffprobe_version,
            ffmpeg_version,
            probe_timeout: Duration::from_secs(30),
        })
    }

    /// As configured: a bare name means "found on `PATH`".
    pub fn ffprobe(&self) -> &Path {
        &self.ffprobe
    }

    pub fn ffmpeg(&self) -> &Path {
        &self.ffmpeg
    }

    /// The third word of `ffprobe -version`'s first line, e.g. `"7.1.1"`.
    pub fn ffprobe_version(&self) -> &str {
        &self.ffprobe_version
    }

    pub fn ffmpeg_version(&self) -> &str {
        &self.ffmpeg_version
    }
}

/// `"ffmpeg version 7.1.1 Copyright ..."` => `"7.1.1"`.
fn parse_version(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(2))
        .unwrap_or("unknown")
        .to_owned()
}

async fn check(tool: &'static str, path: PathBuf) -> Result<(PathBuf, String), ToolsError> {
    let output = Command::new(&path)
        .arg("-version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await;
    match output {
        Ok(output) if output.status.success() => Ok((path, parse_version(&output.stdout))),
        Ok(output) => Err(ToolsError::Broken {
            tool,
            path,
            source: std::io::Error::other(format!("`-version` exited with {}", output.status)),
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Err(ToolsError::NotFound {
            tool,
            looked_for: path,
        }),
        Err(source) => Err(ToolsError::Broken { tool, path, source }),
    }
}
