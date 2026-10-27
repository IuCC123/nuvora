//! Downloads: search the admin's Torznab indexers for a movie, download it
//! over BitTorrent inside the server, and move it into a movie library.
//!
//! A download writes to a hidden folder inside the library's first folder
//! (`.nuvora-downloads/<id>/`), which the scanner skips, so a half-written
//! file never shows up as a broken movie. Only video and subtitle files are
//! fetched. Once every piece is in, the torrent stops (Nuvora does not seed),
//! the files move to `<Title (Year)>/` beside the library's other movies, and
//! a scan picks them up. Staging inside the library keeps that move a rename
//! on the same disk.
//!
//! The indexers, the target library and the list of downloads are saved as
//! one JSON value in the catalog's `setting` table. A restart adds every
//! unfinished download again; the engine re-checks what is already on disk
//! and carries on from there.

pub mod torznab;

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use librqbit::dht::DhtPersistenceConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, DhtSessionConfig,
    ListenerOptions, ManagedTorrent, PeerConnectionOptions, Session, SessionOptions, TorrentStats,
    TorrentStatsState,
};
use nuvora_library::{Library, LibraryId, LibraryInfo, LibraryKind};
use nuvora_media::{EpisodeName, PathClass, ReleaseName, classify_path};
use serde::{Deserialize, Serialize};

pub use torznab::{Indexer, Release, TorznabError};

const SETTING_KEY: &str = "downloads";
/// The staging folder inside a library folder; hidden, so never scanned.
const STAGING: &str = ".nuvora-downloads";
/// BitTorrent's usual port. When it is taken, any free port is used.
const LISTEN_PORT: u16 = 6881;
/// Peers a torrent may have connected or still answering at once. librqbit's
/// 128 filled with peers that never answer, so a torrent with hundreds of
/// seeders downloaded from about twenty.
const PEER_LIMIT: usize = 400;
/// How long a peer has to answer before its place goes to another (librqbit
/// waits 10 seconds).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Files worth fetching: videos and subtitles. Everything else in a torrent
/// (`.nfo`, `.txt`, `.exe`, artwork) stays behind.
const WANTED_FILES: &str =
    r"(?i)\.(mkv|mp4|m4v|mov|webm|avi|ts|m2ts|mts|mpg|mpeg|wmv|srt|ass|ssa|vtt)$";
/// A `.torrent` bigger than this is not a movie's.
const MAX_TORRENT_BYTES: usize = 16 * 1024 * 1024;
const POLL: Duration = Duration::from_secs(1);

pub type DownloadId = u64;

/// How the BitTorrent engine finds peers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// No engine: downloads are refused.
    Off,
    /// Trackers, the DHT, local discovery and UPnP, listening on port 6881
    /// or any free one.
    Internet,
    /// Trackers on this machine only, so tests never reach the network.
    #[cfg(test)]
    Loopback,
}

pub struct Downloads {
    library: Library,
    /// `Err` says why there is no engine: it failed to start, or is off.
    engine: Result<Arc<Session>, String>,
    http: reqwest::Client,
    /// Fetches `.torrent` links, stopping at a redirect to a magnet link.
    fetcher: reqwest::Client,
    /// `<data>/downloads`: the engine's DHT state and fetched `.torrent` files.
    dir: PathBuf,
    /// The one copy of what is saved. Every change saves while holding it, so
    /// saves land in order.
    saved: tokio::sync::Mutex<Saved>,
    /// Downloads with a task working on them.
    live: Mutex<HashMap<DownloadId, Runner>>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    last_id: DownloadId,
    indexers: Vec<Indexer>,
    /// Where movies go; unset or gone, the first Movies library.
    library_id: Option<i64>,
    /// Where episodes and seasons go; unset or gone, the first TV Shows library.
    shows_library_id: Option<i64>,
    downloads: Vec<Record>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Record {
    id: DownloadId,
    title: String,
    source: Source,
    library_id: i64,
    /// The library folder the movie lands in.
    root: PathBuf,
    added_at: i64,
    phase: Phase,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Source {
    Magnet {
        uri: String,
    },
    /// An indexer's link, not fetched yet: a `.torrent`, or a redirect to a
    /// magnet link. Fetched once; afterwards one of the other two.
    Link {
        url: String,
    },
    /// Saved as `<data>/downloads/<id>.torrent`.
    TorrentFile,
}

/// What the admin asked for, and how it ended.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Phase {
    Active,
    Paused,
    Done { path: PathBuf, finished_at: i64 },
    Failed { error: String },
}

struct Runner {
    stage: Stage,
    task: tokio::task::AbortHandle,
}

/// Where a running download's task is.
enum Stage {
    /// Fetching the `.torrent`, or a magnet link's details from peers.
    Starting,
    Running(Arc<ManagedTorrent>),
    Moving,
}

/// A download as the Downloads page shows it.
pub struct Snapshot {
    pub id: DownloadId,
    pub title: String,
    pub library_id: i64,
    pub added_at: i64,
    pub state: State,
}

pub enum State {
    Starting,
    /// Re-checking files already on disk, after a restart or a retry.
    Checking(Progress),
    Downloading(Progress),
    /// `None` when paused before it started.
    Paused(Option<Progress>),
    Moving,
    Done {
        path: PathBuf,
        finished_at: i64,
    },
    Failed {
        error: String,
    },
}

pub struct Progress {
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub down_bps: u64,
    pub up_bps: u64,
    pub peers: u32,
}

pub struct Settings {
    pub indexers: Vec<Indexer>,
    pub library_id: Option<i64>,
    pub shows_library_id: Option<i64>,
}

pub struct SearchResults {
    /// Most seeders first.
    pub releases: Vec<Release>,
    /// Indexers that failed, and why.
    pub failures: Vec<(String, TorznabError)>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("Downloading isn't available: {0}")]
    NoEngine(String),
    #[error("Add a Movies library first: movies go into one")]
    NoMovieLibrary,
    #[error("Add a TV Shows library first: episodes and seasons go into one")]
    NoShowLibrary,
    #[error("Use a magnet link or a link to a .torrent file")]
    BadLink,
    #[error("Nuvora can't write to {}: {error}", .path.display())]
    NotWritable {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error(transparent)]
    Save(#[from] nuvora_library::QueryError),
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("No such download")]
    NotFound,
    #[error("This download has already finished")]
    Finished,
    #[error("Wait until it has finished moving into the library")]
    Moving,
    #[error(transparent)]
    Save(#[from] nuvora_library::QueryError),
}

impl Downloads {
    /// Starts the engine and picks up every unfinished download.
    pub async fn start(library: Library, data_dir: &Path, engine: Engine) -> Arc<Downloads> {
        let dir = data_dir.join("downloads");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("cannot create {}: {e}", dir.display());
        }
        let engine = start_engine(&dir, engine).await;
        if let Err(e) = &engine {
            tracing::warn!("downloads unavailable: {e}");
        }
        let saved = match library.setting(SETTING_KEY).await {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_else(|e| {
                tracing::warn!("saved downloads don't parse, starting empty: {e}");
                Saved::default()
            }),
            Ok(None) => Saved::default(),
            Err(e) => {
                tracing::warn!("cannot read saved downloads: {e}");
                Saved::default()
            }
        };
        let unfinished: Vec<DownloadId> = saved
            .downloads
            .iter()
            .filter(|r| matches!(r.phase, Phase::Active | Phase::Paused))
            .map(|r| r.id)
            .collect();
        let downloads = Arc::new(Downloads {
            library,
            engine,
            http: torznab::client(),
            fetcher: fetcher(),
            dir,
            saved: tokio::sync::Mutex::new(saved),
            live: Mutex::default(),
        });
        if downloads.engine.is_ok() {
            for id in unfinished {
                downloads.spawn(id);
            }
        }
        downloads
    }

    /// The engine, which torrent streams share.
    pub fn engine(&self) -> Result<Arc<Session>, String> {
        self.engine.clone()
    }

    /// Why downloading is unavailable, if it is.
    pub fn engine_error(&self) -> Option<&str> {
        self.engine.as_ref().err().map(String::as_str)
    }

    pub async fn settings(&self) -> Settings {
        let saved = self.saved.lock().await;
        Settings {
            indexers: saved.indexers.clone(),
            library_id: saved.library_id,
            shows_library_id: saved.shows_library_id,
        }
    }

    /// Checks the indexer answers before saving it.
    pub async fn add_indexer(
        &self,
        name: String,
        url: String,
        api_key: String,
    ) -> Result<Result<(), TorznabError>, nuvora_library::QueryError> {
        let mut indexer = Indexer {
            id: 0,
            name,
            url,
            api_key,
        };
        if let Err(e) = torznab::check(&self.http, &indexer).await {
            return Ok(Err(e));
        }
        let mut saved = self.saved.lock().await;
        indexer.id = saved.indexers.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        saved.indexers.push(indexer);
        self.save(&saved).await?;
        Ok(Ok(()))
    }

    /// False when there is no such indexer.
    pub async fn remove_indexer(&self, id: u32) -> Result<bool, nuvora_library::QueryError> {
        let mut saved = self.saved.lock().await;
        let before = saved.indexers.len();
        saved.indexers.retain(|i| i.id != id);
        if saved.indexers.len() == before {
            return Ok(false);
        }
        self.save(&saved).await?;
        Ok(true)
    }

    /// Where movies, and episodes and seasons, go; `None` for the first
    /// library of the kind.
    pub async fn set_libraries(
        &self,
        movies: Option<i64>,
        shows: Option<i64>,
    ) -> Result<(), nuvora_library::QueryError> {
        let mut saved = self.saved.lock().await;
        saved.library_id = movies;
        saved.shows_library_id = shows;
        self.save(&saved).await
    }

    /// Asks every indexer at once.
    pub async fn search(&self, query: &str) -> SearchResults {
        let indexers = self.saved.lock().await.indexers.clone();
        let mut tasks = tokio::task::JoinSet::new();
        for indexer in indexers {
            let http = self.http.clone();
            let query = query.to_owned();
            tasks.spawn(async move {
                let found = torznab::search(&http, &indexer, &query).await;
                (indexer.name, found)
            });
        }
        let mut results = SearchResults {
            releases: Vec::new(),
            failures: Vec::new(),
        };
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((_, Ok(releases))) => results.releases.extend(releases),
                Ok((name, Err(e))) => results.failures.push((name, e)),
                Err(e) => tracing::error!("indexer search panicked: {e}"),
            }
        }
        results
            .releases
            .sort_by(|a, b| b.seeders.cmp(&a.seeders).then(a.title.cmp(&b.title)));
        // The same release from two indexers: keep the better-seeded one.
        let mut seen = std::collections::HashSet::new();
        results.releases.retain(|r| seen.insert(r.title.clone()));
        results.failures.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    /// Starts downloading `link` into the downloads library. `title` names it
    /// until it lands; a magnet link's own name stands in when absent.
    pub async fn add(
        self: &Arc<Self>,
        link: &str,
        title: Option<String>,
    ) -> Result<DownloadId, AddError> {
        if let Err(e) = &self.engine {
            return Err(AddError::NoEngine(e.clone()));
        }
        if !valid_link(link) {
            return Err(AddError::BadLink);
        }
        let source = if link.starts_with("magnet:?") {
            Source::Magnet {
                uri: link.to_owned(),
            }
        } else {
            Source::Link {
                url: link.to_owned(),
            }
        };
        let title = title
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .or_else(|| magnet_name(link))
            .unwrap_or_else(|| "Untitled download".to_owned());
        let mut saved = self.saved.lock().await;
        let show = ReleaseName::parse(&title).content.is_show();
        let chosen = if show {
            saved.shows_library_id
        } else {
            saved.library_id
        };
        let library = self.target_library(show, chosen).await?;
        let root = library.roots.first().cloned().ok_or(if show {
            AddError::NoShowLibrary
        } else {
            AddError::NoMovieLibrary
        })?;
        // Said now rather than when the download fails: a read-only media
        // folder (Docker's `:ro`) can't take downloads at all.
        if let Err(e) = tokio::fs::create_dir_all(root.join(STAGING)).await {
            return Err(AddError::NotWritable {
                path: root,
                error: e,
            });
        }
        saved.last_id += 1;
        let id = saved.last_id;
        saved.downloads.push(Record {
            id,
            title,
            source,
            library_id: library.id.raw(),
            root,
            added_at: unix_now(),
            phase: Phase::Active,
        });
        self.save(&saved).await?;
        drop(saved);
        self.spawn(id);
        Ok(id)
    }

    /// Newest first.
    pub async fn list(&self) -> Vec<Snapshot> {
        let saved = self.saved.lock().await;
        let live = self.live.lock().expect("live lock");
        saved
            .downloads
            .iter()
            .rev()
            .map(|r| Snapshot {
                id: r.id,
                title: r.title.clone(),
                library_id: r.library_id,
                added_at: r.added_at,
                state: state(&r.phase, live.get(&r.id).map(|l| &l.stage)),
            })
            .collect()
    }

    pub async fn pause(&self, id: DownloadId) -> Result<(), ControlError> {
        self.set_phase(id, |phase| match phase {
            Phase::Active | Phase::Paused => Ok(Phase::Paused),
            Phase::Done { .. } => Err(ControlError::Finished),
            Phase::Failed { .. } => Ok(phase.clone()),
        })
        .await?;
        if let (Some(torrent), Ok(engine)) = (self.running(id), &self.engine)
            && let Err(e) = engine.pause(&torrent).await
        {
            tracing::debug!(id, "pause: {e:#}");
        }
        Ok(())
    }

    /// Resumes a paused download, or starts a failed one again.
    pub async fn resume(self: &Arc<Self>, id: DownloadId) -> Result<(), ControlError> {
        let before = self
            .set_phase(id, |phase| match phase {
                Phase::Done { .. } => Err(ControlError::Finished),
                _ => Ok(Phase::Active),
            })
            .await?;
        if matches!(before, Phase::Failed { .. }) {
            if self.engine.is_ok() {
                self.spawn(id);
            }
        } else if let (Some(torrent), Ok(engine)) = (self.running(id), &self.engine)
            && let Err(e) = engine.unpause(&torrent).await
        {
            tracing::debug!(id, "resume: {e:#}");
        }
        Ok(())
    }

    /// Stops a download and deletes what it fetched, or takes a finished one
    /// off the list (the movie stays in the library).
    pub async fn remove(&self, id: DownloadId) -> Result<(), ControlError> {
        let runner = {
            let mut live = self.live.lock().expect("live lock");
            if matches!(live.get(&id).map(|r| &r.stage), Some(Stage::Moving)) {
                return Err(ControlError::Moving);
            }
            live.remove(&id)
        };
        if let Some(runner) = runner {
            runner.task.abort();
            if let (Stage::Running(torrent), Ok(engine)) = (runner.stage, &self.engine)
                && let Err(e) = engine.delete(torrent.id().into(), true).await
            {
                tracing::debug!(id, "delete: {e:#}");
            }
        }
        let mut saved = self.saved.lock().await;
        let at = saved
            .downloads
            .iter()
            .position(|r| r.id == id)
            .ok_or(ControlError::NotFound)?;
        let record = saved.downloads.remove(at);
        self.save(&saved).await?;
        drop(saved);
        if !matches!(record.phase, Phase::Done { .. }) {
            remove_dir_if_present(&staging(&record)).await;
        }
        let _ = tokio::fs::remove_file(self.torrent_file(id)).await;
        Ok(())
    }

    fn spawn(self: &Arc<Self>, id: DownloadId) {
        let mut live = self.live.lock().expect("live lock");
        let downloads = self.clone();
        let task = tokio::spawn(async move { downloads.drive(id).await });
        live.insert(
            id,
            Runner {
                stage: Stage::Starting,
                task: task.abort_handle(),
            },
        );
    }

    /// One download from start to its place in the library.
    async fn drive(self: Arc<Self>, id: DownloadId) {
        let outcome = self.download(id).await;
        self.live.lock().expect("live lock").remove(&id);
        let phase = match outcome {
            Ok(path) => {
                tracing::info!(id, path = %path.display(), "download in the library");
                self.library.request_scan();
                Phase::Done {
                    path,
                    finished_at: unix_now(),
                }
            }
            Err(error) => {
                tracing::warn!(id, "download failed: {error}");
                Phase::Failed { error }
            }
        };
        if let Err(e) = self.set_phase(id, |_| Ok(phase.clone())).await
            && !matches!(e, ControlError::NotFound)
        {
            tracing::error!(id, "cannot save the download's outcome: {e}");
        }
    }

    async fn download(&self, id: DownloadId) -> Result<PathBuf, String> {
        let engine = self.engine.clone()?;
        let record = self.record(id).await.ok_or("No such download")?;
        let torrent = self.torrent(&record).await?;
        let staging = staging(&record);
        tokio::fs::create_dir_all(&staging)
            .await
            .map_err(|e| format!("Can't write to {}: {e}", staging.display()))?;
        let options = AddTorrentOptions {
            paused: record.phase == Phase::Paused,
            overwrite: true,
            output_folder: Some(staging.to_string_lossy().into_owned()),
            only_files_regex: Some(WANTED_FILES.to_owned()),
            ..Default::default()
        };
        let torrent = match engine.add_torrent(torrent, Some(options)).await {
            Ok(AddTorrentResponse::Added(_, torrent)) => torrent,
            Ok(AddTorrentResponse::AlreadyManaged(..)) => {
                return Err("This one is already downloading".to_owned());
            }
            Ok(AddTorrentResponse::ListOnly(_)) => unreachable!("list_only is off"),
            Err(e) => return Err(engine_message(&format!("{e:#}"))),
        };
        self.set_stage(id, Stage::Running(torrent.clone()));
        // Paused or resumed while starting: the torrent was added as it was then.
        let paused = self.record(id).await.map(|r| r.phase) == Some(Phase::Paused);
        let result = if paused != torrent.is_paused() {
            if paused {
                engine.pause(&torrent).await
            } else {
                engine.unpause(&torrent).await
            }
        } else {
            Ok(())
        };
        if let Err(e) = result {
            tracing::debug!(id, "pause/resume after start: {e:#}");
        }
        let finished = until_finished(&torrent).await;
        self.set_stage(id, Stage::Moving);
        if let Err(e) = engine.delete(torrent.id().into(), false).await {
            tracing::debug!(id, "stop after download: {e:#}");
        }
        finished?;
        let (root, title) = match self.show_instead(&record, &staging).await {
            Some(show) => show,
            None => (record.root.clone(), record.title.clone()),
        };
        tokio::task::spawn_blocking(move || move_into_library(&staging, &root, &title))
            .await
            .map_err(|e| format!("Moving it into the library stopped: {e}"))?
            .map_err(|e| format!("Couldn't move it into the library: {e}"))
    }

    /// What to hand the engine. An indexer link is fetched once and kept.
    async fn torrent(&self, record: &Record) -> Result<AddTorrent<'static>, String> {
        match &record.source {
            Source::Magnet { uri } => Ok(AddTorrent::from_url(uri.clone())),
            Source::TorrentFile => tokio::fs::read(self.torrent_file(record.id))
                .await
                .map(AddTorrent::from_bytes)
                .map_err(|e| format!("The saved .torrent file is gone: {e}")),
            Source::Link { url } => {
                let source = match fetch(&self.fetcher, url).await? {
                    Fetched::Magnet(uri) => Source::Magnet { uri },
                    Fetched::Torrent(bytes) => {
                        tokio::fs::write(self.torrent_file(record.id), &bytes)
                            .await
                            .map_err(|e| format!("Can't save the .torrent file: {e}"))?;
                        Source::TorrentFile
                    }
                };
                let mut saved = self.saved.lock().await;
                if let Some(r) = saved.downloads.iter_mut().find(|r| r.id == record.id) {
                    r.source = source.clone();
                }
                self.save(&saved)
                    .await
                    .map_err(|e| format!("Can't save the download: {e}"))?;
                drop(saved);
                Box::pin(self.torrent(&Record {
                    source,
                    ..record.clone()
                }))
                .await
            }
        }
    }

    /// TV Shows' folder and the show's name, for a download whose name read
    /// as a movie but whose videos are one show's episodes (a box set named
    /// without episode numbers). `None` keeps it where it was added.
    async fn show_instead(&self, record: &Record, staging: &Path) -> Option<(PathBuf, String)> {
        if ReleaseName::parse(&record.title).content.is_show() {
            return None;
        }
        let staging = staging.to_owned();
        let show = tokio::task::spawn_blocking(move || show_in(&staging))
            .await
            .ok()??;
        let chosen = self.saved.lock().await.shows_library_id;
        let library = self.target_library(true, chosen).await.ok()?;
        let root = library.roots.first()?.clone();
        let mut saved = self.saved.lock().await;
        if let Some(r) = saved.downloads.iter_mut().find(|r| r.id == record.id) {
            r.library_id = library.id.raw();
        }
        Some((root, show))
    }

    /// The saved choice if it is still a library of the kind, else the first
    /// one of the kind: TV Shows for a `show`, Movies otherwise.
    async fn target_library(
        &self,
        show: bool,
        chosen: Option<i64>,
    ) -> Result<LibraryInfo, AddError> {
        let libraries = self.library.libraries().await.map_err(AddError::Save)?;
        let kind = if show {
            LibraryKind::Shows
        } else {
            LibraryKind::Movies
        };
        let fits = |l: &&LibraryInfo| l.kind == kind && !l.roots.is_empty();
        chosen
            .and_then(|id| {
                libraries
                    .iter()
                    .filter(fits)
                    .find(|l| l.id == LibraryId::from_raw(id))
            })
            .or_else(|| libraries.iter().find(fits))
            .cloned()
            .ok_or(if show {
                AddError::NoShowLibrary
            } else {
                AddError::NoMovieLibrary
            })
    }

    async fn record(&self, id: DownloadId) -> Option<Record> {
        let saved = self.saved.lock().await;
        saved.downloads.iter().find(|r| r.id == id).cloned()
    }

    /// Sets the phase `next` picks and saves; returns the phase before.
    async fn set_phase(
        &self,
        id: DownloadId,
        next: impl FnOnce(&Phase) -> Result<Phase, ControlError>,
    ) -> Result<Phase, ControlError> {
        let mut saved = self.saved.lock().await;
        let record = saved
            .downloads
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(ControlError::NotFound)?;
        let before = std::mem::replace(&mut record.phase, Phase::Active);
        match next(&before) {
            Ok(phase) => record.phase = phase,
            Err(e) => {
                record.phase = before;
                return Err(e);
            }
        }
        self.save(&saved).await?;
        Ok(before)
    }

    fn set_stage(&self, id: DownloadId, stage: Stage) {
        if let Some(runner) = self.live.lock().expect("live lock").get_mut(&id) {
            runner.stage = stage;
        }
    }

    fn running(&self, id: DownloadId) -> Option<Arc<ManagedTorrent>> {
        match self.live.lock().expect("live lock").get(&id)?.stage {
            Stage::Running(ref torrent) => Some(torrent.clone()),
            _ => None,
        }
    }

    fn torrent_file(&self, id: DownloadId) -> PathBuf {
        self.dir.join(format!("{id}.torrent"))
    }

    async fn save(&self, saved: &Saved) -> Result<(), nuvora_library::QueryError> {
        let json = serde_json::to_string(saved).expect("downloads serialize");
        self.library.set_setting(SETTING_KEY, json).await
    }
}

async fn start_engine(dir: &Path, engine: Engine) -> Result<Arc<Session>, String> {
    match engine {
        Engine::Off => return Err("it is turned off".to_owned()),
        Engine::Internet => {}
        #[cfg(test)]
        Engine::Loopback => {
            return Session::new_with_opts(dir.to_owned(), loopback_options())
                .await
                .map_err(|e| format!("{e:#}"));
        }
    }
    let options = |listen_addr: SocketAddr| SessionOptions {
        dht: Some(DhtSessionConfig {
            persistence: Some(DhtPersistenceConfig {
                config_filename: Some(dir.join("dht.json")),
                ..Default::default()
            }),
            ..Default::default()
        }),
        listen: Some(ListenerOptions {
            listen_addr,
            enable_upnp_port_forwarding: true,
            ..Default::default()
        }),
        connect: Some(ConnectionOptions {
            peer_opts: Some(PeerConnectionOptions {
                connect_timeout: Some(CONNECT_TIMEOUT),
                ..Default::default()
            }),
            ..Default::default()
        }),
        peer_limit: Some(PEER_LIMIT),
        ..Default::default()
    };
    // `[::]` takes IPv6 and IPv4 peers on one socket; a machine without IPv6
    // gets IPv4 alone.
    let attempts: [SocketAddr; 4] = [
        (Ipv6Addr::UNSPECIFIED, LISTEN_PORT).into(),
        (Ipv4Addr::UNSPECIFIED, LISTEN_PORT).into(),
        (Ipv6Addr::UNSPECIFIED, 0).into(),
        (Ipv4Addr::UNSPECIFIED, 0).into(),
    ];
    let mut failed = String::new();
    for addr in attempts {
        match Session::new_with_opts(dir.to_owned(), options(addr)).await {
            Ok(session) => return Ok(session),
            Err(e) => {
                tracing::info!("BitTorrent can't listen on {addr} ({e:#}); trying the next");
                failed = format!("{e:#}");
            }
        }
    }
    Err(failed)
}

#[cfg(test)]
pub fn loopback_options() -> SessionOptions {
    SessionOptions {
        dht: None,
        disable_local_service_discovery: true,
        listen: Some(ListenerOptions {
            listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn state(phase: &Phase, stage: Option<&Stage>) -> State {
    match (phase, stage) {
        (Phase::Done { path, finished_at }, _) => State::Done {
            path: path.clone(),
            finished_at: *finished_at,
        },
        (Phase::Failed { error }, _) => State::Failed {
            error: error.clone(),
        },
        (_, Some(Stage::Moving)) => State::Moving,
        (phase, Some(Stage::Running(torrent))) => {
            let stats = torrent.stats();
            let progress = progress(&stats);
            match (phase, stats.state) {
                (_, TorrentStatsState::Initializing { .. }) => State::Checking(progress),
                (Phase::Paused, _) => State::Paused(Some(progress)),
                _ => State::Downloading(progress),
            }
        }
        (Phase::Paused, _) => State::Paused(None),
        (Phase::Active, _) => State::Starting,
    }
}

fn progress(stats: &TorrentStats) -> Progress {
    let live = stats.live.as_ref();
    let bps = |mib_per_s: f64| (mib_per_s * 1024.0 * 1024.0) as u64;
    Progress {
        done_bytes: stats.progress_bytes,
        total_bytes: stats.total_bytes,
        down_bps: live.map_or(0, |l| bps(l.download_speed.mbps)),
        up_bps: live.map_or(0, |l| bps(l.upload_speed.mbps)),
        peers: live.map_or(0, |l| l.snapshot.peer_stats.live),
    }
}

/// Polled rather than awaited: the engine's own wait does not survive a
/// pause and resume.
async fn until_finished(torrent: &ManagedTorrent) -> Result<(), String> {
    loop {
        let stats = torrent.stats();
        if let Some(error) = stats.error {
            return Err(engine_message(&error));
        }
        if stats.finished {
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The engine's errors, in words for the Downloads page where we know them.
fn engine_message(error: &str) -> String {
    if error.contains("none of the filenames match") {
        "It has no video files".to_owned()
    } else {
        let first = error.lines().next().unwrap_or(error);
        format!("The download stopped: {first}")
    }
}

enum Fetched {
    Magnet(String),
    Torrent(Vec<u8>),
}

/// A magnet link, or an `http(s)` link to a `.torrent`.
pub fn valid_link(link: &str) -> bool {
    link.starts_with("magnet:?")
        || (torznab::is_downloadable(link) && reqwest::Url::parse(link).is_ok())
}

/// What to hand the engine for a [`valid_link`]: a magnet link as it is; an
/// indexer's link fetched.
pub async fn resolve(fetcher: &reqwest::Client, link: &str) -> Result<AddTorrent<'static>, String> {
    if link.starts_with("magnet:?") {
        return Ok(AddTorrent::from_url(link.to_owned()));
    }
    Ok(match fetch(fetcher, link).await? {
        Fetched::Magnet(uri) => AddTorrent::from_url(uri),
        Fetched::Torrent(bytes) => AddTorrent::from_bytes(bytes),
    })
}

pub fn fetcher() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Nuvora/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "magnet" {
                attempt.stop()
            } else if attempt.previous().len() >= 10 {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .expect("TLS backend initializes")
}

/// An indexer's download link: a `.torrent`, or (Jackett and Prowlarr do this
/// for magnet-only trackers) a redirect to a magnet link.
async fn fetch(http: &reqwest::Client, url: &str) -> Result<Fetched, String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Couldn't get the torrent: {}", e.without_url()))?;
    let status = response.status();
    if status.is_redirection() {
        return response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|l| l.to_str().ok())
            .filter(|l| l.starts_with("magnet:?"))
            .map(|l| Fetched::Magnet(l.to_owned()))
            .ok_or_else(|| "The indexer's link goes nowhere".to_owned());
    }
    if !status.is_success() {
        return Err(format!("The indexer answered {status} for the torrent"));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_TORRENT_BYTES as u64)
    {
        return Err("That isn't a torrent file: it's too big".to_owned());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Couldn't get the torrent: {}", e.without_url()))?;
    // A torrent file is a bencoded dictionary.
    if bytes.len() > MAX_TORRENT_BYTES || bytes.first() != Some(&b'd') {
        return Err("The indexer didn't send a torrent file".to_owned());
    }
    Ok(Fetched::Torrent(bytes.to_vec()))
}

fn staging(record: &Record) -> PathBuf {
    record.root.join(STAGING).join(record.id.to_string())
}

/// Moves the videos and subtitles from `staging` into the movie's or show's
/// folder, `root/<Title (Year)>/`, keeping the torrent's sub-folders below
/// its top folder (a show's `Season 01/`, say), then deletes `staging` and
/// whatever else it held. Episodes join the show's folder if it is there.
/// Returns the folder.
fn move_into_library(staging: &Path, root: &Path, title: &str) -> std::io::Result<PathBuf> {
    let name = ReleaseName::parse(title);
    let folder = match name.year {
        Some(year) => format!("{} ({year})", name.title),
        None => name.title,
    };
    let folder = safe_name(&folder).unwrap_or_else(|| "Downloaded".to_owned());
    let destination = root.join(folder);
    // A multi-file torrent has one top folder; its name is the release's.
    let mut top = staging.to_path_buf();
    let entries: Vec<_> = std::fs::read_dir(staging)?.collect::<Result<_, _>>()?;
    if let [only] = entries.as_slice()
        && only.file_type()?.is_dir()
    {
        top = only.path();
    }
    let mut moved = 0;
    for file in files_under(&top)? {
        let rel = file.strip_prefix(&top).expect("under top");
        if !matches!(classify_path(rel), PathClass::Video | PathClass::Subtitle) {
            continue;
        }
        let target = free_path(&destination.join(rel));
        std::fs::create_dir_all(target.parent().expect("has a parent"))?;
        move_file(&file, &target)?;
        moved += 1;
    }
    if moved == 0 {
        return Err(std::io::Error::other("it holds no video"));
    }
    std::fs::remove_dir_all(staging)?;
    // The staging parent goes too once nothing else is downloading there.
    if let Some(parent) = staging.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(destination)
}

/// The show a finished torrent holds: every main video names an episode of
/// it, and there are at least two. Read from file names alone, since the
/// torrent's top folder is the release's name.
fn show_in(staging: &Path) -> Option<String> {
    let mut first: Option<EpisodeName> = None;
    let mut episodes = HashSet::new();
    for file in files_under(staging).ok()? {
        if classify_path(file.strip_prefix(staging).ok()?) != PathClass::Video {
            continue;
        }
        let episode = EpisodeName::parse(Path::new(file.file_name()?))?;
        if first
            .as_ref()
            .is_some_and(|f| f.show_key() != episode.show_key())
        {
            return None;
        }
        episodes.insert(episode.episode_key());
        first.get_or_insert(episode);
    }
    let show = first?.show;
    (episodes.len() >= 2).then(|| match show.year {
        Some(year) => format!("{} ({year})", show.title),
        None => show.title,
    })
}

fn files_under(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// `path`, or `name (2).ext`, `name (3).ext`, ... when it is taken.
fn free_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("some number is free")
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)
        }
        result => result,
    }
}

/// A folder name every filesystem accepts.
fn safe_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches(|c| c == '.' || c == ' ');
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

/// A magnet link's `dn` (display name).
pub fn magnet_name(link: &str) -> Option<String> {
    reqwest::Url::parse(link)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == "dn")
        .map(|(_, v)| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

async fn remove_dir_if_present(dir: &Path) {
    if let Err(e) = tokio::fs::remove_dir_all(dir).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("cannot delete {}: {e}", dir.display());
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn tree(root: &Path) -> Vec<String> {
        let mut files: Vec<String> = files_under(root)
            .unwrap()
            .iter()
            .map(|f| {
                f.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn a_finished_torrent_lands_in_a_title_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Movies");
        let staging = root.join(STAGING).join("7");
        let top = staging.join("Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP");
        touch(
            &top.join("Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP.mkv"),
            b"movie",
        );
        touch(&top.join("Subs/English.srt"), b"subs");
        touch(&top.join("Sample/sample.mkv"), b"sample");
        touch(&top.join("RARBG.txt"), b"junk");
        touch(
            &root.join("Other Movie (2001)/Other Movie (2001).mkv"),
            b"other",
        );

        let landed = move_into_library(
            &staging,
            &root,
            "Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP",
        )
        .unwrap();

        assert_eq!(landed, root.join("Night of the Living Dead (1968)"));
        assert_eq!(
            tree(&root),
            [
                "Night of the Living Dead (1968)/Night.of.the.Living.Dead.1968.1080p.BluRay.x264-GRP.mkv",
                "Night of the Living Dead (1968)/Subs/English.srt",
                "Other Movie (2001)/Other Movie (2001).mkv",
            ]
        );
        assert!(!root.join(STAGING).exists());
    }

    #[test]
    fn a_single_file_torrent_beside_an_existing_copy_gets_a_free_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        touch(&root.join("Heat (1995)/Heat.1995.mkv"), b"old");
        touch(&root.join(STAGING).join("1/Heat.1995.mkv"), b"new");
        touch(
            &root.join(STAGING).join("2/Other.mkv"),
            b"still downloading",
        );

        let landed = move_into_library(&root.join(STAGING).join("1"), &root, "Heat 1995").unwrap();

        assert_eq!(landed, root.join("Heat (1995)"));
        assert_eq!(
            tree(&root),
            [
                ".nuvora-downloads/2/Other.mkv",
                "Heat (1995)/Heat.1995 (2).mkv",
                "Heat (1995)/Heat.1995.mkv",
            ]
        );
    }

    #[test]
    fn a_torrent_without_video_fails_and_keeps_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join(STAGING).join("3");
        touch(&staging.join("readme.txt"), b"nope");
        let error = move_into_library(&staging, dir.path(), "Nothing").unwrap_err();
        assert_eq!(error.to_string(), "it holds no video");
        assert!(staging.join("readme.txt").exists());
    }

    #[test]
    fn magnet_names_and_folder_names() {
        assert_eq!(
            magnet_name("magnet:?xt=urn:btih:abc&dn=Big+Buck+Bunny%20(2008)").as_deref(),
            Some("Big Buck Bunny (2008)")
        );
        assert_eq!(magnet_name("magnet:?xt=urn:btih:abc"), None);
        assert_eq!(
            safe_name("AC/DC: Live? <1991>").as_deref(),
            Some("AC DC Live 1991")
        );
        assert_eq!(safe_name(" ... "), None);
    }
}
