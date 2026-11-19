//! Audio fingerprints of a video, to find what episodes share: an intro or
//! end credits sound the same in every episode of a season, the story around
//! them doesn't, and a recap is clips of the episodes before. Chromaprint's
//! algorithm (the one AcoustID uses), ported to Rust, over audio FFmpeg decodes.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use rusty_chromaprint::{Configuration, Fingerprinter, match_fingerprints};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::{Marker, MarkerKind, MediaTools};

/// Bumped when prints change, so stored ones are made again.
pub const AUDIO_PRINT_VERSION: u32 = 1;

/// Decoding 10 minutes of audio takes seconds; reading them from a slow
/// network share can take a few minutes.
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// A fingerprint of a stretch of a file's first audio track.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioPrint {
    /// Where the stretch starts in the file.
    pub start_ms: u64,
    /// One item for each [`AudioPrint::item_ms`] of audio.
    pub items: Vec<u32>,
}

/// A stretch two prints share, in milliseconds from each file's start.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedAudio {
    pub a_start_ms: u64,
    pub b_start_ms: u64,
    pub length_ms: u64,
    /// How different the two are, 0 (the same) to 32; under 10 is a match.
    pub score: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum AudioPrintError {
    #[error("ffmpeg could not run: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("reading the audio took longer than {0:?}")]
    TimedOut(Duration),
}

fn config() -> Configuration {
    Configuration::preset_test2()
}

impl AudioPrint {
    /// The length of audio each item covers: about 124 ms.
    pub fn item_ms() -> f64 {
        f64::from(config().item_duration_in_seconds()) * 1000.0
    }

    pub fn end_ms(&self) -> u64 {
        self.start_ms + (self.items.len() as f64 * Self::item_ms()) as u64
    }

    /// The part of the print between two times in the file.
    pub fn slice(&self, from_ms: u64, to_ms: u64) -> AudioPrint {
        let at = |ms: u64| {
            let i = (ms.saturating_sub(self.start_ms) as f64 / Self::item_ms()) as usize;
            i.min(self.items.len())
        };
        let (from, to) = (at(from_ms), at(to_ms));
        AudioPrint {
            start_ms: self.start_ms + (from as f64 * Self::item_ms()) as u64,
            items: self.items[from..to.max(from)].to_vec(),
        }
    }

    /// The stretches `self` and `other` share at their best alignment.
    pub fn shared(&self, other: &AudioPrint) -> Vec<SharedAudio> {
        let config = config();
        let Ok(segments) = match_fingerprints(&self.items, &other.items, &config) else {
            return Vec::new();
        };
        let ms = |s: f32| (f64::from(s) * 1000.0) as u64;
        segments
            .iter()
            .map(|s| SharedAudio {
                a_start_ms: self.start_ms + ms(s.start1(&config)),
                b_start_ms: other.start_ms + ms(s.start2(&config)),
                length_ms: ms(s.duration(&config)),
                score: s.score,
            })
            .collect()
    }

    /// Items as little-endian bytes, to store.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.items.iter().flat_map(|i| i.to_le_bytes()).collect()
    }

    pub fn from_bytes(start_ms: u64, bytes: &[u8]) -> AudioPrint {
        let items = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        AudioPrint { start_ms, items }
    }
}

impl MediaTools {
    /// The fingerprint of `length` of `input`'s first audio track from `start`.
    pub async fn audio_print(
        &self,
        input: &Path,
        start: Duration,
        length: Duration,
    ) -> Result<AudioPrint, AudioPrintError> {
        let config = config();
        let rate = config.sample_rate();
        let mut child = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-ss"])
            .arg(format!("{:.3}", start.as_secs_f64()))
            .arg("-i")
            .arg(input)
            .arg("-t")
            .arg(format!("{:.3}", length.as_secs_f64()))
            .args(["-map", "0:a:0", "-vn", "-sn", "-dn", "-ac", "1", "-ar"])
            .arg(rate.to_string())
            .args(["-f", "s16le", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let mut stderr = child.stderr.take().expect("stderr is piped");
        // Read at once, or a file FFmpeg complains about at every packet fills the pipe and stalls it.
        let errors = tokio::spawn(async move {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text).await;
            text
        });
        // The fingerprinter can't move between threads, so it stays on one and samples go to it.
        let (samples_tx, mut samples_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(8);
        let printer = tokio::task::spawn_blocking(move || {
            let mut printer = Fingerprinter::new(&config);
            printer
                .start(rate, 1)
                .expect("11025 Hz mono is a rate Chromaprint takes");
            while let Some(samples) = samples_rx.blocking_recv() {
                printer.consume(&samples);
            }
            printer.finish();
            printer.fingerprint().to_vec()
        });
        let read = async {
            let mut buf = vec![0u8; 64 * 1024];
            let mut carry: Option<u8> = None;
            loop {
                let n = stdout.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                let mut samples = Vec::with_capacity(n / 2 + 1);
                let mut bytes = &buf[..n];
                if let Some(low) = carry.take() {
                    samples.push(i16::from_le_bytes([low, bytes[0]]));
                    bytes = &bytes[1..];
                }
                let (pairs, rest) = bytes.as_chunks::<2>();
                carry = rest.first().copied();
                samples.extend(pairs.iter().map(|p| i16::from_le_bytes(*p)));
                if samples_tx.send(samples).await.is_err() {
                    break;
                }
            }
            drop(samples_tx);
            let status = child.wait().await?;
            let errors = errors.await.unwrap_or_default();
            if !status.success() {
                return Err(AudioPrintError::Ffmpeg(errors.trim().to_owned()));
            }
            Ok(printer.await.expect("fingerprinting doesn't panic"))
        };
        let items = tokio::time::timeout(TIMEOUT, read)
            .await
            .map_err(|_| AudioPrintError::TimedOut(TIMEOUT))??;
        Ok(AudioPrint {
            start_ms: start.as_millis() as u64,
            items,
        })
    }
}

/// Intros are looked for in an episode's first quarter, at most this much.
const HEAD_MS: u64 = 10 * 60_000;
/// Credits are looked for in an episode's last five minutes.
const TAIL_MS: u64 = 5 * 60_000;
/// A recap starts within this of the episode's start...
const RECAP_STARTS_MS: u64 = 30_000;
/// ...is looked for in its first four minutes...
const RECAP_SPAN_MS: u64 = 4 * 60_000;
/// ...and ends where clips of earlier episodes stop coming within this. At
/// least two clips and this much of them, or it's a sting every episode opens with.
const RECAP_GAP_MS: u64 = 10_000;
const RECAP_MIN_MS: u64 = 8_000;
const RECAP_CLIPS_MS: u64 = 6_000;
/// A recap cuts every second or two, so a clip counts from about 1.2 s.
/// Runs are found where `SMOOTH` items in a row differ by `RUN_BITS` or less
/// on average, and count where the whole run differs by `CLIP_BITS` or less:
/// a clip played as it was differs by 4 to 6.5, dialogue that merely sounds
/// alike by 7 and up. A clip with music laid under it differs as much as
/// that, so recaps made that way aren't found.
const CLIP_ITEMS: usize = 10;
const SMOOTH: usize = 8;
const RUN_BITS: u32 = 10;
const CLIP_BITS: f64 = 6.5;
/// A sound this many earlier episodes have is the show's music or a sting,
/// not a clip of one of them.
const RECURRING: usize = 3;
/// Alignments with fewer items agreeing on their top 12 bits aren't checked.
const CLIP_VOTES: u32 = 5;
/// Shared stretches shorter than these are coincidence, longer intros are not intros.
const INTRO_MIN_MS: u64 = 5_000;
const INTRO_MAX_MS: u64 = 150_000;
const CREDITS_MIN_MS: u64 = 15_000;
/// Credits end at most this long before the file does (silence, a logo).
const CREDITS_END_SLACK_MS: u64 = 20_000;
/// Average bits (of 32) that differ: matching audio is near 2, unrelated near 16.
const SHARED_SCORE: f64 = 7.0;
/// Episodes that agree on where an intro or credits are, at most this apart.
const AGREE_MS: u64 = 3_000;

/// Where `episode` (a whole-file print) has a recap, an intro and end
/// credits, found by comparing it with other episodes of its season
/// (`season`, nearest first) and with episodes before it in the show
/// (`earlier`, latest first). An intro or credits count only where two of
/// the season's episodes agree (one, in a season of two), since a false end
/// of credits starts the next episode early.
pub fn find_markers(
    episode: &AudioPrint,
    duration_ms: u64,
    season: &[&AudioPrint],
    earlier: &[&AudioPrint],
) -> Vec<Marker> {
    let need = season.len().min(2);
    let head = |p: &AudioPrint| p.slice(0, (p.end_ms() / 4).min(HEAD_MS));
    let tail = |p: &AudioPrint| p.slice(p.end_ms().saturating_sub(TAIL_MS), p.end_ms());
    let mut markers = Vec::new();

    let mine = head(episode);
    let found = season.iter().map(|o| stretches(episode, &mine, &head(o)));
    let intro = agreed(found, need, |(s, e)| {
        (INTRO_MIN_MS..=INTRO_MAX_MS).contains(&(e - s))
    });
    // Every earlier episode has the intro too: the recap ends before it.
    let before = intro.map_or(u64::MAX, |(start, _)| start);
    if let Some(end) = find_recap(episode, earlier, before) {
        markers.push(Marker {
            kind: MarkerKind::Recap,
            start_ms: 0,
            end_ms: end,
        });
    }
    if let Some((start, end)) = intro {
        markers.push(Marker {
            kind: MarkerKind::Intro,
            start_ms: start,
            end_ms: end,
        });
    }

    let mine = tail(episode);
    let found = season.iter().map(|o| stretches(episode, &mine, &tail(o)));
    if let Some((start, _)) = agreed(found, need, |(s, e)| {
        e - s >= CREDITS_MIN_MS && e + CREDITS_END_SLACK_MS >= duration_ms
    }) {
        markers.push(Marker {
            kind: MarkerKind::Credits,
            start_ms: start,
            end_ms: duration_ms,
        });
    }
    markers
}

/// The stretches of `part` (of `episode`) that `other` has too, joined where
/// a match only dips briefly, as (start, end) in `episode`.
fn stretches(episode: &AudioPrint, part: &AudioPrint, other: &AudioPrint) -> Vec<(u64, u64)> {
    let mut joined: Vec<(u64, u64, f64)> = Vec::new();
    for m in part.shared(other) {
        let (start, end) = (m.a_start_ms, m.a_start_ms + m.length_ms);
        match joined.last_mut() {
            Some(last) if start <= last.1 + 1_500 => {
                let (a, b) = ((last.1 - last.0) as f64, (end - start) as f64);
                last.2 = (last.2 * a + m.score * b) / (a + b);
                last.1 = end;
            }
            _ => joined.push((start, end, m.score)),
        }
    }
    joined
        .into_iter()
        .filter(|&(s, e, score)| score <= SHARED_SCORE && varied(&episode.slice(s, e)))
        .map(|(s, e, _)| (s, e))
        .collect()
}

/// The stretch at least `need` episodes found (each giving its stretches),
/// agreeing on where it starts: the earliest start and latest end among them.
fn agreed(
    found: impl Iterator<Item = Vec<(u64, u64)>>,
    need: usize,
    fits: impl Fn((u64, u64)) -> bool,
) -> Option<(u64, u64)> {
    if need == 0 {
        return None;
    }
    let mut all: Vec<(u64, u64, usize)> = found
        .enumerate()
        .flat_map(|(n, list)| {
            list.into_iter()
                .filter(|&s| fits(s))
                .map(move |(s, e)| (s, e, n))
        })
        .collect();
    all.sort();
    let mut best: Option<(usize, (u64, u64))> = None;
    for (i, &(start, _, _)) in all.iter().enumerate() {
        let group: Vec<_> = all[i..]
            .iter()
            .take_while(|g| g.0 <= start + AGREE_MS)
            .collect();
        let episodes: HashSet<usize> = group.iter().map(|g| g.2).collect();
        let end = group.iter().map(|g| g.1).max().unwrap_or(start);
        if episodes.len() >= need && best.is_none_or(|(n, _)| episodes.len() > n) {
            best = Some((episodes.len(), (start, end)));
        }
    }
    best.map(|(_, s)| s)
}

/// Where the recap ends: a run of clips of earlier episodes from near the
/// start (before `before_ms`). Each earlier episode votes for alignments
/// where its items share their top bits with the opening's, as Chromaprint
/// aligns prints; the likely ones are then compared item by item.
fn find_recap(episode: &AudioPrint, earlier: &[&AudioPrint], before_ms: u64) -> Option<u64> {
    let span = RECAP_SPAN_MS.min(episode.end_ms() / 4).min(before_ms);
    let opening = episode.slice(0, span).items;
    if earlier.is_empty() || opening.len() < CLIP_ITEMS {
        return None;
    }
    let key = |item: u32| item >> 20;
    let mut index: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, &item) in opening.iter().enumerate() {
        index.entry(key(item)).or_default().push(i);
    }
    // A key all over the opening is silence or a held note; it aligns everything.
    index.retain(|_, at| at.len() <= 16);
    // Each clip: where it is in the opening, and which earlier episode and
    // alignment it came from.
    let mut clips: Vec<(usize, usize, (usize, isize))> = Vec::new();
    for (n, print) in earlier.iter().enumerate() {
        let mut votes: HashMap<isize, u32> = HashMap::new();
        for (j, &item) in print.items.iter().enumerate() {
            for &i in index.get(&key(item)).into_iter().flatten() {
                *votes.entry(j as isize - i as isize).or_default() += 1;
            }
        }
        for (&shift, _) in votes.iter().filter(|&(_, &v)| v >= CLIP_VOTES) {
            for (from, to) in clip_runs(&opening, &print.items, shift) {
                clips.push((from, to, (n, shift)));
            }
        }
    }
    let recurring = |&(from, to, _): &(usize, usize, (usize, isize))| {
        let episodes: HashSet<usize> = clips
            .iter()
            .filter(|c| c.0.max(from) < c.1.min(to))
            .map(|c| c.2.0)
            .collect();
        episodes.len() >= RECURRING
    };
    let mut clips: Vec<_> = clips.iter().filter(|c| !recurring(c)).copied().collect();
    clips.sort();
    let item_ms = AudioPrint::item_ms();
    let ms = |i: usize| (i as f64 * item_ms) as u64;
    let first = clips.first().filter(|c| ms(c.0) <= RECAP_STARTS_MS)?;
    let (mut end, mut covered) = (first.1, first.1 - first.0);
    let mut sources = HashSet::from([first.2]);
    for &(from, to, source) in &clips[1..] {
        if ms(from) > ms(end) + RECAP_GAP_MS {
            break;
        }
        covered += to.saturating_sub(from.max(end));
        end = end.max(to);
        sources.insert(source);
    }
    let fits = sources.len() >= 2 && ms(covered) >= RECAP_CLIPS_MS && ms(end) >= RECAP_MIN_MS;
    fits.then_some(ms(end))
}

/// The runs of `opening` that match `print` shifted by `shift` items, as
/// item ranges: at least [`CLIP_ITEMS`] long, close enough, and not silence.
fn clip_runs(opening: &[u32], print: &[u32], shift: isize) -> Vec<(usize, usize)> {
    let first = (-shift).max(0) as usize;
    let last = (print.len() as isize - shift).clamp(0, opening.len() as isize) as usize;
    if last < first + SMOOTH {
        return Vec::new();
    }
    let bits: Vec<u32> = (first..last)
        .map(|i| (opening[i] ^ print[(i as isize + shift) as usize]).count_ones())
        .collect();
    let mut runs = Vec::new();
    let mut keep = |mut from: usize, mut to: usize| {
        // A run's windows reach a few items into the audio around the clip.
        while from < to && bits[from] > RUN_BITS {
            from += 1;
        }
        while to > from && bits[to - 1] > RUN_BITS {
            to -= 1;
        }
        if to - from < CLIP_ITEMS {
            return;
        }
        let mean = f64::from(bits[from..to].iter().sum::<u32>()) / (to - from) as f64;
        let items = AudioPrint {
            start_ms: 0,
            items: opening[first + from..first + to].to_vec(),
        };
        if mean <= CLIP_BITS && varied(&items) {
            runs.push((first + from, first + to));
        }
    };
    let mut window: u32 = bits[..SMOOTH].iter().sum();
    let mut run: Option<usize> = None;
    for w in 0..=bits.len() - SMOOTH {
        if w > 0 {
            window = window - bits[w - 1] + bits[w + SMOOTH - 1];
        }
        match (window <= RUN_BITS * SMOOTH as u32, run) {
            (true, None) => run = Some(w),
            (false, Some(start)) => {
                keep(start, w - 1 + SMOOTH);
                run = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run {
        keep(start, bits.len());
    }
    runs
}

/// Not silence or a held tone, which every episode shares.
fn varied(print: &AudioPrint) -> bool {
    let distinct: HashSet<u32> = print.items.iter().copied().collect();
    !print.items.is_empty() && distinct.len() * 2 >= print.items.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Items no other call makes, like audio nothing else has.
    fn noise(seed: u64, ms: u64) -> Vec<u32> {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..items(ms))
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u32
            })
            .collect()
    }

    fn items(ms: u64) -> usize {
        (ms as f64 / AudioPrint::item_ms()) as usize
    }

    fn ms(items: usize) -> u64 {
        (items as f64 * AudioPrint::item_ms()) as u64
    }

    /// A 44-minute episode: `recap` (clips), a cold open, the intro, the
    /// story, then the credits.
    fn episode(
        seed: u64,
        recap: &[Vec<u32>],
        cold_open_ms: u64,
        intro: &[u32],
        credits: &[u32],
    ) -> AudioPrint {
        let mut items_ = Vec::new();
        for clip in recap {
            items_.extend_from_slice(clip);
        }
        items_.extend(noise(seed, cold_open_ms));
        items_.extend_from_slice(intro);
        let story = items(44 * 60_000) - items_.len() - credits.len() - items(3_000);
        items_.extend(noise(seed + 1000, ms(story)));
        items_.extend_from_slice(credits);
        items_.extend(vec![0; items(3_000)]);
        AudioPrint {
            start_ms: 0,
            items: items_,
        }
    }

    fn near(a: u64, b: u64) -> bool {
        a.abs_diff(b) <= 1_000
    }

    #[test]
    fn episodes_agree_on_their_intro_and_credits() {
        let intro = noise(1, 8_500);
        let credits = noise(2, 37_000);
        let eps: Vec<AudioPrint> = (0..4)
            .map(|n| episode(10 + n, &[], 300_000 + n * 40_000, &intro, &credits))
            .collect();
        let others: Vec<&AudioPrint> = eps[1..].iter().collect();
        let length = eps[0].end_ms();
        let found = find_markers(&eps[0], length, &others, &[]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].kind, MarkerKind::Intro);
        assert!(
            near(found[0].start_ms, 300_000) && near(found[0].end_ms, 308_500),
            "{found:?}"
        );
        assert_eq!(found[1].kind, MarkerKind::Credits);
        assert!(near(found[1].start_ms, length - 40_000), "{found:?}");
        assert_eq!(found[1].end_ms, length);
    }

    #[test]
    fn one_other_episode_is_not_enough_in_a_longer_season() {
        let intro = noise(1, 8_500);
        let credits = noise(2, 37_000);
        let a = episode(10, &[], 300_000, &intro, &credits);
        let b = episode(11, &[], 200_000, &intro, &credits);
        let c = episode(12, &[], 250_000, &noise(3, 8_500), &noise(4, 37_000));
        assert!(find_markers(&a, a.end_ms(), &[&b, &c], &[]).is_empty());
        // In a season of two, the other one is enough.
        assert_eq!(find_markers(&a, a.end_ms(), &[&b], &[]).len(), 2);
    }

    #[test]
    fn a_recap_is_clips_of_earlier_episodes_from_the_start() {
        let intro = noise(1, 8_500);
        let credits = noise(2, 37_000);
        let earlier: Vec<AudioPrint> = (0..2)
            .map(|n| episode(20 + n, &[], 200_000, &intro, &credits))
            .collect();
        let clip = |e: &AudioPrint, at_ms: u64, len_ms: u64| e.slice(at_ms, at_ms + len_ms).items;
        // "Previously on…" (in no episode), then three clips back to back.
        let recap = vec![
            noise(5, 3_000),
            clip(&earlier[0], 1_000_000, 8_000),
            clip(&earlier[1], 1_500_000, 7_000),
            clip(&earlier[1], 2_200_000, 9_000),
        ];
        let ep = episode(30, &recap, 120_000, &intro, &credits);
        let earlier_refs: Vec<&AudioPrint> = earlier.iter().collect();
        let found = find_markers(&ep, ep.end_ms(), &earlier_refs, &earlier_refs);
        assert_eq!(found[0].kind, MarkerKind::Recap, "{found:?}");
        assert_eq!(found[0].start_ms, 0);
        assert!(found[0].end_ms.abs_diff(27_000) <= 3_000, "{found:?}");
        assert_eq!(found[1].kind, MarkerKind::Intro);
        assert!(near(found[1].start_ms, 27_000 + 120_000), "{found:?}");
    }

    #[test]
    fn a_recap_that_cuts_every_second_and_a_half_is_found() {
        let intro = noise(1, 8_500);
        let credits = noise(2, 37_000);
        let earlier: Vec<AudioPrint> = (0..6)
            .map(|n| episode(40 + n, &[], 200_000, &intro, &credits))
            .collect();
        // Twelve 1.5 s clips, each item 3 bits off: music and narration over them.
        let mut recap = vec![noise(5, 3_000)];
        for k in 0..12u64 {
            let source = &earlier[(k % 6) as usize];
            let at = 400_000 + k * 97_000;
            let mut clip = source.slice(at, at + 1_500).items;
            for (n, item) in clip.iter_mut().enumerate() {
                *item ^= (0b111 << (n % 29)) as u32;
            }
            recap.push(clip);
        }
        let ep = episode(50, &recap, 120_000, &intro, &credits);
        let refs: Vec<&AudioPrint> = earlier.iter().collect();
        let found = find_markers(&ep, ep.end_ms(), &refs, &refs);
        assert_eq!(found[0].kind, MarkerKind::Recap, "{found:?}");
        assert!(found[0].end_ms.abs_diff(21_000) <= 1_500, "{found:?}");
    }

    #[test]
    fn shared_silence_is_not_an_intro() {
        let credits = noise(2, 37_000);
        let silence = vec![0u32; items(20_000)];
        let eps: Vec<AudioPrint> = (0..3)
            .map(|n| episode(10 + n, &[], 300_000, &silence, &credits))
            .collect();
        let found = find_markers(&eps[0], eps[0].end_ms(), &[&eps[1], &eps[2]], &[]);
        assert!(
            found.iter().all(|m| m.kind != MarkerKind::Intro),
            "{found:?}"
        );
    }

    #[test]
    fn prints_round_trip_as_bytes() {
        let print = AudioPrint {
            start_ms: 0,
            items: noise(9, 5_000),
        };
        assert_eq!(AudioPrint::from_bytes(0, &print.to_bytes()), print);
    }
}
