//! Shows end to end: real files on disk, a real scan, then what home, the
//! show page and the library page read back.

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{Duration, SystemTime};

use nuvora_media::{DeviceProfile, MediaTools};

use crate::test_support::sdr_movie;
use crate::{
    CardNote, EpisodeNumber, Item, ItemKind, Library, LibraryConfig, LibraryKind, LibraryQuery,
    LibrarySpec, NewUser, Role, RowKind, ScanState, ScanSummary, UserId, WatchEvent,
};

async fn open(root: &Path, data: &Path, kind: LibraryKind) -> Library {
    Library::open(LibraryConfig {
        data_dir: data.to_path_buf(),
        libraries: vec![LibrarySpec {
            key: "tv".into(),
            name: "TV".into(),
            kind,
            roots: vec![root.to_path_buf()],
        }],
        tools: MediaTools::locate(None, None).await.unwrap(),
        probe_concurrency: NonZeroUsize::new(4).unwrap(),
        settle_window: Duration::ZERO,
        online_metadata: false,
    })
    .await
    .unwrap()
}

async fn scan(lib: &Library) -> ScanSummary {
    let before = SystemTime::now();
    let mut status = lib.watch_scan();
    lib.request_scan();
    status
        .wait_for(|s| {
            matches!(s.state, ScanState::Idle)
                && s.last.as_ref().is_some_and(|l| l.started_at >= before)
        })
        .await
        .unwrap()
        .last
        .clone()
        .unwrap()
}

async fn user(lib: &Library, name: &str) -> UserId {
    let new = NewUser {
        username: name.into(),
        display_name: name.into(),
        password_hash: "not a hash".into(),
        role: Role::Member,
    };
    if name == "ada" {
        lib.create_first_admin(new, 0).await.unwrap().id
    } else {
        lib.create_user(new, 0).await.unwrap().id
    }
}

/// One generated clip copied to every path (with distinct trailing bytes,
/// so no two files look like one moved file).
fn clips(root: &Path, paths: &[&str]) {
    let first = root.join(paths[0]);
    sdr_movie(&first);
    let bytes = std::fs::read(&first).unwrap();
    for (i, path) in paths.iter().enumerate().skip(1) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut copy = bytes.clone();
        copy.extend((i as u64).to_le_bytes());
        std::fs::write(path, copy).unwrap();
    }
}

const TOWER: &str = "[Ñ] Tower of God 2 - Ouji no Kikan [1080p]";

/// A TV library: the owner's anime layout (two episodes), a scene-named
/// show in season folders with a special and a folder poster, and a file
/// that is no episode.
fn tv_library(root: &Path) {
    clips(
        root,
        &[
            &format!("{TOWER}/[Ñ] Tower of God 2 - Ouji no Kikan - 01 [1080p] [080DD9EA].mp4"),
            &format!("{TOWER}/[Ñ] Tower of God 2 - Ouji no Kikan - 02 [1080p] [C6F9D246].mp4"),
            "Dark (2017)/Season 1/Dark.S01E01.Secrets.720p.mp4",
            "Dark (2017)/Season 1/Dark.S01E02.Lies.720p.mp4",
            "Dark (2017)/Season 2/Dark.S02E01.Beginnings.720p.mp4",
            "Dark (2017)/Specials/Dark.S00E01.Making.Of.mp4",
            "BigBuckBunny_512kb.mp4",
        ],
    );
    crate::test_support::ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=600x900",
            "-frames:v",
            "1",
        ],
        &root.join("Dark (2017)/poster.jpg"),
    );
}

async fn show_titled(lib: &Library, title: &str, user: UserId) -> crate::Show {
    let library = lib.libraries().await.unwrap()[0].id;
    let page = lib
        .library_items(library, LibraryQuery::default(), None, 100, user)
        .await
        .unwrap()
        .unwrap();
    let card = page
        .cards
        .iter()
        .find(|c| c.title == title)
        .unwrap_or_else(|| panic!("{title} in {:?}", page.cards));
    let Some(Item::Show(show)) = lib
        .item(card.id, DeviceProfile::baseline(), user)
        .await
        .unwrap()
    else {
        panic!("{title} is a show")
    };
    show
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tv_library_groups_episodes_into_shows_and_seasons() {
    let dir = tempfile::tempdir().unwrap();
    let (root, data) = (dir.path().join("TV"), dir.path().join("data"));
    tv_library(&root);
    let lib = open(&root, &data, LibraryKind::Shows).await;
    let summary = scan(&lib).await;
    assert_eq!((summary.added, summary.failed), (6, 0), "{summary:?}");
    assert!(
        summary
            .issues
            .iter()
            .any(|i| i.path == "BigBuckBunny_512kb.mp4" && i.message.starts_with("skipped")),
        "{:?}",
        summary.issues
    );
    let me = user(&lib, "ada").await;

    let libraries = lib.libraries().await.unwrap();
    assert_eq!(
        (libraries[0].kind, libraries[0].item_count),
        (LibraryKind::Shows, 2)
    );
    let page = lib
        .library_items(libraries[0].id, LibraryQuery::default(), None, 100, me)
        .await
        .unwrap()
        .unwrap();
    let cards: Vec<(&str, ItemKind, Option<u16>, Option<u32>)> = page
        .cards
        .iter()
        .map(|c| (c.title.as_str(), c.kind, c.year, c.unwatched))
        .collect();
    assert_eq!(
        cards,
        [
            ("Dark", ItemKind::Show, Some(2017), Some(4)),
            (
                "Tower of God 2 - Ouji no Kikan",
                ItemKind::Show,
                None,
                Some(2)
            ),
        ]
    );
    assert!(page.cards[0].poster.is_some(), "Dark/poster.jpg");

    let dark = show_titled(&lib, "Dark", me).await;
    let seasons: Vec<(u32, u32)> = dark
        .seasons
        .iter()
        .map(|s| (s.number, s.episode_count))
        .collect();
    assert_eq!(seasons, [(1, 2), (2, 1), (0, 1)], "Specials last");
    assert_eq!((dark.episode_count, dark.watched_count), (4, 0));
    assert!(dark.backdrop.is_some(), "a frame of S1E1");
    let next = dark.next.as_ref().unwrap();
    assert_eq!(
        (next.number, next.title.as_deref()),
        (
            EpisodeNumber {
                season: 1,
                episode: 1
            },
            Some("Secrets")
        )
    );

    let episodes = lib
        .episodes(dark.seasons[0].id, DeviceProfile::baseline(), me)
        .await
        .unwrap()
        .unwrap();
    let listed: Vec<(u32, Option<&str>, bool, usize)> = episodes
        .iter()
        .map(|e| {
            (
                e.number.episode,
                e.title.as_deref(),
                e.thumb.is_some(),
                e.versions.len(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [(1, Some("Secrets"), true, 1), (2, Some("Lies"), true, 1)]
    );
    assert!(
        episodes[0]
            .runtime_ms()
            .is_some_and(|ms| (900..1200).contains(&ms))
    );
    assert!(
        lib.episodes(dark.id, DeviceProfile::baseline(), me)
            .await
            .unwrap()
            .is_none(),
        "a show id is not a season"
    );

    let tower = show_titled(&lib, "Tower of God 2 - Ouji no Kikan", me).await;
    assert_eq!(tower.seasons.len(), 1);
    let Some(Item::Episode(e1)) = lib
        .item(tower.next.unwrap().id, DeviceProfile::baseline(), me)
        .await
        .unwrap()
    else {
        panic!("an episode")
    };
    assert_eq!(
        (
            e1.show_title.as_str(),
            e1.number.episode,
            e1.title.as_deref()
        ),
        ("Tower of God 2 - Ouji no Kikan", 1, None)
    );
    assert_eq!(e1.next.as_ref().map(|n| n.number.episode), Some(2));
    let Some(Item::Season(season)) = lib
        .item(e1.season_id, DeviceProfile::baseline(), me)
        .await
        .unwrap()
    else {
        panic!("a season")
    };
    assert_eq!((season.number, season.episode_count), (1, 2));

    let rescan = scan(&lib).await;
    assert_eq!((rescan.unchanged, rescan.added), (6, 0), "{rescan:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn next_up_follows_the_last_watched_episode() {
    let dir = tempfile::tempdir().unwrap();
    let (root, data) = (dir.path().join("TV"), dir.path().join("data"));
    tv_library(&root);
    let lib = open(&root, &data, LibraryKind::Shows).await;
    scan(&lib).await;
    let (ada, bo) = (user(&lib, "ada").await, user(&lib, "bo").await);
    let dark = show_titled(&lib, "Dark", ada).await;
    let tower = show_titled(&lib, "Tower of God 2 - Ouji no Kikan", ada).await;
    let season_one = lib
        .episodes(dark.seasons[0].id, DeviceProfile::baseline(), ada)
        .await
        .unwrap()
        .unwrap();
    let tower_e1 = tower.next.as_ref().unwrap().id;

    let watching = |lib: &Library, user| {
        let lib = lib.clone();
        async move {
            let page = lib
                .row(RowKind::ContinueWatching, None, 40, user)
                .await
                .unwrap();
            page.cards
                .iter()
                .map(|c| (c.title.clone(), c.note, c.show))
                .collect::<Vec<_>>()
        }
    };
    let ep = |season, episode| EpisodeNumber { season, episode };

    lib.record_watch(ada, tower_e1, WatchEvent::Watched, 1_000)
        .await
        .unwrap();
    lib.record_watch(ada, season_one[0].id, WatchEvent::Watched, 2_000)
        .await
        .unwrap();
    let file = season_one[1].versions[0].file_id;
    lib.record_watch(
        ada,
        season_one[1].id,
        WatchEvent::Progress {
            file_id: file,
            position_ms: 61_000,
            duration_ms: 100_000,
        },
        3_000,
    )
    .await
    .unwrap();
    assert_eq!(
        watching(&lib, ada).await,
        [
            (
                "Dark".to_owned(),
                CardNote::Episode(ep(1, 2)),
                Some(dark.id)
            ),
            (
                "Tower of God 2 - Ouji no Kikan".to_owned(),
                CardNote::NextUp(ep(1, 2)),
                Some(tower.id)
            ),
        ],
        "Dark's next episode is in progress, so it shows once; newest activity first"
    );
    assert!(
        watching(&lib, bo).await.is_empty(),
        "another user's progress never shows"
    );
    let home = lib.home(ada).await.unwrap();
    assert_eq!(
        (home.rows[0].kind, home.rows[0].total),
        (RowKind::ContinueWatching, 2)
    );
    let next_up = &home.rows[0].cards[1];
    assert_eq!(
        (next_up.kind, next_up.watch.resume, next_up.watch.watched()),
        (ItemKind::Episode, None, false),
        "a Next Up entry starts at 0%"
    );

    lib.record_watch(ada, dark.seasons[0].id, WatchEvent::Watched, 4_000)
        .await
        .unwrap();
    let dark_now = show_titled(&lib, "Dark", ada).await;
    assert_eq!(
        (
            dark_now.watched_count,
            dark_now.seasons[0].watched_count,
            dark_now.next.as_ref().map(|n| n.number)
        ),
        (2, 2, Some(ep(2, 1))),
        "marking season 1 watched moves Dark on to S2E1"
    );
    assert_eq!(
        watching(&lib, ada).await[0],
        ("Dark".to_owned(), CardNote::NextUp(ep(2, 1)), Some(dark.id))
    );

    lib.record_watch(ada, dark.id, WatchEvent::Watched, 5_000)
        .await
        .unwrap();
    let dark_done = show_titled(&lib, "Dark", ada).await;
    assert_eq!(
        (dark_done.watched_count, dark_done.next.is_none()),
        (4, true),
        "the whole show, specials included, and nothing comes next"
    );
    assert_eq!(watching(&lib, ada).await.len(), 1, "only Tower's Next Up");

    lib.record_watch(ada, dark.id, WatchEvent::Unwatched, 6_000)
        .await
        .unwrap();
    let dark_reset = show_titled(&lib, "Dark", ada).await;
    assert_eq!(
        (dark_reset.watched_count, dark_reset.next.map(|n| n.number)),
        (0, Some(ep(1, 1)))
    );
    let show_progress = lib
        .record_watch(
            ada,
            dark.id,
            WatchEvent::Progress {
                file_id: file,
                position_ms: 61_000,
                duration_ms: 100_000,
            },
            7_000,
        )
        .await;
    assert!(
        matches!(show_progress, Err(crate::WatchError::FileNotOnItem)),
        "{show_progress:?}"
    );
    let bo_dark = show_titled(&lib, "Dark", bo).await;
    assert_eq!(bo_dark.watched_count, 0, "marks are per user");
}

#[tokio::test(flavor = "multi_thread")]
async fn recently_added_lists_shows_with_their_new_episodes() {
    let dir = tempfile::tempdir().unwrap();
    let (root, data) = (dir.path().join("TV"), dir.path().join("data"));
    tv_library(&root);
    let lib = open(&root, &data, LibraryKind::Shows).await;
    scan(&lib).await;
    let me = user(&lib, "ada").await;
    let home = lib.home(me).await.unwrap();
    let kinds: Vec<RowKind> = home.rows.iter().map(|r| r.kind).collect();
    let library = lib.libraries().await.unwrap()[0].id;
    assert_eq!(kinds, [RowKind::RecentlyAdded, RowKind::Library(library)]);
    let mut recent: Vec<(&str, CardNote)> = home.rows[0]
        .cards
        .iter()
        .map(|c| (c.title.as_str(), c.note))
        .collect();
    recent.sort_by_key(|(title, _)| *title);
    assert_eq!(
        recent,
        [
            ("Dark", CardNote::NewEpisodes(4)),
            ("Tower of God 2 - Ouji no Kikan", CardNote::NewEpisodes(2)),
        ]
    );
    assert_eq!(home.rows[1].total, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_parser_change_rederives_stored_files_without_reading_them() {
    let dir = tempfile::tempdir().unwrap();
    let (root, data) = (dir.path().join("Movies"), dir.path().join("data"));
    clips(
        &root,
        &["BigBuckBunny_512kb.mp4", "Heat (1995)/Heat (1995).mp4"],
    );
    let lib = open(&root, &data, LibraryKind::Movies).await;
    scan(&lib).await;
    let me = user(&lib, "ada").await;
    let library = lib.libraries().await.unwrap()[0].id;
    let titles = |lib: &Library| {
        let lib = lib.clone();
        async move {
            lib.library_items(library, LibraryQuery::default(), None, 10, me)
                .await
                .unwrap()
                .unwrap()
                .cards
                .iter()
                .map(|c| (c.id, c.title.clone()))
                .collect::<Vec<_>>()
        }
    };
    let before = titles(&lib).await;
    let bunny = before[0].0;
    lib.record_watch(me, bunny, WatchEvent::Watched, 1_000)
        .await
        .unwrap();

    // What an older parser left behind: its title and key, and its version.
    lib.inner
        .db
        .write(move |tx| {
            tx.execute(
                "UPDATE item_metadata SET title = 'BigBuckBunny 512kb', sort_title = 'BigBuckBunny 512kb' \
                 WHERE item_id = ?1",
                [bunny.0],
            )?;
            tx.execute(
                "UPDATE item SET identity_key = 'bigbuckbunny512kb|' WHERE id = ?1",
                [bunny.0],
            )?;
            tx.execute("UPDATE media_file SET parse_version = 0", [])?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(titles(&lib).await[0].1, "BigBuckBunny 512kb");

    let rescan = scan(&lib).await;
    assert_eq!(
        (
            rescan.unchanged,
            rescan.changed,
            rescan.added,
            rescan.removed
        ),
        (2, 0, 0, 0),
        "nothing probed again: {rescan:?}"
    );
    assert_eq!(
        titles(&lib).await,
        [
            (bunny, "Big Buck Bunny".to_owned()),
            (before[1].0, "Heat".to_owned())
        ],
        "same ids, titles from the current parser"
    );
    let Some(Item::Movie(movie)) = lib
        .item(bunny, DeviceProfile::baseline(), me)
        .await
        .unwrap()
    else {
        panic!("a movie")
    };
    assert!(movie.watch.watched(), "watch state kept");
    let rev = lib.rev();
    assert_eq!(scan(&lib).await.unchanged, 2);
    assert_eq!(lib.rev(), rev, "re-derived once, then nothing to do");
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_the_type_rebuilds_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let (root, data) = (dir.path().join("Media"), dir.path().join("data"));
    tv_library(&root);
    let lib = open(&root, &data, LibraryKind::Movies).await;
    assert_eq!(scan(&lib).await.added, 7);
    let me = user(&lib, "ada").await;
    let library = lib.libraries().await.unwrap().remove(0);
    assert_eq!(library.item_count, 7, "seven movies, one per file");
    let movies = lib
        .library_items(library.id, LibraryQuery::default(), None, 10, me)
        .await
        .unwrap()
        .unwrap();
    lib.record_watch(me, movies.cards[0].id, WatchEvent::Watched, 1_000)
        .await
        .unwrap();

    let (info, changed) = lib
        .update_library(library.id, None, None, Some(LibraryKind::Shows), 2_000)
        .await
        .unwrap();
    assert!(changed);
    assert_eq!((info.kind, info.item_count), (LibraryKind::Shows, 2));
    let shows = lib
        .library_items(library.id, LibraryQuery::default(), None, 10, me)
        .await
        .unwrap()
        .unwrap();
    let titles: Vec<(&str, ItemKind)> = shows
        .cards
        .iter()
        .map(|c| (c.title.as_str(), c.kind))
        .collect();
    assert_eq!(
        titles,
        [
            ("Dark", ItemKind::Show),
            ("Tower of God 2 - Ouji no Kikan", ItemKind::Show)
        ]
    );
    assert!(
        lib.item(movies.cards[0].id, DeviceProfile::baseline(), me)
            .await
            .unwrap()
            .is_none(),
        "the movies are gone"
    );
    let states: i64 = lib
        .inner
        .db
        .read(|c| c.query_row("SELECT count(*) FROM watch_state", [], |r| r.get(0)))
        .await
        .unwrap();
    assert_eq!(states, 0, "watch progress in the library is reset");

    let summary = scan(&lib).await;
    assert_eq!(
        (
            summary.added,
            summary.changed,
            summary.removed,
            summary.unchanged
        ),
        (0, 0, 0, 6),
        "nothing read again; the file that is no episode is skipped: {summary:?}"
    );
    let dark = show_titled(&lib, "Dark", me).await;
    assert!(
        dark.poster.is_some() && dark.backdrop.is_some(),
        "artwork for the new items"
    );

    let (back, _) = lib
        .update_library(library.id, None, None, Some(LibraryKind::Movies), 3_000)
        .await
        .unwrap();
    assert_eq!(back.item_count, 6, "the episodes are movies again");
    assert_eq!(scan(&lib).await.added, 1, "the skipped file comes back");
}
