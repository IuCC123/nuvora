//! Torrent streams: play a release while it downloads, with nothing kept.
//!
//! A stream adds the torrent to the downloads engine with only its main
//! video selected, written to `<data>/downloads/streams/<id>/`. Reads go
//! through the engine's file streams, which fetch the pieces under the read
//! position first, so the player (HTTP Range) and FFmpeg (for converting)
//! both get bytes as soon as peers send them. FFmpeg reads from a feed on
//! the loopback interface, `http://127.0.0.1:<port>/<id>`: the id is 128
//! random bits and never leaves the server's own processes.
//!
//! Nothing is saved: a stream nobody has asked about for [`IDLE`] stops,
//! and its files are deleted, as are all of them when the server starts.

use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use librqbit::{AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session};
use nuvora_library::StreamSource;
use nuvora_media::{EpisodeName, MediaInfo, MediaTools, PathClass, classify_path};
use password_hash::rand_core::{OsRng, RngCore};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// A stream nobody has read from or asked about for this long stops.
const IDLE: Duration = Duration::from_secs(120);
/// Streams at once; starting one more stops the one least recently used.
const MAX_STREAMS: usize = 3;
/// ffprobe waits for the pieces it reads; each try has the probe timeout.
const PROBE_TRIES: u32 = 6;

pub struct Streams {
    engine: Result<Arc<Session>, String>,
    fetcher: reqwest::Client,
    tools: MediaTools,
    dir: PathBuf,
    feed: SocketAddr,
    live: Mutex<HashMap<String, Arc<Stream>>>,
    keys: AtomicI64,
}

pub struct Stream {
    pub id: String,
    /// The converter's name for this stream's video: negative, so it never
    /// meets a library file's id.
    pub key: i64,
    /// The release's name: as given, else the torrent's own once it is in.
    title: Mutex<Option<String>>,
    link: String,
    seen: Mutex<Instant>,
    phase: Mutex<Phase>,
    task: Mutex<Option<tokio::task::AbortHandle>>,
    /// Once added; `true` when this stream added it, so stopping removes it.
    torrent: Mutex<Option<(Arc<ManagedTorrent>, bool)>>,
}

#[derive(Clone)]
pub enum Phase {
    /// Fetching the `.torrent`, or a magnet link's details from peers.
    Starting,
    /// Reading the start of the video to learn what it is.
    Probing(StreamFile),
    Ready(StreamFile, Arc<MediaInfo>),
    Failed(String),
}

#[derive(Clone)]
pub struct StreamFile {
    pub torrent: Arc<ManagedTorrent>,
    /// The file's index in the torrent.
    pub index: usize,
    pub name: String,
    pub len: u64,
}

pub struct Progress {
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub down_bps: u64,
    pub peers: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("Streaming isn't available: {0}")]
    NoEngine(String),
    #[error("Use a magnet link or a link to a .torrent file")]
    BadLink,
}

impl Streams {
    /// Deletes what earlier runs left and starts the feed FFmpeg reads.
    pub async fn start(
        engine: Result<Arc<Session>, String>,
        tools: MediaTools,
        data_dir: &Path,
    ) -> std::io::Result<Arc<Streams>> {
        let dir = data_dir.join("downloads").join("streams");
        if let Err(e) = tokio::fs::remove_dir_all(&dir).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("cannot clear {}: {e}", dir.display());
        }
        tokio::fs::create_dir_all(&dir).await?;
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let streams = Arc::new(Streams {
            engine,
            fetcher: crate::downloads::fetcher(),
            tools,
            dir,
            feed: listener.local_addr()?,
            live: Mutex::default(),
            keys: AtomicI64::new(1),
        });
        let feed = axum::Router::new()
            .route("/{id}", axum::routing::get(feed))
            .with_state(streams.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, feed).await {
                tracing::error!("stream feed stopped: {e}");
            }
        });
        let sweeper = streams.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                sweeper.sweep().await;
            }
        });
        Ok(streams)
    }

    /// Starts streaming `link`, or returns the stream already playing it.
    pub fn open(
        self: &Arc<Self>,
        link: &str,
        title: Option<String>,
    ) -> Result<Arc<Stream>, OpenError> {
        if let Err(e) = &self.engine {
            return Err(OpenError::NoEngine(e.clone()));
        }
        if !crate::downloads::valid_link(link) {
            return Err(OpenError::BadLink);
        }
        let mut live = self.live.lock().expect("streams lock");
        if let Some(same) = live
            .values()
            .find(|s| s.link == link && !matches!(s.phase(), Phase::Failed(_)))
        {
            same.touch();
            return Ok(same.clone());
        }
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let stream = Arc::new(Stream {
            id: id.clone(),
            key: -self.keys.fetch_add(1, Ordering::Relaxed),
            title: Mutex::new(title),
            link: link.to_owned(),
            seen: Mutex::new(Instant::now()),
            phase: Mutex::new(Phase::Starting),
            task: Mutex::default(),
            torrent: Mutex::default(),
        });
        live.insert(id, stream.clone());
        let mut by_use: Vec<(Instant, String)> = live
            .values()
            .map(|s| (*s.seen.lock().expect("seen lock"), s.id.clone()))
            .collect();
        by_use.sort();
        let excess = live.len().saturating_sub(MAX_STREAMS);
        let evicted: Vec<Arc<Stream>> = by_use
            .into_iter()
            .take(excess)
            .filter_map(|(_, id)| live.remove(&id))
            .collect();
        drop(live);
        for old in evicted {
            let streams = self.clone();
            tokio::spawn(async move { streams.end(&old).await });
        }
        let streams = self.clone();
        let running = stream.clone();
        let task = tokio::spawn(async move { streams.run(running).await });
        *stream.task.lock().expect("task lock") = Some(task.abort_handle());
        Ok(stream)
    }

    /// The stream with `id`, which counts as using it.
    pub fn get(&self, id: &str) -> Option<Arc<Stream>> {
        let stream = self.live.lock().expect("streams lock").get(id).cloned()?;
        stream.touch();
        Some(stream)
    }

    /// The stream the converter knows as `key`.
    pub fn by_key(&self, key: i64) -> Option<Arc<Stream>> {
        let live = self.live.lock().expect("streams lock");
        live.values().find(|s| s.key == key).cloned()
    }

    /// Stops a stream and deletes what it fetched. False when there is none.
    pub async fn stop(&self, id: &str) -> bool {
        let Some(stream) = self.live.lock().expect("streams lock").remove(id) else {
            return false;
        };
        self.end(&stream).await;
        true
    }

    /// What the converter reads for a ready stream: the feed, and its media.
    pub fn source(&self, stream: &Stream) -> Option<StreamSource> {
        let Phase::Ready(_, media) = stream.phase() else {
            return None;
        };
        Some(StreamSource {
            path: PathBuf::from(self.feed_url(&stream.id)),
            container: media.container,
            fingerprint: stream.id.clone(),
            media: (*media).clone(),
        })
    }

    async fn end(&self, stream: &Stream) {
        if let Some(task) = stream.task.lock().expect("task lock").take() {
            task.abort();
        }
        let torrent = stream.torrent.lock().expect("torrent lock").take();
        if let (Some((torrent, true)), Ok(engine)) = (torrent, &self.engine)
            && let Err(e) = engine.delete(torrent.id().into(), true).await
        {
            tracing::debug!(id = stream.id, "stop stream: {e:#}");
        }
        let _ = tokio::fs::remove_dir_all(self.dir.join(&stream.id)).await;
    }

    async fn sweep(&self) {
        let idle: Vec<Arc<Stream>> = {
            let mut live = self.live.lock().expect("streams lock");
            let ids: Vec<String> = live
                .values()
                .filter(|s| s.seen.lock().expect("seen lock").elapsed() > IDLE)
                .map(|s| s.id.clone())
                .collect();
            ids.iter().filter_map(|id| live.remove(id)).collect()
        };
        for stream in idle {
            tracing::info!(title = stream.title(), "stream idle, stopped");
            self.end(&stream).await;
        }
    }

    fn feed_url(&self, id: &str) -> String {
        format!("http://{}/{id}", self.feed)
    }

    async fn run(self: Arc<Self>, stream: Arc<Stream>) {
        let phase = match self.prepare(&stream).await {
            Ok((file, media)) => Phase::Ready(file, Arc::new(media)),
            Err(error) => {
                tracing::warn!(title = stream.title(), "stream failed: {error}");
                Phase::Failed(error)
            }
        };
        *stream.phase.lock().expect("phase lock") = phase;
    }

    async fn prepare(&self, stream: &Stream) -> Result<(StreamFile, MediaInfo), String> {
        let engine = self.engine.clone()?;
        let torrent = crate::downloads::resolve(&self.fetcher, &stream.link).await?;
        let options = AddTorrentOptions {
            paused: true,
            overwrite: true,
            output_folder: Some(self.dir.join(&stream.id).to_string_lossy().into_owned()),
            ..Default::default()
        };
        let (torrent, owned) = match engine.add_torrent(torrent, Some(options)).await {
            Ok(AddTorrentResponse::Added(_, torrent)) => (torrent, true),
            // Already downloading: stream from that, and leave it be.
            Ok(AddTorrentResponse::AlreadyManaged(_, torrent)) => (torrent, false),
            Ok(AddTorrentResponse::ListOnly(_)) => unreachable!("list_only is off"),
            Err(e) => return Err(format!("Couldn't start it: {e:#}")),
        };
        *stream.torrent.lock().expect("torrent lock") = Some((torrent.clone(), owned));
        stream
            .title
            .lock()
            .expect("title lock")
            .get_or_insert_with(|| torrent.name().unwrap_or_default());
        let (index, name, len) = torrent
            .with_metadata(|m| main_video(&m.file_infos))
            .map_err(|e| format!("Couldn't read the torrent: {e:#}"))?
            .ok_or("It has no video files")?;
        if owned {
            // The engine checks for files already on disk first; until then
            // it takes no new selection.
            torrent
                .wait_until_initialized()
                .await
                .map_err(|e| format!("Couldn't start it: {e:#}"))?;
            engine
                .update_only_files(&torrent, &HashSet::from([index]))
                .await
                .map_err(|e| format!("Couldn't pick the video: {e:#}"))?;
            engine
                .unpause(&torrent)
                .await
                .map_err(|e| format!("Couldn't start it: {e:#}"))?;
        }
        let file = StreamFile {
            torrent,
            index,
            name,
            len,
        };
        *stream.phase.lock().expect("phase lock") = Phase::Probing(file.clone());
        let url = self.feed_url(&stream.id);
        let mut last = String::new();
        for _ in 0..PROBE_TRIES {
            match nuvora_media::probe(&self.tools, Path::new(&url)).await {
                Ok(media) if media.video.is_empty() => return Err("It has no picture".to_owned()),
                Ok(media) => return Ok((file, media)),
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("Couldn't read the video: {last}"))
    }
}

impl Stream {
    /// The release's name, or `""` until the torrent's details are in.
    pub fn title(&self) -> String {
        self.title
            .lock()
            .expect("title lock")
            .clone()
            .unwrap_or_default()
    }

    pub fn phase(&self) -> Phase {
        self.phase.lock().expect("phase lock").clone()
    }

    /// The video, once the torrent's details are in.
    pub fn file(&self) -> Option<StreamFile> {
        match self.phase() {
            Phase::Probing(file) | Phase::Ready(file, _) => Some(file),
            Phase::Starting | Phase::Failed(_) => None,
        }
    }

    /// How much of the video is here, and how fast more comes.
    pub fn progress(&self) -> Option<Progress> {
        let file = self.file()?;
        let stats = file.torrent.stats();
        let live = stats.live.as_ref();
        Some(Progress {
            done_bytes: stats.progress_bytes,
            total_bytes: stats.total_bytes.max(file.len),
            down_bps: live.map_or(0, |l| (l.download_speed.mbps * 1024.0 * 1024.0) as u64),
            peers: live.map_or(0, |l| l.snapshot.peer_stats.live),
        })
    }

    fn touch(&self) {
        *self.seen.lock().expect("seen lock") = Instant::now();
    }
}

/// What to play: the first episode of a pack of episodes, else the biggest
/// video that isn't a sample or an extra. Index, name, size.
fn main_video(files: &librqbit::FileInfos) -> Option<(usize, String, u64)> {
    let videos: Vec<(usize, &librqbit::file_info::FileInfo)> = files
        .iter()
        .enumerate()
        .filter(|(_, f)| classify_path(&f.relative_filename) == PathClass::Video)
        .collect();
    let episodes: Vec<(u32, u32, usize)> = videos
        .iter()
        .filter_map(|(i, f)| {
            EpisodeName::parse(&f.relative_filename).map(|e| (e.season, e.episode, *i))
        })
        .collect();
    let chosen = if episodes.len() > 1 {
        episodes.into_iter().min().map(|(_, _, i)| i)
    } else {
        videos.iter().max_by_key(|(_, f)| f.len).map(|(i, _)| *i)
    };
    chosen
        .and_then(|i| files.get(i).map(|f| (i, f)))
        .map(|(i, f)| {
            let name = f
                .relative_filename
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            (i, name, f.len)
        })
}

async fn feed(
    State(streams): State<Arc<Streams>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    match streams.get(&id).and_then(|s| s.file()) {
        Some(file) => serve_range(&file, &headers, "application/octet-stream").await,
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The file, or the one byte range asked for, as the engine fetches it.
pub async fn serve_range(file: &StreamFile, headers: &HeaderMap, mime: &str) -> Response {
    let len = file.len;
    let asked = headers
        .get(header::RANGE)
        .and_then(|r| r.to_str().ok())
        .and_then(|r| byte_range(r, len));
    let (start, end, status) = match asked {
        None => (0, len.saturating_sub(1), StatusCode::OK),
        Some(Some((start, end))) => (start, end, StatusCode::PARTIAL_CONTENT),
        Some(None) => {
            let unsatisfiable = [(header::CONTENT_RANGE, format!("bytes */{len}"))];
            return (StatusCode::RANGE_NOT_SATISFIABLE, unsatisfiable).into_response();
        }
    };
    let mut reader = match file.torrent.clone().stream(file.index).await {
        Ok(reader) => reader,
        Err(e) => {
            tracing::warn!("cannot read stream: {e:#}");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    if let Err(e) = reader.seek(SeekFrom::Start(start)).await {
        tracing::warn!("cannot seek stream: {e}");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let body = Body::from_stream(tokio_util::io::ReaderStream::with_capacity(
        reader.take(end + 1 - start),
        64 * 1024,
    ));
    let mut response = (status, body).into_response();
    let out = response.headers_mut();
    out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    out.insert(header::CONTENT_LENGTH, (end + 1 - start).into());
    if status == StatusCode::PARTIAL_CONTENT {
        let range = format!("bytes {start}-{end}/{len}");
        out.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&range).expect("ascii"),
        );
    }
    response
}

/// One `bytes=` range of a `len`-byte file, as inclusive offsets. `None`:
/// not a single byte range, so the whole file. `Some(None)`: out of range.
fn byte_range(header: &str, len: u64) -> Option<Option<(u64, u64)>> {
    let spec = header.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (from, to) = spec.split_once('-')?;
    let range = match (from.trim(), to.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (n > 0 && len > 0).then(|| (len.saturating_sub(n), len - 1))
        }
        (from, "") => {
            let start: u64 = from.parse().ok()?;
            (start < len).then(|| (start, len - 1))
        }
        (from, to) => {
            let (start, end): (u64, u64) = (from.parse().ok()?, to.parse().ok()?);
            (start <= end && start < len).then(|| (start, end.min(len - 1)))
        }
    };
    Some(range)
}

#[cfg(test)]
mod tests {
    use super::{byte_range, main_video};

    #[test]
    fn a_pack_plays_its_first_episode_and_a_movie_its_biggest_video() {
        let file = |name: &str, len: u64| librqbit::file_info::FileInfo {
            relative_filename: name.into(),
            offset_in_torrent: 0,
            piece_range: 0..1,
            attrs: Default::default(),
            len,
        };
        let pack = vec![
            file("Show.S01/Show.S01E02.mkv", 900),
            file("Show.S01/Show.S01E01.mkv", 800),
            file("Show.S01/Show.S01E10.mkv", 950),
        ];
        assert_eq!(main_video(&pack).map(|(i, ..)| i), Some(1));
        let movie = vec![
            file("Movie.2020/Sample/sample.mkv", 50),
            file("Movie.2020/Movie.2020.mkv", 4000),
            file("Movie.2020/Movie.2020.nfo", 1),
        ];
        assert_eq!(main_video(&movie).map(|(i, ..)| i), Some(1));
    }

    #[test]
    fn byte_ranges() {
        assert_eq!(byte_range("bytes=0-99", 1000), Some(Some((0, 99))));
        assert_eq!(byte_range("bytes=900-", 1000), Some(Some((900, 999))));
        assert_eq!(byte_range("bytes=-100", 1000), Some(Some((900, 999))));
        assert_eq!(byte_range("bytes=990-2000", 1000), Some(Some((990, 999))));
        assert_eq!(byte_range("bytes=1000-", 1000), Some(None));
        assert_eq!(byte_range("bytes=0-1,5-9", 1000), None);
        assert_eq!(byte_range("items=0-1", 1000), None);
    }
}
