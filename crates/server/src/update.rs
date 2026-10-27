//! Updating the server from the web app (Settings › General).
//!
//! The newest version is GitHub's latest release, asked for when an admin
//! looks and at most every few hours otherwise. Installing downloads that
//! release's server binary for this platform (`nuvora-server-<target>`),
//! checks it against the release's `SHA256SUMS` and that it starts, and puts
//! it in place: over this binary when its folder is writable (a native
//! install), else in `<data dir>/bin` (the Docker image, whose binary belongs
//! to root). Then the server stops and starts again as the new binary, with
//! the same arguments. At startup a newer binary in `<data dir>/bin` takes
//! over, so a staged update survives a container restart, and a newer image
//! wins over an older staged binary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::watch;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LATEST_RELEASE: &str = "https://api.github.com/repos/IuCC123/nuvora/releases/latest";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// An admin's "Check again" asks GitHub at most this often (60 requests an hour without a token).
const RECHECK_AFTER: Duration = Duration::from_secs(60);
const STAGED_PREFIX: &str = "nuvora-";

/// The release asset with this platform's server, e.g. `nuvora-server-aarch64-apple-darwin`.
pub fn asset_name() -> String {
    format!(
        "nuvora-server-{}{}",
        env!("NUVORA_TARGET"),
        std::env::consts::EXE_SUFFIX
    )
}

/// `major.minor.patch`; a pre-release or build suffix is ignored.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Version(u64, u64, u64);

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let core = text
            .trim()
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()?;
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let version = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }

    pub fn current() -> Version {
        Version::parse(VERSION).expect("the crate version is major.minor.patch")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    /// The release page, with what changed.
    pub page: String,
    /// This platform's binary, when the release has one.
    binary: Option<String>,
    checksums: Option<String>,
}

impl Release {
    pub fn installable(&self) -> bool {
        self.binary.is_some() && self.checksums.is_some()
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Downloading,
    /// Installed; the server restarts in a moment.
    Restarting,
    Failed(String),
}

pub struct Updater {
    /// `None`: checks are off (`--updates false`).
    feed: Option<String>,
    exe: PathBuf,
    data_dir: PathBuf,
    http: reqwest::Client,
    checked: tokio::sync::Mutex<Option<(Instant, Result<Release, String>)>>,
    phase: Mutex<Phase>,
    restart: watch::Sender<Option<PathBuf>>,
}

impl Updater {
    /// `feed` is the GitHub API URL of the latest release; `exe`, the binary to replace.
    pub fn new(feed: Option<String>, exe: PathBuf, data_dir: PathBuf) -> Arc<Updater> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("nuvora/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("HTTP client builds");
        Arc::new(Updater {
            feed,
            exe,
            data_dir,
            http,
            checked: tokio::sync::Mutex::new(None),
            phase: Mutex::new(Phase::Idle),
            restart: watch::channel(None).0,
        })
    }

    pub fn enabled(&self) -> bool {
        self.feed.is_some()
    }

    pub fn phase(&self) -> Phase {
        self.phase.lock().unwrap().clone()
    }

    /// The binary to restart as, once an update is in place.
    pub fn restarts(&self) -> watch::Receiver<Option<PathBuf>> {
        self.restart.subscribe()
    }

    /// The newest release, from cache unless it is old (or `refresh` and a minute old).
    pub async fn latest(&self, refresh: bool) -> Option<Result<Release, String>> {
        let feed = self.feed.as_deref()?;
        let mut checked = self.checked.lock().await;
        let fresh = checked.as_ref().is_some_and(|(at, _)| {
            at.elapsed() < if refresh { RECHECK_AFTER } else { CHECK_EVERY }
        });
        if !fresh {
            *checked = Some((Instant::now(), self.fetch(feed).await));
        }
        checked.as_ref().map(|(_, release)| release.clone())
    }

    async fn fetch(&self, feed: &str) -> Result<Release, String> {
        let unreachable = |e: reqwest::Error| {
            tracing::warn!("cannot check for updates: {e}");
            "Couldn't reach GitHub to check for updates.".to_owned()
        };
        let release: GithubRelease = self
            .http
            .get(feed)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(unreachable)?
            .json()
            .await
            .map_err(unreachable)?;
        let version = Version::parse(&release.tag_name)
            .ok_or_else(|| format!("The newest release has an odd name: {}.", release.tag_name))?;
        let asset = |name: &str| {
            release
                .assets
                .iter()
                .find(|a| a.name == name)
                .map(|a| a.browser_download_url.clone())
        };
        Ok(Release {
            version,
            page: release.html_url.clone(),
            binary: asset(&asset_name()),
            checksums: asset("SHA256SUMS"),
        })
    }

    /// Starts installing `release` in the background. False when an install is already running.
    pub fn install(self: &Arc<Self>, release: Release) -> bool {
        {
            let mut phase = self.phase.lock().unwrap();
            if matches!(*phase, Phase::Downloading | Phase::Restarting) {
                return false;
            }
            *phase = Phase::Downloading;
        }
        let updater = self.clone();
        tokio::spawn(async move {
            let placed = updater.download_and_place(&release).await;
            let mut phase = updater.phase.lock().unwrap();
            match placed {
                Ok(exe) => {
                    tracing::info!(version = %release.version, exe = %exe.display(), "update installed; restarting");
                    *phase = Phase::Restarting;
                    updater.restart.send_replace(Some(exe));
                }
                Err(message) => {
                    tracing::warn!(version = %release.version, "update failed: {message}");
                    *phase = Phase::Failed(message);
                }
            }
        });
        true
    }

    async fn download_and_place(&self, release: &Release) -> Result<PathBuf, String> {
        let (Some(binary), Some(checksums)) = (&release.binary, &release.checksums) else {
            return Err("This release has no server download for this computer.".into());
        };
        let failed = |e: reqwest::Error| {
            tracing::warn!("update download failed: {e}");
            "The download failed. Check the server's internet connection, then try again."
                .to_owned()
        };
        let sums = self.get(checksums).await.map_err(failed)?;
        let expected = expected_sha256(&String::from_utf8_lossy(&sums), &asset_name())
            .ok_or("The release's checksums don't list this computer's download.")?;
        let bytes = self.get(binary).await.map_err(failed)?;
        if hex(&Sha256::digest(&bytes)) != expected {
            return Err("The download was damaged (its checksum doesn't match). Try again.".into());
        }
        let (exe, staging, version) =
            (self.exe.clone(), self.data_dir.join("bin"), release.version);
        tokio::task::spawn_blocking(move || place(&exe, &staging, version, &bytes))
            .await
            .map_err(|e| e.to_string())?
    }

    async fn get(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
        let response = self
            .http
            .get(url)
            .timeout(Duration::from_secs(600))
            .send()
            .await?
            .error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }
}

/// The hash `SHA256SUMS` (`sha256sum` output) lists for `name`.
fn expected_sha256(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes the new binary next to `exe` and swaps it in, or, when that folder
/// isn't writable or `exe` is itself a staged update, into `staging`. Either
/// way the binary must answer `--version` with `version` first.
fn place(exe: &Path, staging: &Path, version: Version, bytes: &[u8]) -> Result<PathBuf, String> {
    let beside = exe.parent().filter(|_| !exe.starts_with(staging));
    if let Some(dir) = beside {
        let download = dir.join(format!(".nuvora-{version}.download"));
        match write_executable(&download, bytes) {
            Ok(()) => {
                check_runs(&download, version)?;
                swap_in(&download, exe)
                    .map_err(|e| format!("Couldn't replace {}: {e}", exe.display()))?;
                return Ok(exe.to_path_buf());
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                ) => {}
            Err(e) => {
                return Err(format!(
                    "Couldn't save the update next to {}: {e}",
                    exe.display()
                ));
            }
        }
    }
    let saving =
        |e: std::io::Error| format!("Couldn't save the update in {}: {e}", staging.display());
    std::fs::create_dir_all(staging).map_err(saving)?;
    let download = staging.join(format!(".nuvora-{version}.download"));
    write_executable(&download, bytes).map_err(saving)?;
    check_runs(&download, version)?;
    let target = staged_path(staging, version);
    std::fs::rename(&download, &target).map_err(saving)?;
    Ok(target)
}

fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// A binary for another processor or an older system fails here, before it replaces anything.
fn check_runs(binary: &Path, version: Version) -> Result<(), String> {
    let output = std::process::Command::new(binary).arg("--version").output();
    let says = output
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    match says {
        Ok(text)
            if Version::parse(text.trim().trim_start_matches("nuvora").trim()) == Some(version) =>
        {
            Ok(())
        }
        _ => {
            let _ = std::fs::remove_file(binary);
            Err(format!(
                "Nuvora {version} doesn't run on this computer, so it wasn't installed."
            ))
        }
    }
}

/// Renaming over a running binary is fine on Unix: the running process keeps
/// the old file. Windows won't replace a running .exe but lets it be renamed.
fn swap_in(new: &Path, exe: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let old = exe.with_extension("exe.old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old)?;
    }
    std::fs::rename(new, exe)
}

fn staged_path(staging: &Path, version: Version) -> PathBuf {
    staging.join(format!(
        "{STAGED_PREFIX}{version}{}",
        std::env::consts::EXE_SUFFIX
    ))
}

/// At startup: the newest binary in `<data dir>/bin` when it is newer than this
/// one. Older ones are deleted, and so is a Windows `.exe.old` left by an update.
pub fn staged(data_dir: &Path) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("exe.old"));
    }
    let staging = data_dir.join("bin");
    let current = Version::current();
    let mut newest: Option<(Version, PathBuf)> = None;
    for entry in std::fs::read_dir(&staging).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(version) = name
            .strip_prefix(STAGED_PREFIX)
            .map(|rest| rest.trim_end_matches(std::env::consts::EXE_SUFFIX))
            .and_then(Version::parse)
        else {
            continue;
        };
        if version < current {
            let _ = std::fs::remove_file(entry.path());
        } else if version > current && newest.as_ref().is_none_or(|(v, _)| version > *v) {
            newest = Some((version, entry.path()));
        }
    }
    newest.map(|(_, path)| path)
}

/// Becomes `exe`, with this process's arguments. On Unix the process is
/// replaced in place (same pid, so launchd, systemd and Docker's tini carry
/// on); on Windows the new one starts and this one exits.
pub fn restart_as(exe: &Path) -> ! {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut command = std::process::Command::new(exe);
    command.args(&args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        tracing::error!("cannot start {}: {error}", exe.display());
        std::process::exit(1);
    }
    #[cfg(not(unix))]
    {
        match command.spawn() {
            Ok(_) => std::process::exit(0),
            Err(error) => {
                tracing::error!("cannot start {}: {error}", exe.display());
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(Version::parse("v0.10.0").unwrap() > Version::parse("0.9.9").unwrap());
        assert_eq!(Version::parse("1.2.3-rc.1"), Version::parse("1.2.3"));
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse("nuvora"), None);
    }

    #[test]
    fn checksums_are_found_by_file_name() {
        let sums = "AB12  nuvora-server-x86_64-unknown-linux-gnu\ncd34 *nuvora-server-aarch64-apple-darwin\n";
        assert_eq!(
            expected_sha256(sums, "nuvora-server-aarch64-apple-darwin").as_deref(),
            Some("cd34")
        );
        assert_eq!(
            expected_sha256(sums, "nuvora-server-x86_64-unknown-linux-gnu").as_deref(),
            Some("ab12")
        );
        assert_eq!(
            expected_sha256(sums, "nuvora-server-x86_64-pc-windows-msvc.exe"),
            None
        );
    }

    /// A GitHub stand-in serving one release, `v99.0.0`, whose binary is a
    /// script that answers `--version` like Nuvora 99.0.0 would.
    #[cfg(unix)]
    async fn fake_github(sums: impl Fn(&str) -> String) -> (String, Vec<u8>) {
        use axum::routing::get;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let binary = b"#!/bin/sh\necho 'nuvora 99.0.0'\n".to_vec();
        let release = serde_json::json!({
            "tag_name": "v99.0.0",
            "html_url": "https://github.com/IuCC123/nuvora/releases/tag/v99.0.0",
            "assets": [
                {"name": asset_name(), "browser_download_url": format!("{base}/binary")},
                {"name": "SHA256SUMS", "browser_download_url": format!("{base}/sums")},
            ],
        });
        let sums = sums(&hex(&Sha256::digest(&binary)));
        let served = binary.clone();
        let app = axum::Router::new()
            .route("/release", get(move || async move { axum::Json(release) }))
            .route("/binary", get(move || async move { served }))
            .route("/sums", get(move || async move { sums }));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("{base}/release"), binary)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn installing_replaces_the_binary_then_asks_for_a_restart() {
        let (feed, binary) = fake_github(|hash| format!("{hash}  {}\n", asset_name())).await;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("nuvora");
        std::fs::write(&exe, b"the old server").unwrap();
        let updater = Updater::new(Some(feed), exe.clone(), dir.path().join("data"));

        let release = updater.latest(false).await.unwrap().unwrap();
        assert_eq!(release.version, Version(99, 0, 0));
        assert!(release.installable());

        let mut restarts = updater.restarts();
        assert!(updater.install(release));
        let restart_as =
            tokio::time::timeout(Duration::from_secs(10), restarts.wait_for(Option::is_some))
                .await
                .unwrap()
                .unwrap()
                .clone();
        assert_eq!(restart_as, Some(exe.clone()));
        assert_eq!(std::fs::read(&exe).unwrap(), binary);
        assert_eq!(updater.phase(), Phase::Restarting);
    }

    /// The Docker image: the binary's folder belongs to root, so the update
    /// goes to `<data dir>/bin`, where the next start picks it up.
    #[cfg(unix)]
    #[tokio::test]
    async fn without_write_access_to_the_binary_the_update_is_staged_in_the_data_dir() {
        use std::os::unix::fs::PermissionsExt;
        let (feed, binary) = fake_github(|hash| format!("{hash}  {}\n", asset_name())).await;
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("usr-local-bin");
        std::fs::create_dir(&bin_dir).unwrap();
        let exe = bin_dir.join("nuvora");
        std::fs::write(&exe, b"the image's server").unwrap();
        std::fs::set_permissions(&bin_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(bin_dir.join("probe"), b"").is_ok() {
            return; // root writes anywhere; CI runs tests as an ordinary user
        }
        let data = dir.path().join("data");
        let updater = Updater::new(Some(feed), exe.clone(), data.clone());

        let mut restarts = updater.restarts();
        assert!(updater.install(updater.latest(false).await.unwrap().unwrap()));
        let restart_as =
            tokio::time::timeout(Duration::from_secs(10), restarts.wait_for(Option::is_some))
                .await
                .unwrap()
                .unwrap()
                .clone();
        std::fs::set_permissions(&bin_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        let staged_binary = staged_path(&data.join("bin"), Version(99, 0, 0));
        assert_eq!(restart_as, Some(staged_binary.clone()));
        assert_eq!(std::fs::read(&staged_binary).unwrap(), binary);
        assert_eq!(std::fs::read(&exe).unwrap(), b"the image's server");
        assert_eq!(staged(&data), Some(staged_binary));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_damaged_download_is_not_installed() {
        let (feed, _) = fake_github(|_| format!("{}  {}\n", "0".repeat(64), asset_name())).await;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("nuvora");
        std::fs::write(&exe, b"the old server").unwrap();
        let updater = Updater::new(Some(feed), exe.clone(), dir.path().join("data"));

        let release = updater.latest(false).await.unwrap().unwrap();
        assert!(updater.install(release));
        let failed = async {
            loop {
                if let Phase::Failed(message) = updater.phase() {
                    return message;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let message = tokio::time::timeout(Duration::from_secs(10), failed)
            .await
            .unwrap();
        assert!(message.contains("damaged"), "{message}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"the old server");
        assert_eq!(*updater.restarts().borrow(), None);
    }

    #[test]
    fn a_newer_staged_binary_takes_over_and_older_ones_go() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let current = Version::current();
        let older = Version(0, 0, 1);
        let newer = Version(current.0 + 1, 0, 0);
        let newest = Version(current.0 + 2, 0, 0);
        for v in [older, current, newer, newest] {
            std::fs::write(staged_path(&bin, v), b"").unwrap();
        }
        assert_eq!(staged(dir.path()), Some(staged_path(&bin, newest)));
        assert!(!staged_path(&bin, older).exists());
        assert!(
            staged_path(&bin, current).exists(),
            "the running binary may be this one"
        );
        assert_eq!(staged(&dir.path().join("nothing")), None);
    }
}
