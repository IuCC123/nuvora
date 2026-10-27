"use client";

import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { EASE } from "./Reveal";
import { Mark } from "./Nav";
import { TitleCard, type PaletteName } from "./TitleCard";
import {
  ArrowLeftIcon,
  Back10Icon,
  CheckIcon,
  ChevronDownIcon,
  FilmIcon,
  Forward10Icon,
  FullscreenIcon,
  HouseIcon,
  PauseIcon,
  PipIcon,
  SearchIcon,
  SlidersIcon,
  SubtitlesIcon,
  TvIcon,
  UsersIcon,
  VolumeIcon,
} from "./icons";

/*
 * Stills, posters and subtitles come from the Blender Foundation's open movies (CC BY):
 * frames are taken from Sintel.2010.1080p.mkv and Tears of Steel, posters from Wikimedia Commons.
 */

export const media = (name: string) => `/media/${name}.webp`;

export const SCENE_MEDIA = [
  "poster-big-buck-bunny",
  "poster-cosmos-laundromat",
  "poster-sintel",
  "poster-tears-of-steel",
  "sintel-131",
  "sintel-140",
  "sintel-144",
  "sintel-200",
  "sintel-290",
  "sintel-320",
  "thumb-big-buck-bunny",
  "thumb-cosmos-laundromat",
  "thumb-elephants-dream",
  "thumb-sintel-scales",
  "thumb-tears-of-steel-balcony",
].map(media);

/** Seconds since the scene mounted, re-rendering a few times a second to drive its script. */
function useSeconds(fps = 12) {
  const [t, setT] = useState(0);
  useEffect(() => {
    const start = performance.now();
    const id = setInterval(() => setT((performance.now() - start) / 1000), 1000 / fps);
    return () => clearInterval(id);
  }, [fps]);
  return t;
}

const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

const SPINNER = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

function Spinner() {
  const t = useSeconds(12);
  return <span className="spinner">{SPINNER[Math.floor(t * 12) % SPINNER.length]}</span>;
}

/** A film still, cross-fading when the frame changes, letterboxed like the player does. */
function Picture({ frame, className = "pl-picture" }: { frame: string; className?: string }) {
  return (
    <div className={className}>
      <AnimatePresence initial={false}>
        <motion.img
          key={frame}
          src={media(frame)}
          alt=""
          draggable={false}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.5 }}
        />
      </AnimatePresence>
    </div>
  );
}

/* Player chrome ------------------------------------------------------------ */

const SINTEL_LENGTH = 888;

function PlayerTop() {
  return (
    <div className="pl-top">
      <ArrowLeftIcon />
      <span className="pl-title">Sintel</span>
      <span className="pl-year">2010</span>
    </div>
  );
}

function PlayerBottom({ time, cc = false, preview }: { time: number; cc?: boolean; preview?: React.ReactNode }) {
  const pct = (time / SINTEL_LENGTH) * 100;
  return (
    <div className="pl-bottom">
      <div className="pl-timeline">
        <span className="pl-time">{clock(time)}</span>
        <div className="pl-scrub">
          <span className="pl-buffer" style={{ left: `${pct}%` }} />
          <span className="pl-played" style={{ width: `${pct}%` }} />
          <span className="pl-knob" style={{ left: `${pct}%` }} />
          {preview}
        </div>
        <span className="pl-time">-{clock(SINTEL_LENGTH - time)}</span>
      </div>
      <div className="pl-controls">
        <div className="pl-cluster">
          <VolumeIcon />
          <span className="pl-volume">
            <span />
          </span>
        </div>
        <div className="pl-cluster pl-center">
          <Back10Icon />
          <span className="pl-play">
            <PauseIcon />
          </span>
          <Forward10Icon />
        </div>
        <div className="pl-cluster pl-right">
          <span className="pl-cc" data-on={cc}>
            <SubtitlesIcon on={cc} />
          </span>
          <PipIcon />
          <FullscreenIcon />
        </div>
      </div>
    </div>
  );
}

/* Posters, as the app's poster cards draw them ------------------------------ */

type Shelf = { title: string; art?: string; palette: PaletteName; year?: number; progress?: number; watched?: boolean; sub?: string };

function PosterCard({ item, delay = 0, artVisible = true }: { item: Shelf; delay?: number; artVisible?: boolean }) {
  return (
    <motion.div
      className="ui-card"
      initial={{ opacity: 0, y: 10, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.5, delay, ease: EASE }}
    >
      <div className="ui-poster">
        <TitleCard title={item.title} year={item.year} palette={item.palette} />
        {item.art && (
          <motion.img
            src={media(item.art)}
            alt=""
            draggable={false}
            initial={{ opacity: artVisible ? 1 : 0 }}
            animate={{ opacity: artVisible ? 1 : 0 }}
            transition={{ duration: 0.6 }}
          />
        )}
        {item.progress !== undefined && (
          <span className="ui-progress">
            <span style={{ width: `${item.progress * 100}%` }} />
          </span>
        )}
        {item.watched && (
          <span className="ui-watched">
            <CheckIcon />
          </span>
        )}
      </div>
      <span className="ui-cap-title">{item.title}</span>
      <span className="ui-cap-year">{item.sub ?? item.year}</span>
    </motion.div>
  );
}

function AppTop({ children }: { children?: React.ReactNode }) {
  return (
    <div className="app-top">
      <span className="app-brand">
        <Mark size={10} />
        nuvora
      </span>
      <span className="app-search">
        <SearchIcon />
        Search
      </span>
      {children}
    </div>
  );
}

function AppRail({ active }: { active: "home" | "movies" }) {
  return (
    <nav className="app-rail" aria-hidden>
      <span data-active={active === "home"}>
        <HouseIcon />
      </span>
      <span data-active={active === "movies"}>
        <FilmIcon />
      </span>
      <span>
        <TvIcon />
      </span>
    </nav>
  );
}

/* 1. Library --------------------------------------------------------------- */

type Movie = Shelf & { file: string; size: string; thumb?: string };

const MOVIES: Movie[] = [
  { file: "Big Buck Bunny (2008).mkv", size: "887 MB", thumb: "thumb-big-buck-bunny", title: "Big Buck Bunny", year: 2008, art: "poster-big-buck-bunny", palette: "moss" },
  { file: "Cosmos Laundromat (2015).mkv", size: "1.4 GB", thumb: "thumb-cosmos-laundromat", title: "Cosmos Laundromat", year: 2015, art: "poster-cosmos-laundromat", palette: "violet" },
  { file: "Elephants Dream (2006).mp4", size: "703 MB", thumb: "thumb-elephants-dream", title: "Elephants Dream", year: 2006, palette: "slate" },
  { file: "Sintel (2010).mkv", size: "1.1 GB", thumb: "thumb-sintel-scales", title: "Sintel", year: 2010, art: "poster-sintel", palette: "ember" },
  { file: "Spring (2019).mkv", size: "1.2 GB", title: "Spring", year: 2019, palette: "jade" },
  { file: "Tears of Steel (2012).mkv", size: "2.6 GB", thumb: "thumb-tears-of-steel-balcony", title: "Tears of Steel", year: 2012, art: "poster-tears-of-steel", palette: "ocean" },
];

const READ_AT = 1.1;
const READ_STEP = 0.42;
const ART_AT = READ_AT + MOVIES.length * READ_STEP + 0.25;
const DONE_AT = ART_AT + 1.5;

export function LibraryScene() {
  const t = useSeconds();
  const read = Math.max(0, Math.min(MOVIES.length, Math.floor((t - READ_AT) / READ_STEP) + 1));
  const reading = t >= READ_AT && t < ART_AT ? read - 1 : -1;
  const phase = t < READ_AT ? "Finding files" : t < ART_AT ? "Reading files" : t < DONE_AT ? "Making artwork" : null;
  const withArt = MOVIES.filter((m) => m.art);

  return (
    <div className="scene lib">
      <motion.div className="finder" initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.7, ease: EASE }}>
        <div className="win-bar">
          <i />
          <i />
          <i />
          <span>Movies</span>
        </div>
        <div className="finder-head">
          <span>Name</span>
          <span>Size</span>
        </div>
        <div className="finder-list">
          {MOVIES.map((m, i) => (
            <div key={m.file} className="finder-row" data-reading={i === reading} data-read={i < read && i !== reading}>
              <span className="finder-icon">{m.thumb ? <img src={media(m.thumb)} alt="" draggable={false} /> : <FilmIcon />}</span>
              <span className="finder-name">{m.file}</span>
              <span className="finder-size">{m.size}</span>
            </div>
          ))}
        </div>
        <div className="finder-foot">6 items · ~/Media/Movies</div>
      </motion.div>

      <motion.div
        className="app"
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.7, delay: 0.12, ease: EASE }}
      >
        <AppTop>
          <AnimatePresence>
            {phase && (
              <motion.span
                className="scan-pill"
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                transition={{ duration: 0.3 }}
              >
                <Spinner />
                {phase}
                {phase === "Reading files" && (
                  <>
                    <span className="scan-count">
                      {read} of {MOVIES.length}
                    </span>
                    <span className="scan-bar">
                      <span style={{ width: `${(read / MOVIES.length) * 100}%` }} />
                    </span>
                  </>
                )}
              </motion.span>
            )}
          </AnimatePresence>
          <span className="avatar" data-who="You">
            Y
          </span>
        </AppTop>
        <div className="app-body">
          <AppRail active="movies" />
          <div className="app-content">
            <div className="lib-head">
              <FilmIcon />
              <span className="lib-title">Movies</span>
              <span className="lib-count">{read > 0 ? `${read} movie${read === 1 ? "" : "s"}` : ""}</span>
              <span className="lib-sort">
                Title <ChevronDownIcon />
              </span>
            </div>
            <div className="lib-grid">
              {MOVIES.map((m, i) =>
                i < read ? (
                  <PosterCard key={m.file} item={m} artVisible={!!m.art && t >= ART_AT + withArt.indexOf(m) * 0.3} />
                ) : (
                  <div key={m.file} className="ui-card ui-card-empty" />
                ),
              )}
            </div>
          </div>
        </div>
      </motion.div>
    </div>
  );
}

/* 2. Playback -------------------------------------------------------------- */

const SPOT_A = { at: 320, frame: "sintel-320" };
const SPOT_B = { at: 200, frame: "sintel-200" };

/** Ten-second loop: play, hover a spot on the timeline, jump there, play, jump back. */
function playback(c: number) {
  if (c < 3) return { time: SPOT_A.at + c, frame: SPOT_A.frame, preview: null };
  if (c < 4.4) return { time: SPOT_A.at + c, frame: SPOT_A.frame, preview: SPOT_B };
  if (c < 7.6) return { time: SPOT_B.at + (c - 4.4), frame: SPOT_B.frame, preview: null };
  if (c < 9) return { time: SPOT_B.at + (c - 4.4), frame: SPOT_B.frame, preview: SPOT_A };
  return { time: SPOT_A.at + (c - 9), frame: SPOT_A.frame, preview: null };
}

const STREAMS = [
  { kind: "Video", source: "H.264 · 1080p", result: "Plays as is", converting: false },
  { kind: "Audio", source: "AC3 5.1", result: "AAC 5.1", converting: true },
  { kind: "Subtitles", source: "English · SRT", result: "Plays as is", converting: false },
];

export function PlaybackScene() {
  const t = useSeconds();
  const { time, frame, preview } = playback(t % 10);
  const head = (time / SINTEL_LENGTH) * 100;

  return (
    <div className="scene player">
      <Picture frame={frame} />
      <PlayerTop />

      <motion.div
        className="pl-info"
        initial={{ opacity: 0, x: 16 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ duration: 0.7, delay: 0.3, ease: EASE }}
      >
        <div className="pl-info-file">
          <span className="mono">Sintel.2010.1080p.mkv</span>
          <span>1.1 GB</span>
        </div>
        {STREAMS.map((s, i) => (
          <motion.div
            key={s.kind}
            className="pl-info-row"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: 0.55 + i * 0.12, ease: EASE }}
          >
            <span className="pl-info-kind">{s.kind}</span>
            <span className="pl-info-src">{s.source}</span>
            {s.converting ? (
              <span className="pl-info-out converting">
                <span className="eq" aria-hidden>
                  <i />
                  <i />
                  <i />
                </span>
                {s.result}
              </span>
            ) : (
              <span className="pl-info-out">
                <CheckIcon />
                {s.result}
              </span>
            )}
          </motion.div>
        ))}
        <p className="pl-info-note">This browser can't play AC3, so only the audio is converted.</p>
      </motion.div>

      <PlayerBottom
        time={time}
        cc
        preview={
          <AnimatePresence>
            {preview && (
              <motion.span
                key={preview.frame}
                className="pl-preview"
                initial={{ left: `${head}%`, x: "-50%", opacity: 0, y: 6 }}
                animate={{ left: `${(preview.at / SINTEL_LENGTH) * 100}%`, x: "-50%", opacity: 1, y: 0 }}
                exit={{ x: "-50%", opacity: 0, y: 4 }}
                transition={{ duration: 0.9, ease: EASE }}
              >
                <img src={media(preview.frame)} alt="" draggable={false} />
                <span>{clock(preview.at)}</span>
              </motion.span>
            )}
          </AnimatePresence>
        }
      />
    </div>
  );
}

/* 3. Subtitles ------------------------------------------------------------- */

/** Sintel's own cues, in English and then in Japanese from a .ja.srt beside the file. */
const CUES = [
  { frame: "sintel-140", time: 130, lang: "en", lines: ["What brings you to", "the land of the gatekeepers?"] },
  { frame: "sintel-131", time: 136, lang: "en", lines: ["I'm searching for someone."] },
  { frame: "sintel-140", time: 139, lang: "ja", lines: ["大切な人？", "親類かい？"] },
  { frame: "sintel-144", time: 144, lang: "ja", lines: ["ドラゴンを"] },
];

const TRACKS = [
  { id: "off", name: "Off" },
  { id: "en", name: "English", note: "Embedded" },
  { id: "de", name: "Deutsch", note: "Embedded" },
  { id: "es", name: "Español", note: "Embedded" },
  { id: "fr", name: "Français", note: "Embedded" },
  { id: "ja", name: "日本語", note: "Sintel (2010).ja.srt" },
];

const CUE_LENGTH = 2.8;

export function SubtitleScene() {
  const t = useSeconds();
  const step = Math.floor(t / CUE_LENGTH) % CUES.length;
  const cue = CUES[step]!;
  const into = (t % CUE_LENGTH) / CUE_LENGTH;
  // Before the cue that changes language, the pointer moves to the next track and clicks it.
  const next = step === 1 ? "ja" : step === 3 ? "en" : null;
  const pointing = next && into > 0.5 ? next : null;
  const pointerRow = TRACKS.findIndex((tr) => tr.id === (pointing ?? cue.lang));

  return (
    <div className="scene player">
      <Picture frame={cue.frame} />
      <PlayerTop />
      <div className="st-cues" lang={cue.lang}>
        <AnimatePresence mode="wait">
          <motion.p key={step} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }}>
            {cue.lines.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </motion.p>
        </AnimatePresence>
      </div>

      <motion.div
        className="st-menu"
        initial={{ opacity: 0, y: 12, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.6, delay: 0.3, ease: EASE }}
      >
        <div className="st-heading">Subtitles</div>
        <div className="st-tracks">
          {TRACKS.map((tr) => (
            <div key={tr.id} className="st-track" data-on={tr.id === cue.lang}>
              {tr.id === cue.lang && <motion.span layoutId="st-selected" className="st-selected" transition={{ duration: 0.35, ease: EASE }} />}
              <span className="st-radio">{tr.id === cue.lang && <CheckIcon />}</span>
              <span className="st-name" lang={tr.id === "ja" ? "ja" : undefined}>
                {tr.name}
              </span>
              {tr.note && <span className="st-note">{tr.note}</span>}
            </div>
          ))}
          <motion.span
            className="st-pointer"
            initial={false}
            animate={{ top: `${pointerRow * 2.2 + 1.2}em`, opacity: pointing ? 1 : 0, scale: pointing && into > 0.85 ? 0.82 : 1 }}
            transition={{ duration: 0.45, ease: EASE }}
            aria-hidden
          >
            <svg viewBox="0 0 16 22">
              <path d="M1 1v17l4.5-4.2 3 6.7 2.8-1.3-3-6.5H14z" fill="#fff" stroke="#000" strokeWidth="1.2" strokeLinejoin="round" />
            </svg>
          </motion.span>
        </div>
        <div className="st-style">
          <span className="st-label">Size</span>
          <span className="st-seg">
            <span>Small</span>
            <span data-on="true">Medium</span>
            <span>Large</span>
          </span>
          <span className="st-label">Background</span>
          <span className="st-seg">
            <span>None</span>
            <span data-on="true">Shadow</span>
            <span>Box</span>
          </span>
        </div>
      </motion.div>

      <PlayerBottom time={cue.time + (t % CUE_LENGTH)} cc />
    </div>
  );
}

/* 4. Watch Together -------------------------------------------------------- */

type SyncEvent = "play" | "pause" | "seek";

/** Ten seconds: play, Sam pauses, play, you skip ahead. */
function together(c: number): { time: number; frame: string; event: SyncEvent } {
  if (c < 3) return { time: 200 + c, frame: "sintel-200", event: "play" };
  if (c < 4.6) return { time: 203, frame: "sintel-200", event: "pause" };
  if (c < 6.5) return { time: 203 + (c - 4.6), frame: "sintel-200", event: "play" };
  return { time: 290 + (c - 6.5), frame: "sintel-290", event: c < 8.6 ? "seek" : "play" };
}

function Toast({ event }: { event: SyncEvent }) {
  const text = event === "pause" ? "Sam paused" : event === "seek" ? "You skipped to 4:50" : null;
  return (
    <AnimatePresence>
      {text && (
        <motion.span
          key={text}
          className="wt-toast"
          initial={{ opacity: 0, y: -6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.35, ease: EASE }}
        >
          {text}
        </motion.span>
      )}
    </AnimatePresence>
  );
}

function PausedGlyph({ show }: { show: boolean }) {
  return (
    <AnimatePresence>
      {show && (
        <motion.span
          className="wt-paused"
          initial={{ opacity: 0, scale: 0.7 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 1.25 }}
          transition={{ duration: 0.3, ease: EASE }}
        >
          <PauseIcon />
        </motion.span>
      )}
    </AnimatePresence>
  );
}

function SyncBar({ time }: { time: number }) {
  return (
    <div className="wt-bar">
      <span className="wt-time">{clock(time)}</span>
      <span className="wt-track">
        <span style={{ width: `${(time / SINTEL_LENGTH) * 100}%` }} />
      </span>
    </div>
  );
}

export function TogetherScene() {
  const t = useSeconds();
  const { time, frame, event } = together(t % 10);
  const paused = event === "pause";

  return (
    <div className="scene wt">
      <motion.div className="laptop" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.8, ease: EASE }}>
        <div className="laptop-lid">
          <div className="browser">
            <div className="browser-bar">
              <i />
              <i />
              <i />
              <span className="browser-url">192.168.1.20:8640</span>
            </div>
            <div className="browser-page">
              <Picture frame={frame} className="wt-picture" />
              <PausedGlyph show={paused} />
              <Toast event={event} />
              <div className="wt-panel">
                <div className="wt-panel-title">Watch together</div>
                <div className="wt-now">
                  <span>{paused ? "Paused" : "Playing now"}</span>
                  Sintel
                </div>
                <div className="wt-group">In this group</div>
                <div className="wt-person">
                  <span className="avatar" data-who="You">
                    Y
                  </span>
                  You <em>(you)</em>
                  <span className="wt-status">Host</span>
                </div>
                <div className="wt-person">
                  <span className="avatar" data-who="Sam">
                    S
                  </span>
                  Sam
                  <span className="wt-status">Here</span>
                </div>
              </div>
              <SyncBar time={time} />
            </div>
          </div>
        </div>
        <div className="laptop-base" />
      </motion.div>

      <div className="wt-link" aria-hidden>
        <span />
        <span />
        <span />
      </div>

      <motion.div
        className="tv-set"
        initial={{ opacity: 0, y: 16 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.8, delay: 0.15, ease: EASE }}
      >
        <div className="tv-panel">
          <Picture frame={frame} className="wt-picture" />
          <PausedGlyph show={paused} />
          <Toast event={event} />
          <SyncBar time={time} />
        </div>
        <div className="tv-stand" />
        <div className="tv-who">
          <span className="avatar" data-who="Sam">
            S
          </span>
          Sam's TV
        </div>
      </motion.div>
    </div>
  );
}

/* 5. Everyone -------------------------------------------------------------- */

const SHELF = {
  bbb: { title: "Big Buck Bunny", art: "poster-big-buck-bunny", palette: "moss", year: 2008 },
  cosmos: { title: "Cosmos Laundromat", art: "poster-cosmos-laundromat", palette: "violet", year: 2015 },
  dream: { title: "Elephants Dream", palette: "slate", year: 2006 },
  sintel: { title: "Sintel", art: "poster-sintel", palette: "ember", year: 2010 },
  spring: { title: "Spring", palette: "jade", year: 2019 },
  tos: { title: "Tears of Steel", art: "poster-tears-of-steel", palette: "ocean", year: 2012 },
  pioneer: { title: "Pioneer One", palette: "teal", sub: "S1 · E4" },
} satisfies Record<string, Shelf>;

const PEOPLE: { name: string; bg: "dusk" | "ocean" | "ember"; rows: { title: string; items: Shelf[] }[] }[] = [
  {
    name: "Ana",
    bg: "dusk",
    rows: [
      {
        title: "Continue Watching",
        items: [
          { ...SHELF.sintel, progress: 0.62, sub: "5 min left" },
          { ...SHELF.pioneer, progress: 0.35 },
          { ...SHELF.tos, progress: 0.18, sub: "10 min left" },
        ],
      },
      { title: "Recently Added", items: [SHELF.cosmos, SHELF.spring, SHELF.bbb, { ...SHELF.dream, watched: true }, SHELF.sintel, SHELF.tos] },
    ],
  },
  {
    name: "Ben",
    bg: "ocean",
    rows: [
      { title: "Recently Added", items: [SHELF.cosmos, SHELF.spring, SHELF.bbb, SHELF.dream, { ...SHELF.sintel, watched: true }, SHELF.tos] },
      {
        title: "Continue Watching",
        items: [
          { ...SHELF.tos, progress: 0.81, sub: "2 min left" },
          { ...SHELF.bbb, progress: 0.4, sub: "6 min left" },
        ],
      },
    ],
  },
  {
    name: "Kai",
    bg: "ember",
    rows: [
      {
        title: "Continue Watching",
        items: [
          { ...SHELF.cosmos, progress: 0.5, sub: "6 min left" },
          { ...SHELF.dream, progress: 0.25, sub: "8 min left" },
          { ...SHELF.pioneer, progress: 0.7, sub: "S1 · E2" },
          { ...SHELF.spring, progress: 0.1, sub: "7 min left" },
        ],
      },
      { title: "Movies", items: [{ ...SHELF.bbb, watched: true }, SHELF.cosmos, SHELF.dream, SHELF.sintel, SHELF.spring, SHELF.tos] },
    ],
  },
];

export function EveryoneScene() {
  const t = useSeconds(6);
  const person = PEOPLE[Math.floor(t / 3.4) % PEOPLE.length]!;

  return (
    <div className="scene hm">
      <div className="app">
        <AnimatePresence initial={false}>
          <motion.div
            key={person.bg}
            className="hm-wash"
            data-bg={person.bg}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 1, ease: EASE }}
          />
        </AnimatePresence>
        <AppTop>
          <span className="app-icon">
            <SlidersIcon />
          </span>
          <span className="app-icon">
            <UsersIcon />
          </span>
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span
              key={person.name}
              className="avatar"
              data-who={person.name}
              initial={{ opacity: 0, scale: 0.6 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.6 }}
              transition={{ duration: 0.35, ease: EASE }}
            >
              {person.name[0]}
            </motion.span>
          </AnimatePresence>
        </AppTop>
        <div className="app-body">
          <AppRail active="home" />
          <div className="app-content">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={person.name}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.35, ease: EASE }}
              >
                <div className="hm-hello">
                  Signed in as <b>{person.name}</b>
                </div>
                {person.rows.map((row) => (
                  <div key={row.title} className="hm-row">
                    <div className="hm-row-title">{row.title}</div>
                    <div className="hm-strip">
                      {row.items.map((item, i) => (
                        <PosterCard key={item.title + i} item={item} delay={i * 0.04} />
                      ))}
                    </div>
                  </div>
                ))}
              </motion.div>
            </AnimatePresence>
          </div>
        </div>
      </div>
    </div>
  );
}
