//! Recap, intro and credits markers for episodes whose files have no chapters
//! naming them, found by sound (see `nuvora_media::find_markers`): after
//! each scan, show by show, newest first, one file at a time so playback
//! keeps most of the machine. A show someone opens in the player goes first,
//! the season being watched before the others.
//!
//! Each episode file gets a whole-file audio print, stored by content
//! fingerprint like seek previews: `<data>/audio-prints/<fingerprint>-v<n>.bin`,
//! about 85 KB for 45 minutes. Markers are found again from those when a
//! show's episodes change, which `audio_marker_show` notices.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nuvora_media::{AUDIO_PRINT_VERSION, AudioPrint, Marker, MediaTools, find_markers};
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::{mpsc, watch};

use crate::db::Db;
use crate::{FileId, ScanState, ScanStatus};

/// Bumped when detection changes, so every show is looked at again.
const MARKER_VERSION: u32 = 2;
/// Other episodes of the season an episode is compared with, nearest first.
const SEASON_NEIGHBOURS: usize = 4;
/// Episodes before it whose clips a recap is looked for in, latest first.
const EARLIER: usize = 8;
const UNFINISHED: &str = ".unfinished-";

struct PrintStore {
    root: PathBuf,
}

impl PrintStore {
    fn name(fingerprint: &[u8]) -> String {
        let hex: String = fingerprint.iter().map(|b| format!("{b:02x}")).collect();
        format!("{hex}-v{AUDIO_PRINT_VERSION}.bin")
    }

    fn get(&self, fingerprint: &[u8]) -> Option<AudioPrint> {
        let bytes = std::fs::read(self.root.join(Self::name(fingerprint))).ok()?;
        Some(AudioPrint::from_bytes(0, &bytes))
    }

    async fn make(&self, tools: &MediaTools, file: &EpisodeFile) -> Option<AudioPrint> {
        let print = tools
            .audio_print(
                &file.path,
                Duration::ZERO,
                Duration::from_millis(file.duration_ms),
            )
            .await;
        let print = match print {
            Ok(p) if !p.items.is_empty() => p,
            Ok(_) => return None,
            Err(e) => {
                tracing::warn!(path = %file.path.display(), "no audio print: {e}");
                return None;
            }
        };
        let name = Self::name(&file.fingerprint);
        let scratch = self.root.join(format!("{UNFINISHED}{name}"));
        let saved = async {
            tokio::fs::write(&scratch, print.to_bytes()).await?;
            tokio::fs::rename(&scratch, self.root.join(&name)).await
        };
        if let Err(e) = saved.await {
            tracing::warn!("cannot save an audio print: {e}");
        }
        Some(print)
    }

    /// Deletes prints of files no longer in the catalog, of older versions,
    /// and unfinished ones.
    fn retain(&self, live: &HashSet<String>) -> std::io::Result<usize> {
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let keep = entry.file_name().to_str().is_some_and(|n| live.contains(n));
            if !keep && entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[derive(Debug, Clone)]
struct EpisodeFile {
    file: i64,
    season: i64,
    episode: i64,
    path: PathBuf,
    fingerprint: Vec<u8>,
    duration_ms: u64,
}

#[derive(Debug)]
struct Show {
    id: i64,
    /// In watching order: by season, then episode.
    files: Vec<EpisodeFile>,
    signature: Vec<u8>,
}

/// Episode files that have audio, grouped by show, newest show first.
fn shows(conn: &Connection) -> rusqlite::Result<Vec<Show>> {
    let mut stmt = conn.prepare_cached(
        "SELECT show.id, COALESCE(season.index_number, 0), COALESCE(ep.index_number, 0), f.id, r.path, \
         f.rel_path, f.fingerprint, f.duration_ms, f.added_at \
         FROM item ep JOIN item season ON season.id = ep.parent_id JOIN item show ON show.id = season.parent_id \
         JOIN media_file f ON f.item_id = ep.id JOIN library_root r ON r.id = f.root_id \
         WHERE ep.kind = 'episode' AND ep.missing_since IS NULL AND f.probe_error IS NULL AND f.duration_ms > 0 \
         AND EXISTS (SELECT 1 FROM media_stream s WHERE s.file_id = f.id AND s.kind = 'audio') \
         ORDER BY show.id, 2, 3, f.id",
    )?;
    let mut shows: Vec<(Show, i64)> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let show: i64 = r.get(0)?;
        let root: String = r.get(4)?;
        let rel: String = r.get(5)?;
        let added: i64 = r.get(8)?;
        let file = EpisodeFile {
            season: r.get(1)?,
            episode: r.get(2)?,
            file: r.get(3)?,
            path: PathBuf::from(root).join(rel),
            fingerprint: r.get(6)?,
            duration_ms: r.get::<_, i64>(7)? as u64,
        };
        match shows.last_mut() {
            Some((s, newest)) if s.id == show => {
                s.files.push(file);
                *newest = (*newest).max(added);
            }
            _ => shows.push((
                Show {
                    id: show,
                    files: vec![file],
                    signature: Vec::new(),
                },
                added,
            )),
        }
    }
    shows.sort_by_key(|(_, newest)| std::cmp::Reverse(*newest));
    Ok(shows
        .into_iter()
        .map(|(mut s, _)| {
            let mut hash = blake3::Hasher::new();
            hash.update(&MARKER_VERSION.to_le_bytes());
            hash.update(&AUDIO_PRINT_VERSION.to_le_bytes());
            for f in &s.files {
                hash.update(&f.season.to_le_bytes());
                hash.update(&f.episode.to_le_bytes());
                hash.update(&f.file.to_le_bytes());
                hash.update(&f.fingerprint);
            }
            s.signature = hash.finalize().as_bytes().to_vec();
            s
        })
        .collect())
}

/// Shows whose markers were found from other episodes than they have now.
fn stale(conn: &Connection) -> rusqlite::Result<Vec<Show>> {
    let done: HashMap<i64, Vec<u8>> = conn
        .prepare_cached("SELECT show_id, signature FROM audio_marker_show")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(shows(conn)?
        .into_iter()
        .filter(|s| done.get(&s.id) != Some(&s.signature))
        .collect())
}

fn show_of(conn: &Connection, file: FileId) -> rusqlite::Result<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT season.parent_id, COALESCE(season.index_number, 0) FROM media_file f \
         JOIN item ep ON ep.id = f.item_id JOIN item season ON season.id = ep.parent_id \
         WHERE f.id = ?1 AND ep.kind = 'episode'",
        [file.0],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Markers of every file of `show`, from the prints there are; none for a file without one.
fn detect(show: &Show, prints: &HashMap<i64, AudioPrint>) -> Vec<(i64, Vec<Marker>)> {
    // One file stands for each episode when others are compared with it.
    let mut episodes: Vec<&EpisodeFile> = Vec::new();
    for f in &show.files {
        let seen = episodes
            .last()
            .is_some_and(|e| (e.season, e.episode) == (f.season, f.episode));
        if !seen && prints.contains_key(&f.file) {
            episodes.push(f);
        }
    }
    show.files
        .iter()
        .map(|f| {
            let Some(print) = prints.get(&f.file) else {
                return (f.file, Vec::new());
            };
            let here = episodes
                .iter()
                .position(|e| (e.season, e.episode) >= (f.season, f.episode))
                .unwrap_or(episodes.len());
            let mut season: Vec<&EpisodeFile> = episodes
                .iter()
                .copied()
                .filter(|e| e.season == f.season && e.episode != f.episode)
                .collect();
            season.sort_by_key(|e| e.episode.abs_diff(f.episode));
            let season: Vec<&AudioPrint> = season
                .iter()
                .take(SEASON_NEIGHBOURS)
                .map(|e| &prints[&e.file])
                .collect();
            let earlier: Vec<&AudioPrint> = episodes[..here]
                .iter()
                .rev()
                .take(EARLIER)
                .map(|e| &prints[&e.file])
                .collect();
            (
                f.file,
                find_markers(print, f.duration_ms, &season, &earlier),
            )
        })
        .collect()
}

/// Replaces the audio markers of `show`'s files; with `signature`, records
/// that the show is done.
fn write(
    tx: &rusqlite::Transaction<'_>,
    show: i64,
    found: &[(i64, Vec<Marker>)],
    signature: Option<&[u8]>,
) -> rusqlite::Result<()> {
    let mut clear =
        tx.prepare_cached("DELETE FROM media_marker WHERE file_id = ?1 AND source = 'audio'")?;
    let mut insert = tx.prepare_cached(
        "INSERT OR IGNORE INTO media_marker (file_id, kind, start_ms, end_ms, source) VALUES (?1, ?2, ?3, ?4, 'audio')",
    )?;
    for (file, markers) in found {
        clear.execute([file])?;
        for m in markers {
            insert.execute(params![
                file,
                m.kind.as_str(),
                m.start_ms as i64,
                m.end_ms as i64
            ])?;
        }
    }
    if let Some(signature) = signature {
        tx.execute(
            "INSERT INTO audio_marker_show (show_id, signature) VALUES (?1, ?2) \
             ON CONFLICT(show_id) DO UPDATE SET signature = excluded.signature",
            params![show, signature],
        )?;
    }
    crate::scan::write::bump_rev(tx)
}

pub(crate) struct MarkerHandle {
    urgent: mpsc::UnboundedSender<FileId>,
}

impl MarkerHandle {
    pub(crate) fn spawn(
        db: Db,
        root: PathBuf,
        tools: MediaTools,
        mut scans: watch::Receiver<ScanStatus>,
    ) -> std::io::Result<MarkerHandle> {
        std::fs::create_dir_all(&root)?;
        let store = Arc::new(PrintStore { root });
        let (urgent, mut requests) = mpsc::unbounded_channel::<FileId>();
        tokio::spawn(async move {
            let mut backlog: VecDeque<Show> = VecDeque::new();
            // Asked for in the player: (show, season).
            let mut asked: Option<(i64, i64)> = None;
            // A file FFmpeg can't read is tried again after a restart, not after every scan.
            let mut failed: HashSet<i64> = HashSet::new();
            let mut last_scan = None;
            loop {
                while let Ok(file) = requests.try_recv() {
                    asked = db
                        .read(move |c| show_of(c, file))
                        .await
                        .ok()
                        .flatten()
                        .or(asked);
                }
                if let Some((show, _)) = asked
                    && let Some(i) = backlog.iter().position(|s| s.id == show)
                {
                    let front = backlog.remove(i).expect("position is in range");
                    backlog.push_front(front);
                }
                if let Some(show) = backlog.pop_front() {
                    let season = asked.filter(|a| a.0 == show.id).map(|a| a.1);
                    run(&db, &store, &tools, &show, season, &mut failed).await;
                    continue;
                }
                tokio::select! {
                    request = requests.recv() => match request {
                        Some(file) => {
                            asked = db.read(move |c| show_of(c, file)).await.ok().flatten().or(asked);
                        }
                        None => return,
                    },
                    changed = scans.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        let finished = {
                            let status = scans.borrow();
                            match status.state {
                                ScanState::Idle => status.last.as_ref().map(|s| s.finished_at),
                                ScanState::Running { .. } => None,
                            }
                        };
                        if finished.is_none() || finished == last_scan {
                            continue;
                        }
                        last_scan = finished;
                        let Ok(all) = db.read(shows).await else { continue };
                        let live: HashSet<String> = all
                            .iter()
                            .flat_map(|s| &s.files)
                            .map(|f| PrintStore::name(&f.fingerprint))
                            .collect();
                        match store.retain(&live) {
                            Ok(0) => {}
                            Ok(removed) => tracing::info!(removed, "old audio prints deleted"),
                            Err(e) => tracing::warn!("cannot tidy audio prints: {e}"),
                        }
                        let Ok(todo) = db.read(stale).await else { continue };
                        backlog = todo.into();
                    }
                }
            }
        });
        Ok(MarkerHandle { urgent })
    }

    /// Puts the show `file` belongs to at the front of the queue, its season first.
    pub(crate) fn hurry(&self, file: FileId) {
        let _ = self.urgent.send(file);
    }
}

/// Prints every file of `show` (`season` first), writing markers after each
/// season so they show up before the whole show is done.
async fn run(
    db: &Db,
    store: &PrintStore,
    tools: &MediaTools,
    show: &Show,
    season: Option<i64>,
    failed: &mut HashSet<i64>,
) {
    let mut seasons: Vec<i64> = show.files.iter().map(|f| f.season).collect();
    seasons.dedup();
    if let Some(first) = season.filter(|s| seasons.contains(s)) {
        seasons.retain(|s| *s != first);
        seasons.insert(0, first);
    }
    let mut prints: HashMap<i64, AudioPrint> = HashMap::new();
    for f in &show.files {
        if let Some(p) = store.get(&f.fingerprint) {
            prints.insert(f.file, p);
        }
    }
    let total = seasons.len();
    for (n, season) in seasons.into_iter().enumerate() {
        let mut made = 0;
        for f in show.files.iter().filter(|f| f.season == season) {
            if prints.contains_key(&f.file) || failed.contains(&f.file) {
                continue;
            }
            match store.make(tools, f).await {
                Some(p) => {
                    prints.insert(f.file, p);
                    made += 1;
                }
                None => {
                    failed.insert(f.file);
                }
            }
        }
        let last = n + 1 == total;
        if made == 0 && !last {
            continue;
        }
        let (show_id, signature) = (show.id, show.signature.clone());
        let taken = std::mem::take(&mut prints);
        let files = Show {
            id: show.id,
            files: show.files.clone(),
            signature: Vec::new(),
        };
        let (found, back) = tokio::task::spawn_blocking(move || (detect(&files, &taken), taken))
            .await
            .expect("marker detection doesn't panic");
        prints = back;
        let count = found.iter().map(|(_, m)| m.len()).sum::<usize>();
        let written = db
            .write(move |tx| write(tx, show_id, &found, last.then_some(&signature[..])))
            .await;
        match written {
            Ok(()) => tracing::info!(
                show = show_id,
                season,
                markers = count,
                "markers found by sound"
            ),
            Err(e) => tracing::warn!(show = show_id, "cannot save markers: {e}"),
        }
    }
}
