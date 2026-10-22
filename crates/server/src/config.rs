//! Configuration: `nuvora.toml` plus CLI overrides, validated into the types
//! the catalog takes. The only place that reads files, flags or env for config.
//! Every flag also reads an environment variable: `NUVORA_CONFIG`,
//! `NUVORA_BIND`, `NUVORA_DATA_DIR`, and `NUVORA_MOVIES` (a path list,
//! `:`-separated, `;` on Windows).
//!
//! The config file and `--movies` only add libraries and folders that do not
//! exist yet; after that the admin panel owns them.
//!
//! ```toml
//! # ~/.config/nuvora/nuvora.toml (Linux), ~/Library/Application Support/Nuvora/nuvora.toml (macOS),
//! # %APPDATA%\Nuvora\nuvora.toml (Windows). All keys optional.
//! bind = "0.0.0.0:8640"            # every interface, so phones and TVs can connect
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

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use nuvora_library::{LibraryKind, LibrarySpec};
use serde::Deserialize;

pub const DEFAULT_BIND: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8640);
const PATH_LIST_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };
const MOVIES_KEY: &str = "movies";

#[derive(clap::Args, Debug, Default)]
pub struct ServeArgs {
    /// Config file (default: the platform config dir's nuvora.toml, if present).
    #[arg(long, env = "NUVORA_CONFIG")]
    pub config: Option<PathBuf>,
    /// Listen address (default: 0.0.0.0:8640).
    #[arg(long, env = "NUVORA_BIND")]
    pub bind: Option<SocketAddr>,
    #[arg(long, env = "NUVORA_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    /// Add a movie folder without a config file: `nuvora --movies ~/Movies`.
    /// Folders join the library with key "movies" (created if absent).
    #[arg(
        long = "movies",
        value_name = "DIR",
        env = "NUVORA_MOVIES",
        value_delimiter = PATH_LIST_SEPARATOR
    )]
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
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("library {key:?}: folder {path} does not exist")]
    MissingFolder { key: String, path: PathBuf },
    #[error("two libraries share the key {0:?}")]
    DuplicateKey(String),
    #[error("library {key:?}: kind {kind:?} is not supported (only \"movies\")")]
    UnknownKind { key: String, kind: String },
    #[error("no data directory: pass --data-dir or set data_dir in nuvora.toml")]
    NoDataDir,
}

/// `nuvora.toml` as written. Unknown keys are errors so a typo cannot silently
/// disable a setting.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    bind: Option<SocketAddr>,
    data_dir: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    #[serde(default)]
    library: Vec<FileLibrary>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLibrary {
    key: String,
    name: Option<String>,
    kind: Option<String>,
    paths: Vec<PathBuf>,
}

/// File, then flags on top. Canonicalizes library paths, rejects duplicates.
pub fn load(args: &ServeArgs) -> Result<Config, ConfigError> {
    let dirs = directories::ProjectDirs::from("", "", "Nuvora");
    let path = args.config.clone().or_else(|| {
        dirs.as_ref()
            .map(|d| d.config_dir().join("nuvora.toml"))
            .filter(|p| p.is_file())
    });
    let file = match &path {
        Some(path) => read_file(path)?,
        None => FileConfig::default(),
    };

    let mut libraries = Vec::with_capacity(file.library.len() + 1);
    for lib in file.library {
        if libraries.iter().any(|l: &LibrarySpec| l.key == lib.key) {
            return Err(ConfigError::DuplicateKey(lib.key));
        }
        let kind = match lib.kind.as_deref().unwrap_or(MOVIES_KEY) {
            MOVIES_KEY => LibraryKind::Movies,
            other => {
                return Err(ConfigError::UnknownKind {
                    key: lib.key,
                    kind: other.to_owned(),
                });
            }
        };
        libraries.push(LibrarySpec {
            name: lib.name.unwrap_or_else(|| lib.key.clone()),
            key: lib.key,
            kind,
            roots: lib.paths,
        });
    }
    if !args.movies.is_empty() {
        match libraries.iter_mut().find(|l| l.key == MOVIES_KEY) {
            Some(movies) => movies.roots.extend(args.movies.iter().cloned()),
            None => libraries.push(LibrarySpec {
                key: MOVIES_KEY.into(),
                name: "Movies".into(),
                kind: LibraryKind::Movies,
                roots: args.movies.clone(),
            }),
        }
    }
    for lib in &mut libraries {
        let mut roots = Vec::with_capacity(lib.roots.len());
        for root in &lib.roots {
            let canonical = root
                .canonicalize()
                .ok()
                .filter(|p| p.is_dir())
                .ok_or_else(|| ConfigError::MissingFolder {
                    key: lib.key.clone(),
                    path: root.clone(),
                })?;
            if !roots.contains(&canonical) {
                roots.push(canonical);
            }
        }
        lib.roots = roots;
    }
    if libraries.is_empty() {
        tracing::warn!(
            "no libraries configured; pass --movies <DIR> or add [[library]] to nuvora.toml"
        );
    }

    let bind = args.bind.or(file.bind).unwrap_or(DEFAULT_BIND);
    let data_dir = args
        .data_dir
        .clone()
        .or(file.data_dir)
        .or_else(|| dirs.map(|d| d.data_dir().to_path_buf()))
        .ok_or(ConfigError::NoDataDir)?;
    Ok(Config {
        bind,
        data_dir,
        libraries,
        ffmpeg: file.ffmpeg,
        ffprobe: file.ffprobe,
    })
}

fn read_file(path: &Path) -> Result<FileConfig, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(config: Option<PathBuf>, movies: Vec<PathBuf>) -> ServeArgs {
        ServeArgs {
            config,
            bind: None,
            data_dir: Some("/tmp/nuvora-test-data".into()),
            movies,
        }
    }

    #[test]
    fn flags_alone_make_a_movies_library() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(&args(None, vec![dir.path().to_path_buf()])).unwrap();
        assert_eq!(config.bind, DEFAULT_BIND);
        assert_eq!(config.libraries.len(), 1);
        assert_eq!(
            (
                config.libraries[0].key.as_str(),
                config.libraries[0].name.as_str()
            ),
            ("movies", "Movies")
        );
        assert_eq!(
            config.libraries[0].roots,
            [dir.path().canonicalize().unwrap()]
        );
    }

    #[test]
    fn file_then_flags_on_top() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let toml = dir.path().join("nuvora.toml");
        std::fs::write(
            &toml,
            format!(
                "bind = \"0.0.0.0:9000\"\nffmpeg = \"/opt/ffmpeg\"\n\n[[library]]\nkey = \"movies\"\nname = \"Films\"\nkind = \"movies\"\npaths = [{:?}]\n",
                a.to_str().unwrap()
            ),
        )
        .unwrap();
        let config = load(&args(Some(toml), vec![b.clone(), b.clone()])).unwrap();
        assert_eq!(config.bind.port(), 9000);
        assert_eq!(config.ffmpeg.as_deref(), Some(Path::new("/opt/ffmpeg")));
        assert_eq!(config.libraries[0].name, "Films");
        assert_eq!(
            config.libraries[0].roots,
            [a.canonicalize().unwrap(), b.canonicalize().unwrap()],
            "flag folders join, deduplicated"
        );
    }

    #[test]
    fn every_flag_reads_an_environment_variable() {
        use clap::{CommandFactory, Parser};
        let command = crate::Cli::command();
        let envs: Vec<_> = command
            .get_arguments()
            .filter_map(|a| a.get_env()?.to_str())
            .collect();
        assert_eq!(
            envs,
            [
                "NUVORA_CONFIG",
                "NUVORA_BIND",
                "NUVORA_DATA_DIR",
                "NUVORA_MOVIES"
            ]
        );
        if cfg!(unix) {
            let cli =
                crate::Cli::try_parse_from(["nuvora", "--movies", "/media/a:/media/b"]).unwrap();
            assert_eq!(
                cli.serve.movies,
                [PathBuf::from("/media/a"), PathBuf::from("/media/b")],
                "a path list splits like NUVORA_MOVIES does"
            );
        }
    }

    #[test]
    fn rejects_missing_folders_duplicate_keys_unknown_kinds_and_typos() {
        let dir = tempfile::tempdir().unwrap();
        let missing = load(&args(None, vec![dir.path().join("nope")]));
        assert!(
            matches!(missing, Err(ConfigError::MissingFolder { .. })),
            "{missing:?}"
        );

        let toml = dir.path().join("dup.toml");
        std::fs::write(
            &toml,
            "[[library]]\nkey = \"m\"\npaths = []\n[[library]]\nkey = \"m\"\npaths = []\n",
        )
        .unwrap();
        assert!(
            matches!(load(&args(Some(toml), vec![])), Err(ConfigError::DuplicateKey(k)) if k == "m")
        );

        let toml = dir.path().join("kind.toml");
        std::fs::write(
            &toml,
            "[[library]]\nkey = \"s\"\nkind = \"shows\"\npaths = []\n",
        )
        .unwrap();
        assert!(matches!(
            load(&args(Some(toml), vec![])),
            Err(ConfigError::UnknownKind { .. })
        ));

        let toml = dir.path().join("typo.toml");
        std::fs::write(&toml, "bindd = \"127.0.0.1:1\"\n").unwrap();
        assert!(matches!(
            load(&args(Some(toml), vec![])),
            Err(ConfigError::Parse { .. })
        ));
    }
}
