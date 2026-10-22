//! Phase 2: pure planning. Given what is stored and what is on disk, decide what
//! to do. No I/O except through the fingerprint closure, so the whole
//! rename/move/removal policy is unit-tested with in-memory tables.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nuvora_media::{ArtworkRole, PROBE_VERSION};

use super::walk::{DiskFile, RelPath, RootId, RootWalk};
use crate::{FileId, ItemId};

/// A stored `media_file` row, as much as planning needs.
#[derive(Debug, Clone)]
pub(crate) struct KnownFile {
    pub id: FileId,
    pub root: RootId,
    pub rel: RelPath,
    pub size: u64,
    pub mtime_ns: i64,
    pub fingerprint: [u8; 16],
    pub probe_version: u32,
}

/// A file to (re)read with ffprobe and upsert.
#[derive(Debug, Clone)]
pub(crate) struct ProbeJob {
    pub root: RootId,
    pub library_id: i64,
    pub disk: DiskFile,
    /// `Some` when the path is already stored (content changed, or
    /// `probe_version` is stale): the row is updated in place, id kept.
    pub existing: Option<FileId>,
}

#[derive(Debug, Clone)]
pub(crate) struct Move {
    pub id: FileId,
    pub to_root: RootId,
    pub to_library_id: i64,
    pub to: DiskFile,
}

#[derive(Debug, Default)]
pub(crate) struct ScanPlan {
    pub unchanged: u32,
    /// Modified within the settle window; probably still copying. Left for the
    /// next scan, and neither probed nor treated as gone.
    pub settling: u32,
    pub to_probe: Vec<ProbeJob>,
    pub moved: Vec<Move>,
    /// Stored files absent from a *completely* walked part of a root whose
    /// removals are allowed this scan.
    pub gone: Vec<FileId>,
    /// Roots whose removals are held back: the root was unreadable, or it
    /// walked as empty while the database holds files for it, which is what an
    /// automounter mount point looks like when the share is down. Each root is
    /// judged on its own, so an offline share never blocks cleanup elsewhere.
    /// Moves still apply.
    pub held_roots: Vec<RootId>,
}

/// Diff stored files against walks. Rules:
/// - same path, same (size, mtime) and current probe_version => unchanged;
/// - same path, different (size, mtime) or stale probe_version => to_probe(existing);
/// - path only on disk => to_probe(new), candidate for move pairing;
/// - path only in store, under a complete root that is not held and not under
///   an unreadable subfolder => gone, candidate for move pairing; otherwise
///   left untouched;
/// - a file modified within `settle` is counted as settling and skipped.
///
/// Deterministic: inputs are sorted by (root, rel) first.
pub(crate) fn diff(
    mut known: Vec<KnownFile>,
    walks: &[RootWalk],
    now: SystemTime,
    settle: Duration,
) -> ScanPlan {
    known.sort_by(|a, b| (a.root, &a.rel).cmp(&(b.root, &b.rel)));
    let settle_cutoff_ns = now
        .checked_sub(settle)
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(i64::MAX, |d| d.as_nanos() as i64);
    let known_by_path: HashMap<(RootId, &RelPath), &KnownFile> =
        known.iter().map(|k| ((k.root, &k.rel), k)).collect();
    let mut plan = ScanPlan::default();
    let mut seen = HashSet::new();

    let mut walks: Vec<&RootWalk> = walks.iter().collect();
    walks.sort_by_key(|w| w.root);
    for walk in &walks {
        let stored_under_root = known.iter().filter(|k| k.root == walk.root).count();
        if !walk.complete || (walk.videos.is_empty() && stored_under_root > 0) {
            plan.held_roots.push(walk.root);
        }
        if !walk.complete {
            continue;
        }
        for disk in &walk.videos {
            let stored = known_by_path.get(&(walk.root, &disk.rel)).copied();
            if let Some(k) = stored {
                seen.insert(k.id);
            }
            if disk.mtime_ns > settle_cutoff_ns {
                plan.settling += 1;
                continue;
            }
            match stored {
                Some(k)
                    if k.size == disk.size
                        && k.mtime_ns == disk.mtime_ns
                        && k.probe_version == PROBE_VERSION =>
                {
                    plan.unchanged += 1;
                }
                _ => plan.to_probe.push(ProbeJob {
                    root: walk.root,
                    library_id: walk.library_id,
                    disk: disk.clone(),
                    existing: stored.map(|k| k.id),
                }),
            }
        }
    }

    for k in &known {
        if seen.contains(&k.id) || plan.held_roots.contains(&k.root) {
            continue;
        }
        let Some(walk) = walks.iter().find(|w| w.root == k.root) else {
            continue;
        };
        if walk.unreadable.iter().any(|folder| k.rel.is_under(folder)) {
            continue;
        }
        plan.gone.push(k.id);
    }
    plan.to_probe
        .sort_by(|a, b| (a.root, &a.disk.rel).cmp(&(b.root, &b.disk.rel)));
    plan
}

/// Pair gone files with new files: only pairs with equal size are
/// fingerprinted (so a 2,000-file rescan with no moves reads nothing), and a
/// pair needs equal fingerprints. Each paired file moves from `gone` and
/// `to_probe` into `moved`. Ambiguous matches (two new files with one gone
/// file's fingerprint, i.e. a copy) pair the lexicographically first and treat
/// the rest as new.
pub(crate) fn pair_moves(
    mut plan: ScanPlan,
    known: &[KnownFile],
    mut fingerprint: impl FnMut(&ProbeJob) -> std::io::Result<[u8; 16]>,
) -> ScanPlan {
    let gone: HashSet<FileId> = plan.gone.iter().copied().collect();
    let mut candidates: BTreeMap<u64, Vec<&KnownFile>> = BTreeMap::new();
    let mut by_path: Vec<&KnownFile> = known.iter().filter(|k| gone.contains(&k.id)).collect();
    by_path.sort_by(|a, b| (a.root, &a.rel).cmp(&(b.root, &b.rel)));
    for k in by_path {
        candidates.entry(k.size).or_default().push(k);
    }
    if candidates.is_empty() {
        return plan;
    }

    let mut paired = HashSet::new();
    let mut to_probe = Vec::with_capacity(plan.to_probe.len());
    let jobs = std::mem::take(&mut plan.to_probe);
    for job in jobs {
        let matched = job.existing.is_none()
            && candidates.get_mut(&job.disk.size).is_some_and(|gone| {
                let Ok(fp) = fingerprint(&job) else {
                    return false;
                };
                let Some(pos) = gone.iter().position(|k| k.fingerprint == fp) else {
                    return false;
                };
                let k = gone.remove(pos);
                paired.insert(k.id);
                plan.moved.push(Move {
                    id: k.id,
                    to_root: job.root,
                    to_library_id: job.library_id,
                    to: job.disk.clone(),
                });
                true
            });
        if !matched {
            to_probe.push(job);
        }
    }
    plan.to_probe = to_probe;
    plan.gone.retain(|id| !paired.contains(id));
    plan
}

/// A local image file. Folder rule: generic names (`poster`, `folder`,
/// `cover` / `fanart`, `backdrop`, `background`) count only when the folder
/// holds exactly one item's videos; `<video stem>-poster` / `-fanart` always count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalImage {
    pub root: RootId,
    pub rel: RelPath,
    pub size: u64,
    pub mtime_ns: i64,
}

/// The artwork one present item should have. A poster is local or nothing (the
/// web app then draws a typographic card), so a frame poster is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WantedArtwork {
    pub item: ItemId,
    pub poster: Option<LocalImage>,
    pub backdrop: Option<WantedSource>,
}

/// Where a backdrop comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WantedSource {
    Local(LocalImage),
    /// No local backdrop: a frame at 10% of the runtime of the item's best SDR
    /// version. An item whose versions are all HDR gets no backdrop.
    Frame {
        file: FileId,
        root: RootId,
        rel: RelPath,
        fingerprint: [u8; 16],
        at_ms: u64,
    },
}

impl WantedSource {
    /// The `artwork.source` column.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            WantedSource::Local(_) => "local",
            WantedSource::Frame { .. } => "frame",
        }
    }

    /// The `artwork.source_sig` string: equal sig => the stored render is current.
    pub(crate) fn sig(&self) -> String {
        match self {
            WantedSource::Local(img) => format!(
                "local:{}:{}:{}:{}",
                img.root.0, img.rel.0, img.size, img.mtime_ns
            ),
            WantedSource::Frame {
                file,
                fingerprint,
                at_ms,
                ..
            } => {
                let hex: String = fingerprint.iter().map(|b| format!("{b:02x}")).collect();
                format!("frame:{}:{hex}:{at_ms}", file.0)
            }
        }
    }
}

/// A present item's files, as much as artwork planning needs.
#[derive(Debug, Clone)]
pub(crate) struct ItemFiles {
    pub item: ItemId,
    pub files: Vec<FileForArtwork>,
}

#[derive(Debug, Clone)]
pub(crate) struct FileForArtwork {
    pub id: FileId,
    pub root: RootId,
    pub rel: RelPath,
    pub fingerprint: [u8; 16],
    pub duration_ms: Option<u64>,
    /// Probed, with an SDR primary video stream: eligible for a frame grab.
    pub sdr: bool,
    pub height: u32,
}

const POSTER_NAMES: &[&str] = &["poster", "folder", "cover"];
const BACKDROP_NAMES: &[&str] = &["fanart", "backdrop", "background"];

/// The artwork every present item should have, from the walked images and the
/// items' files. An item with a file under a held root or an unreadable folder
/// is left out: its images are unknown this scan, so its rows stay as they are.
pub(crate) fn wanted_artwork(
    items: &[ItemFiles],
    walks: &[RootWalk],
    held_roots: &[RootId],
) -> Vec<WantedArtwork> {
    let unknown = |file: &FileForArtwork| {
        let Some(walk) = walks.iter().find(|w| w.root == file.root) else {
            return true;
        };
        !walk.complete
            || held_roots.contains(&file.root)
            || walk.unreadable.iter().any(|f| file.rel.is_under(f))
    };
    let mut images: HashMap<(RootId, &str), Vec<(String, &DiskFile)>> = HashMap::new();
    for walk in walks {
        for image in &walk.images {
            let (folder, stem) = image.rel.folder_and_stem();
            images
                .entry((walk.root, folder))
                .or_default()
                .push((stem, image));
        }
    }
    let mut items_in_folder: HashMap<(RootId, &str), HashSet<ItemId>> = HashMap::new();
    for item in items {
        for file in &item.files {
            items_in_folder
                .entry((file.root, file.rel.folder_and_stem().0))
                .or_default()
                .insert(item.item);
        }
    }

    items
        .iter()
        .filter(|item| !item.files.iter().any(unknown))
        .map(|item| {
            let mut poster = None;
            let mut backdrop = None;
            for file in &item.files {
                let (folder, stem) = file.rel.folder_and_stem();
                let alone = items_in_folder
                    .get(&(file.root, folder))
                    .is_some_and(|set| set.len() == 1);
                let Some(candidates) = images.get(&(file.root, folder)) else {
                    continue;
                };
                let find = |specific: &[String], generic: &[&str]| {
                    let wanted = |name: &str| {
                        specific.iter().any(|s| s == name) || (alone && generic.contains(&name))
                    };
                    let (_, image) = candidates.iter().find(|(stem, _)| wanted(stem))?;
                    Some(LocalImage {
                        root: file.root,
                        rel: image.rel.clone(),
                        size: image.size,
                        mtime_ns: image.mtime_ns,
                    })
                };
                poster = poster.or_else(|| find(&[format!("{stem}-poster")], POSTER_NAMES));
                backdrop = backdrop.or_else(|| {
                    find(
                        &[format!("{stem}-fanart"), format!("{stem}-backdrop")],
                        BACKDROP_NAMES,
                    )
                    .map(WantedSource::Local)
                });
            }
            let backdrop = backdrop.or_else(|| {
                let best = item
                    .files
                    .iter()
                    .filter(|f| f.sdr)
                    .max_by_key(|f| (f.height, std::cmp::Reverse(f.id)))?;
                Some(WantedSource::Frame {
                    file: best.id,
                    root: best.root,
                    rel: best.rel.clone(),
                    fingerprint: best.fingerprint,
                    at_ms: best.duration_ms.unwrap_or(0) / 10,
                })
            });
            WantedArtwork {
                item: item.item,
                poster,
                backdrop,
            }
        })
        .collect()
}

/// One stored `artwork` row's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredArtwork {
    pub item: ItemId,
    pub role: ArtworkRole,
    pub source: String,
    pub sig: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderJob {
    pub item: ItemId,
    pub role: ArtworkRole,
    pub source: WantedSource,
}

/// What to render (wanted and not stored with the same sig) and which stored
/// rows are stale (a scanner-owned source that is no longer the wanted one).
/// Rows from other sources (Step 2's `tmdb`, `user`) are never touched.
pub(crate) fn artwork_diff(
    wanted: &[WantedArtwork],
    stored: &[StoredArtwork],
) -> (Vec<RenderJob>, Vec<(ItemId, ArtworkRole, &'static str)>) {
    let sigs: HashMap<(ItemId, ArtworkRole, &str), &str> = stored
        .iter()
        .map(|s| ((s.item, s.role, s.source.as_str()), s.sig.as_str()))
        .collect();
    let mut render = Vec::new();
    let mut stale = Vec::new();
    for want in wanted {
        let roles = [
            (
                ArtworkRole::Poster,
                want.poster.clone().map(WantedSource::Local),
            ),
            (ArtworkRole::Backdrop, want.backdrop.clone()),
        ];
        for (role, source) in roles {
            let wanted_name = source.as_ref().map(WantedSource::name);
            if let Some(source) = source
                && sigs.get(&(want.item, role, source.name())) != Some(&source.sig().as_str())
            {
                render.push(RenderJob {
                    item: want.item,
                    role,
                    source,
                });
            }
            for row in stored
                .iter()
                .filter(|s| s.item == want.item && s.role == role)
            {
                let scanner_owned = match row.source.as_str() {
                    "local" => "local",
                    "frame" => "frame",
                    _ => continue,
                };
                if Some(scanner_owned) != wanted_name {
                    stale.push((want.item, role, scanner_owned));
                }
            }
        }
    }
    (render, stale)
}

#[cfg(test)]
mod tests {
    //! Table tests over (stored rows, walk) -> plan:
    //! - unchanged library => only `unchanged`, no fingerprint calls;
    //! - file renamed in place => one Move, nothing gone, nothing probed;
    //! - folder renamed (`Alien/` -> `Alien (1979)/`) with poster => Move;
    //! - file replaced by a new encode at the same path => to_probe(existing);
    //! - root unreadable => nothing gone; subfolder unreadable => its files kept;
    //! - file mtime 10 s ago => settling, not probed, not gone;
    //! - running diff+pair twice on its own output state yields an empty plan.

    use super::*;
    use std::path::Path;

    const NOW_NS: i64 = 1_700_000_000_000_000_000;
    const FP: [u8; 16] = [7; 16];

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_nanos(NOW_NS as u64)
    }

    fn known(id: i64, rel: &str, size: u64) -> KnownFile {
        KnownFile {
            id: FileId(id),
            root: RootId(1),
            rel: RelPath(rel.into()),
            size,
            mtime_ns: NOW_NS - 86_400_000_000_000,
            fingerprint: FP,
            probe_version: PROBE_VERSION,
        }
    }

    fn disk(rel: &str, size: u64) -> DiskFile {
        DiskFile {
            rel: RelPath(rel.into()),
            size,
            mtime_ns: NOW_NS - 86_400_000_000_000,
        }
    }

    fn walk(videos: &[DiskFile]) -> RootWalk {
        let mut walk = RootWalk::empty(RootId(1), 1, Path::new("/media"), true);
        walk.videos = videos.to_vec();
        walk
    }

    fn settle() -> Duration {
        Duration::from_secs(60)
    }

    fn no_fingerprint(job: &ProbeJob) -> std::io::Result<[u8; 16]> {
        panic!("fingerprint requested for {:?}", job.disk.rel)
    }

    #[test]
    fn unchanged_library_plans_nothing() {
        let stored = vec![
            known(1, "Alien (1979)/Alien (1979).mkv", 10),
            known(2, "Heat (1995)/Heat (1995).mkv", 20),
        ];
        let walks = [walk(&[
            disk("Alien (1979)/Alien (1979).mkv", 10),
            disk("Heat (1995)/Heat (1995).mkv", 20),
        ])];
        let plan = pair_moves(
            diff(stored.clone(), &walks, now(), settle()),
            &stored,
            no_fingerprint,
        );
        assert_eq!(plan.unchanged, 2);
        assert!(
            plan.to_probe.is_empty()
                && plan.moved.is_empty()
                && plan.gone.is_empty()
                && plan.held_roots.is_empty()
        );
    }

    #[test]
    fn rename_in_place_is_a_move() {
        let stored = vec![known(1, "Alien/Alien.mkv", 10)];
        let walks = [walk(&[disk("Alien (1979)/Alien (1979).mkv", 10)])];
        let plan = pair_moves(
            diff(stored.clone(), &walks, now(), settle()),
            &stored,
            |_| Ok(FP),
        );
        assert_eq!(plan.moved.len(), 1);
        assert_eq!(
            (plan.moved[0].id, plan.moved[0].to.rel.0.as_str()),
            (FileId(1), "Alien (1979)/Alien (1979).mkv")
        );
        assert!(plan.gone.is_empty() && plan.to_probe.is_empty());
    }

    #[test]
    fn different_content_at_a_moved_path_is_new_and_gone() {
        let stored = vec![known(1, "Alien/Alien.mkv", 10)];
        let walks = [walk(&[disk("Alien (1979)/Alien (1979).mkv", 10)])];
        let plan = pair_moves(
            diff(stored.clone(), &walks, now(), settle()),
            &stored,
            |_| Ok([9; 16]),
        );
        assert!(plan.moved.is_empty());
        assert_eq!(plan.gone, [FileId(1)]);
        assert_eq!(plan.to_probe.len(), 1);
    }

    #[test]
    fn only_equal_sizes_are_fingerprinted() {
        let stored = vec![known(1, "Alien/Alien.mkv", 10)];
        let walks = [walk(&[disk("Alien (1979)/Alien (1979).mkv", 11)])];
        let plan = pair_moves(
            diff(stored.clone(), &walks, now(), settle()),
            &stored,
            no_fingerprint,
        );
        assert_eq!(
            (plan.gone.len(), plan.to_probe.len(), plan.moved.len()),
            (1, 1, 0)
        );
    }

    #[test]
    fn a_copy_pairs_the_first_and_leaves_the_rest_new() {
        let stored = vec![known(1, "Alien/Alien.mkv", 10)];
        let walks = [walk(&[disk("B/Alien.mkv", 10), disk("A/Alien.mkv", 10)])];
        let plan = pair_moves(
            diff(stored.clone(), &walks, now(), settle()),
            &stored,
            |_| Ok(FP),
        );
        assert_eq!(plan.moved[0].to.rel.0, "A/Alien.mkv");
        assert_eq!(plan.to_probe[0].disk.rel.0, "B/Alien.mkv");
        assert!(plan.gone.is_empty());
    }

    #[test]
    fn replaced_or_stale_files_are_reprobed_in_place() {
        let mut stale = known(2, "Heat (1995)/Heat (1995).mkv", 20);
        stale.probe_version = PROBE_VERSION - 1;
        let stored = vec![known(1, "Alien (1979)/Alien (1979).mkv", 10), stale];
        let walks = [walk(&[
            disk("Alien (1979)/Alien (1979).mkv", 11),
            disk("Heat (1995)/Heat (1995).mkv", 20),
        ])];
        let plan = diff(stored, &walks, now(), settle());
        let existing: Vec<Option<FileId>> = plan.to_probe.iter().map(|j| j.existing).collect();
        assert_eq!(existing, [Some(FileId(1)), Some(FileId(2))]);
        assert!(plan.gone.is_empty());
    }

    #[test]
    fn settling_files_wait_for_the_next_scan() {
        let stored = vec![known(1, "Alien (1979)/Alien (1979).mkv", 10)];
        let mut fresh = disk("Alien (1979)/Alien (1979).mkv", 999);
        fresh.mtime_ns = NOW_NS - 10_000_000_000;
        let mut new = disk("New (2020)/New (2020).mkv", 5);
        new.mtime_ns = NOW_NS;
        let plan = diff(stored, &[walk(&[fresh, new])], now(), settle());
        assert_eq!(plan.settling, 2);
        assert!(plan.to_probe.is_empty() && plan.gone.is_empty());
        let plan = diff(
            vec![known(1, "Alien (1979)/Alien (1979).mkv", 10)],
            &[walk(&[disk("x.mkv", 1)])],
            now(),
            Duration::ZERO,
        );
        assert_eq!((plan.settling, plan.to_probe.len()), (0, 1));
    }

    #[test]
    fn unreadable_root_blocks_missing_marks() {
        let stored = vec![known(1, "Alien (1979)/Alien (1979).mkv", 10)];
        let mut offline = walk(&[]);
        offline.complete = false;
        let plan = diff(stored, &[offline], now(), settle());
        assert!(plan.gone.is_empty());
        assert_eq!(plan.held_roots, [RootId(1)]);
    }

    #[test]
    fn empty_walk_of_populated_root_blocks_missing_marks() {
        let stored = vec![known(1, "Alien (1979)/Alien (1979).mkv", 10)];
        let plan = diff(stored, &[walk(&[])], now(), settle());
        assert!(plan.gone.is_empty());
        assert_eq!(plan.held_roots, [RootId(1)]);
        let plan = diff(vec![], &[walk(&[])], now(), settle());
        assert!(
            plan.held_roots.is_empty(),
            "an empty root with nothing stored is just empty"
        );
    }

    #[test]
    fn complete_walks_allow_missing_marks() {
        let stored = vec![
            known(1, "Alien (1979)/Alien (1979).mkv", 10),
            known(2, "Heat (1995)/Heat (1995).mkv", 20),
        ];
        let plan = diff(
            stored,
            &[walk(&[disk("Heat (1995)/Heat (1995).mkv", 20)])],
            now(),
            settle(),
        );
        assert_eq!(plan.gone, [FileId(1)]);
        assert!(plan.held_roots.is_empty());
    }

    #[test]
    fn held_roots_are_judged_one_by_one() {
        let mut other = known(2, "Heat (1995)/Heat (1995).mkv", 20);
        other.root = RootId(2);
        let stored = vec![known(1, "Alien (1979)/Alien (1979).mkv", 10), other];
        let mut offline = RootWalk::empty(RootId(2), 1, Path::new("/nas"), false);
        offline.complete = false;
        let online = walk(&[disk("Other (2000)/Other (2000).mkv", 1)]);
        let plan = diff(stored, &[offline, online], now(), settle());
        assert_eq!(plan.gone, [FileId(1)], "the online root is cleaned up");
        assert_eq!(plan.held_roots, [RootId(2)]);
    }

    #[test]
    fn unreadable_subfolder_keeps_its_files() {
        let stored = vec![
            known(1, "Locked (2001)/Locked (2001).mkv", 10),
            known(2, "Gone (2002)/Gone (2002).mkv", 20),
        ];
        let mut w = walk(&[disk("Open (2000)/Open (2000).mkv", 1)]);
        w.unreadable = vec![RelPath("Locked (2001)".into())];
        let plan = diff(stored, &[w], now(), settle());
        assert_eq!(plan.gone, [FileId(2)]);
    }

    fn file(id: i64, rel: &str, sdr: bool, height: u32) -> FileForArtwork {
        FileForArtwork {
            id: FileId(id),
            root: RootId(1),
            rel: RelPath(rel.into()),
            fingerprint: FP,
            duration_ms: Some(100_000),
            sdr,
            height,
        }
    }

    #[test]
    fn wanted_artwork_follows_the_folder_rule() {
        let items = vec![
            ItemFiles {
                item: ItemId(1),
                files: vec![file(1, "Alien (1979)/Alien (1979).mkv", true, 1080)],
            },
            ItemFiles {
                item: ItemId(2),
                files: vec![file(2, "Loose/Heat.1995.mkv", true, 720)],
            },
            ItemFiles {
                item: ItemId(3),
                files: vec![
                    file(3, "Loose/Ronin.1998.mkv", true, 1080),
                    file(4, "Loose/Ronin.1998.4k.mkv", false, 2160),
                ],
            },
            ItemFiles {
                item: ItemId(4),
                files: vec![file(5, "HDR (2020)/HDR (2020).mkv", false, 2160)],
            },
        ];
        let mut w = walk(&[]);
        w.images = vec![
            disk("Alien (1979)/poster.jpg", 1),
            disk("Alien (1979)/fanart.jpg", 2),
            disk("Loose/poster.jpg", 3),
            disk("Loose/Ronin.1998-poster.png", 4),
        ];
        let wanted = wanted_artwork(&items, &[w], &[]);
        assert_eq!(
            wanted[0].poster.as_ref().map(|p| p.rel.0.as_str()),
            Some("Alien (1979)/poster.jpg")
        );
        assert!(
            matches!(&wanted[0].backdrop, Some(WantedSource::Local(img)) if img.rel.0 == "Alien (1979)/fanart.jpg")
        );
        assert_eq!(
            wanted[1].poster, None,
            "generic poster.jpg in a folder with two movies counts for neither"
        );
        assert!(matches!(
            &wanted[1].backdrop,
            Some(WantedSource::Frame {
                file: FileId(2),
                at_ms: 10_000,
                ..
            })
        ));
        assert_eq!(
            wanted[2].poster.as_ref().map(|p| p.rel.0.as_str()),
            Some("Loose/Ronin.1998-poster.png")
        );
        assert!(
            matches!(
                &wanted[2].backdrop,
                Some(WantedSource::Frame {
                    file: FileId(3),
                    ..
                })
            ),
            "the SDR version, not the taller HDR one"
        );
        assert_eq!(
            (wanted[3].poster.as_ref(), wanted[3].backdrop.as_ref()),
            (None, None),
            "all-HDR item gets no frame"
        );
    }

    #[test]
    fn artwork_is_not_planned_for_items_under_held_or_unreadable_folders() {
        let items = vec![
            ItemFiles {
                item: ItemId(1),
                files: vec![file(1, "Alien (1979)/Alien (1979).mkv", true, 1080)],
            },
            ItemFiles {
                item: ItemId(2),
                files: vec![file(2, "Locked (2001)/Locked (2001).mkv", true, 1080)],
            },
        ];
        let mut w = walk(&[]);
        w.unreadable = vec![RelPath("Locked (2001)".into())];
        let wanted = wanted_artwork(&items, &[w], &[]);
        assert_eq!(
            wanted.iter().map(|w| w.item).collect::<Vec<_>>(),
            [ItemId(1)]
        );
        let held = wanted_artwork(&items, &[walk(&[])], &[RootId(1)]);
        assert!(held.is_empty());
    }

    #[test]
    fn artwork_diff_renders_changed_sources_and_flags_stale_rows() {
        let local = |rel: &str, mtime: i64| LocalImage {
            root: RootId(1),
            rel: RelPath(rel.into()),
            size: 1,
            mtime_ns: mtime,
        };
        let wanted = vec![
            WantedArtwork {
                item: ItemId(1),
                poster: Some(local("A/poster.jpg", 5)),
                backdrop: Some(WantedSource::Local(local("A/fanart.jpg", 5))),
            },
            WantedArtwork {
                item: ItemId(2),
                poster: None,
                backdrop: Some(WantedSource::Frame {
                    file: FileId(2),
                    root: RootId(1),
                    rel: RelPath("B/b.mkv".into()),
                    fingerprint: FP,
                    at_ms: 10,
                }),
            },
        ];
        let stored = vec![
            StoredArtwork {
                item: ItemId(1),
                role: ArtworkRole::Poster,
                source: "local".into(),
                sig: WantedSource::Local(local("A/poster.jpg", 5)).sig(),
            },
            StoredArtwork {
                item: ItemId(1),
                role: ArtworkRole::Backdrop,
                source: "frame".into(),
                sig: "frame:1:00:10".into(),
            },
            StoredArtwork {
                item: ItemId(2),
                role: ArtworkRole::Poster,
                source: "local".into(),
                sig: "local:1:B/poster.jpg:1:1".into(),
            },
            StoredArtwork {
                item: ItemId(2),
                role: ArtworkRole::Poster,
                source: "tmdb".into(),
                sig: "tmdb:1".into(),
            },
            StoredArtwork {
                item: ItemId(2),
                role: ArtworkRole::Backdrop,
                source: "frame".into(),
                sig: "frame:2:stale:10".into(),
            },
        ];
        let (render, stale) = artwork_diff(&wanted, &stored);
        let rendered: Vec<(ItemId, ArtworkRole, &str)> = render
            .iter()
            .map(|r| (r.item, r.role, r.source.name()))
            .collect();
        assert_eq!(
            rendered,
            [
                (ItemId(1), ArtworkRole::Backdrop, "local"),
                (ItemId(2), ArtworkRole::Backdrop, "frame")
            ]
        );
        assert_eq!(
            stale,
            [
                (ItemId(1), ArtworkRole::Backdrop, "frame"),
                (ItemId(2), ArtworkRole::Poster, "local")
            ]
        );
    }
}
