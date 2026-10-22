//! Configuration: `nuvora.toml` plus CLI overrides, validated into the types
//! the catalog takes. The only place that reads files, flags or env for config.
//!
//! ```toml
//! # ~/.config/nuvora/nuvora.toml (Linux), ~/Library/Application Support/Nuvora/nuvora.toml (macOS),
//! # %APPDATA%\Nuvora\nuvora.toml (Windows). All keys optional.
//! bind = "127.0.0.1:8640"          # loopback by default: Step 1 has no auth
//! data_dir = "/var/lib/nuvora"     # default: the platform data dir
//! ffmpeg = "/usr/bin/ffmpeg"       # default: PATH
//! ffprobe = "/usr/bin/ffprobe"
//!
//! [[library]]
//! key = "movies"                   # stable identity; renaming `name` keeps ids
//! name = "Movies"
//! kind = "movies"
//! paths = ["/Volumes/Media/Movies"]
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;

use nuvora_library::LibrarySpec;

#[derive(clap::Args, Debug, Default)]
pub struct ServeArgs {
    /// Config file (default: the platform config dir's nuvora.toml, if present).
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub bind: Option<SocketAddr>,
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Add a movie folder without a config file: `nuvora --movies ~/Movies`.
    /// Folders join the library with key "movies" (created if absent).
    #[arg(long = "movies", value_name = "DIR")]
    pub movies: Vec<PathBuf>,
}

/// Validated configuration.
#[derive(Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub libraries: Vec<LibrarySpec>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("{path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("library {key:?}: folder {path} does not exist")]
    MissingFolder { key: String, path: PathBuf },
    #[error("two libraries share the key {0:?}")]
    DuplicateKey(String),
}

/// File, then flags on top. Canonicalizes library paths, rejects duplicates,
/// logs a warning when `bind` is not a loopback address.
pub fn load(args: &ServeArgs) -> Result<Config, ConfigError> {
    let _ = args;
    todo!()
}
