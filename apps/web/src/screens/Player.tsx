import { useCallback, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouter, useRouterState, useSearch } from "@tanstack/react-router";
import { audioLanguage, formatClock, subtitleLabel, type EpisodeDetail, type EpisodeRef, type StreamingCopy, type OnlineSubtitleResult, type SubtitleTrack, type Version } from "@nuvora/core";
import { client, queries, servedClient } from "../nuvora";
import { ArrowLeftIcon, Back10Icon, Forward10Icon, FullscreenIcon, PauseIcon, PipIcon, PlayIcon, SpinIcon, SubtitlesIcon, VolumeIcon } from "../components/Icon";
import { Notice, type NoticeText } from "../components/Notice";
import { NextEpisodeButton } from "../components/player/NextEpisodeButton";
import { LoadingPanel, bufferedEnd, durationText, useLoadStats } from "../components/player/LoadingPanel";
import { PlaybackMode, playbackMode, type Mode } from "../components/player/PlaybackMode";
import { PlainSubtitles } from "../components/player/PlainSubtitles";
import { PlayerTitle } from "../components/player/PlayerTitle";
import { Scrubber } from "../components/player/Scrubber";
import { StyledSubtitles } from "../components/player/StyledSubtitles";
import { SubtitleMenu } from "../components/player/SubtitleMenu";
import { UpNext } from "../components/player/UpNext";
import { initials } from "@nuvora/core";
import { usePartyPlayer } from "../components/party/usePartyPlayer";
import { useHls, type HlsInfo } from "../components/player/useHls";
import { useRebuffer } from "../components/player/useRebuffer";
import { useRewrap } from "../components/player/useRewrap";
import { useSubtitles } from "../components/player/useSubtitles";
import { Volume } from "../components/player/Volume";
import { languageName } from "../lib/languageName";
import { fileGone, playerFailed, playState } from "../lib/playState";
import { loadSubtitleStyle, saveSubtitleStyle, type SubtitleScope, type SubtitleStyle } from "../lib/subtitleMemory";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useMediaState } from "../lib/useMediaState";
import { useProgressReporting, type ProgressTarget } from "../lib/useProgressReporting";
import { loadVolume, saveVolume } from "../lib/volumeMemory";
import { loadQuality, saveQuality, type Quality } from "../lib/qualityMemory";
import { awayFromHome, opensNatively } from "../platform/deviceProfile";
import "../styles/player.css";
import "../styles/subtitles.css";

/** Controls fade after this long of playback without input. */
const AUTO_HIDE_MS = 3_000;
/** A stall shorter than this shows nothing, so a quick seek never flashes a spinner. */
const SPINNER_DELAY_MS = 500;
const PULSE_MS = 650;
const SKIP_SECONDS = 10;
const VOLUME_STEP = 0.1;
/** Held down, these would flip back and forth; arrows keep repeating as steps. */
const TOGGLE_KEYS = new Set([" ", "k", "m", "f", "c", "Escape"]);

/**
 * The Nuvora player: the whole window is the picture, with a top bar (back,
 * title) and a bottom bar (timeline, transport, volume, picture-in-picture,
 * fullscreen) that fade away during playback. The URL comes from the cached
 * item's playback decision; `?t=` starts there before the first frame shows,
 * and follows every reported position so a reload resumes where the viewer was.
 */
export function Player() {
  const { id } = useParams({ from: "/movie/$id/play" });
  const { file, t } = useSearch({ from: "/movie/$id/play" });
  return <PlayerScreen route="/movie/$id/play" id={id} file={file} t={t} />;
}

/** An episode's player. Its loader always settles on a file, so a missing one only means the redirect is still on its way. */
export function EpisodePlayer() {
  const { id } = useParams({ from: "/episode/$id/play" });
  const { file, t } = useSearch({ from: "/episode/$id/play" });
  return file === undefined ? null : <PlayerScreen route="/episode/$id/play" id={id} file={file} t={t} />;
}

function PlayerScreen({ route, id, file, t }: { route: "/movie/$id/play" | "/episode/$id/play"; id: number; file: number; t: number | undefined }) {
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data } = useQuery(queries.item(id));
  const movie = data?.kind === "movie" || data?.kind === "episode" ? data : undefined;
  const episode = movie?.kind === "episode" ? movie : undefined;
  const [failed, setFailed] = useState(false);
  // The server's own stream for a file this browser was told it could play directly but couldn't.
  const [served, setServed] = useState<{ file: number; playback: Version["playback"] } | null>(null);
  const asking = useRef(false);
  useDocumentTitle(episode ? `${episode.show_title} · ${episode.label}` : movie?.title);
  useLeaveFullscreen();

  // Only the detail page links here, so stepping back returns to it without a new history entry.
  // A player opened from a pasted link has nothing behind it and swaps itself for the detail page.
  const back = useCallback(() => {
    if (canGoBack) return router.history.back();
    if (episode) void navigate({ to: "/show/$id", params: { id: episode.show_id }, search: { season: episode.season }, replace: true });
    else void navigate({ to: "/movie/$id", params: { id }, replace: true });
  }, [canGoBack, router, navigate, id, episode]);
  const keepPlace = useCallback(
    (positionMs: number) => {
      const seconds = Math.floor(positionMs / 1000);
      const search = { file, ...(seconds > 0 && { t: seconds }) };
      if (route === "/movie/$id/play") void navigate({ to: route, params: { id }, search, replace: true, resetScroll: false });
      else void navigate({ to: route, params: { id }, search, replace: true, resetScroll: false });
    },
    [navigate, route, id, file],
  );
  // The next episode replaces this one in history, so Back still returns to the show.
  const playNext = useCallback((next: EpisodeRef) => void navigate({ to: "/episode/$id/play", params: { id: next.id }, search: {}, replace: true }), [navigate]);
  const version = movie?.versions.find((v) => v.file_id === file);
  const durationMs = version?.media?.duration_ms;
  // A time at or past the end would open on the last frame; such a link starts from the beginning.
  const startAt = t !== undefined && (durationMs == null || t < durationMs / 1000 - 1) ? t : undefined;
  // The router drops a `t` it can't use but leaves it in the address. On opening, put back the one actually used.
  const rawT = useRouterState({ select: (s) => new URLSearchParams(s.location.searchStr).get("t") });
  useEffect(() => {
    if (rawT !== (startAt === undefined ? null : String(startAt))) keepPlace((startAt ?? 0) * 1000);
  }, [id, file]);
  const decided = version ? playState([version]) : undefined;
  const notice = failed ? playerFailed : movie && !version ? fileGone : decided?.kind === "blocked" ? decided.notice : undefined;
  const fromServer = served?.file === file;
  const playback = notice ? undefined : fromServer ? served.playback : version?.playback;

  // A 4K or very high bitrate file plays from the server's 1080p copy once there is one. At home the viewer's
  // last choice holds; away from home it always starts from the copy, which is all most connections carry.
  const heavy = !!version && isHeavy(version);
  const [quality, setQuality] = useState<Quality>(() => (awayFromHome() ? "copy" : loadQuality()));
  const [autoSwitched, setAutoSwitched] = useState(false);
  const [copyBroken, setCopyBroken] = useState(false);
  const [switchedAt, setSwitchedAt] = useState<number | undefined>(undefined);
  const { data: copy, isPending: copyPending } = useQuery({
    ...queries.streamingCopy(file),
    enabled: heavy && !notice,
    refetchInterval: (q) => (q.state.data?.state === "making" || q.state.data?.state === "queued" ? 30_000 : false),
  });
  const copyReady = copy?.state === "ready" && !!copy.url && !copyBroken && !fromServer;
  const playsCopy = copyReady && quality === "copy";
  // Until the server says whether there is a copy, nothing starts: no original to throw away a second later.
  const deciding = heavy && !notice && copyPending;
  const source = deciding ? undefined : playsCopy ? { kind: "hls" as const, url: client.resolve(copy.url!) } : sourceFor(playback);
  const mode = playsCopy ? COPY_MODE : playback ? playbackMode(playback, source?.kind === "rewrap") : undefined;
  const switchQuality = useCallback(
    (to: Quality, at: number) => {
      setSwitchedAt(at);
      keepPlace(Math.floor(at * 1000));
      setQuality(to);
      saveQuality(to);
    },
    [keepPlace],
  );

  // A file that won't play directly here after all gets the server's stream, from where it stopped.
  const fallBack = useCallback(() => {
    if (version?.playback.kind !== "direct_play" || fromServer) return setFailed(true);
    if (asking.current) return;
    asking.current = true;
    servedClient.item(id).then(
      (item) => {
        asking.current = false;
        const versions = item.kind === "movie" || item.kind === "episode" ? item.versions : [];
        const stream = versions.find((v) => v.file_id === file)?.playback;
        if (stream?.kind === "hls") setServed({ file, playback: stream });
        else setFailed(true);
      },
      () => {
        asking.current = false;
        setFailed(true);
      },
    );
  }, [version, fromServer, id, file]);

  return (
    <PlayerView
      key={`${id}:${file}:${fromServer ? "server" : playsCopy ? "copy" : "file"}`}
      catalog={{ itemId: id, fileId: file }}
      title={episode?.show_title ?? movie?.title}
      year={movie?.kind === "movie" ? movie.year : undefined}
      episode={episode}
      onNext={playNext}
      source={source}
      mode={mode}
      startAt={switchedAt ?? startAt}
      durationHint={durationMs != null ? durationMs / 1000 : undefined}
      about={{
        source: playsCopy ? "1080p copy · H.264" : version ? (heavy ? `Original · ${version.summary}` : version.summary) : null,
        bitrate: playsCopy && copy.state === "ready" ? copy.bitrate : (version?.media?.bitrate ?? null),
        hint: autoSwitched && playsCopy ? AUTO_SWITCH_HINT : heavy && !playsCopy ? copyHint(copy, copyReady) : null,
      }}
      quality={
        copyReady && copy.state === "ready"
          ? {
              current: quality,
              copyLabel: `1080p copy · ${(copy.bitrate / 1e6).toFixed(1)} Mbit/s`,
              originalLabel: `Original · ${version?.summary ?? ""}`,
              onSwitch: switchQuality,
            }
          : undefined
      }
      subtitleTracks={version?.media?.subtitles ?? NO_TRACKS}
      audioLanguage={version?.media ? audioLanguage(version.media.audio) : null}
      subtitleScope={episode ? `show:${episode.show_id}` : `movie:${id}`}
      notice={notice}
      onBack={back}
      onTooSlow={
        // Once only: a viewer who switches back to the original after this keeps it.
        heavy && copyReady && !playsCopy && !autoSwitched
          ? (at) => {
              setAutoSwitched(true);
              switchQuality("copy", at);
            }
          : undefined
      }
      // A copy that won't play falls back to the original at home. Away from home the original
      // needs far more than most connections carry, so the player says it stopped instead.
      onFailed={playsCopy ? () => (awayFromHome() ? setFailed(true) : setCopyBroken(true)) : fallBack}
      onPosition={keepPlace}
    />
  );
}

const NO_TRACKS: SubtitleTrack[] = [];

const AUTO_SWITCH_HINT = "Switched to the 1080p copy: the original needs more than this connection carries. The quality button switches back.";

const COPY_MODE: Mode = {
  label: "1080p copy",
  detail: "A lighter copy the server made of this video, so it streams without stopping. Switch to the original with the quality button.",
};

/** Mirrors the server's rule for which videos get a streaming copy: taller than 1080p, or above 40 Mbit/s. */
function isHeavy(version: Version): boolean {
  const media = version.media;
  if (!media) return false;
  return media.video.some((v) => v.height > 1080) || (media.bitrate ?? 0) > 40_000_000;
}

/** Under the loading figures while the original plays: the copy to switch to, or how far it has got. */
function copyHint(copy: StreamingCopy | undefined, ready: boolean): string | null {
  switch (copy?.state) {
    case "ready":
      return ready ? "The 1080p copy streams smoothly: switch with the quality button." : null;
    case "making":
      return copy.paused
        ? `A 1080p copy that streams smoothly is ${Math.floor(copy.progress * 100)}% made. It carries on whenever nobody is streaming through the server.`
        : `A 1080p copy that streams smoothly is ${Math.floor(copy.progress * 100)}% made${copy.eta_s != null ? `, ready in about ${durationText(copy.eta_s)}` : ""}.`;
    case "queued":
      return "A 1080p copy that streams smoothly is waiting its turn on the server.";
    default:
      return null;
  }
}

/** The quality switch: the server's 1080p copy or the original file. */
interface QualityChoice {
  current: Quality;
  copyLabel: string;
  originalLabel: string;
  /** `at`: where playback is, in seconds, so the other version opens there. */
  onSwitch: (to: Quality, at: number) => void;
}

/** What the loading panel says about the video itself. */
interface About {
  source: string | null;
  /** Bit/s. */
  bitrate: number | null;
  hint: string | null;
}

/** The file as it is (opened by the browser, or rewrapped here), or streamed through the server as HLS. */
export type Source = { kind: "direct" | "rewrap" | "hls"; url: string };

/** Where a playback decision says to get the picture from. */
export function sourceFor(playback: Version["playback"] | undefined): Source | undefined {
  if (playback?.kind === "direct_play") return { kind: opensNatively(playback.mime_type) ? "direct" : "rewrap", url: client.resolve(playback.url) };
  if (playback?.kind === "hls") return { kind: "hls", url: client.resolve(playback.url) };
  return undefined;
}

interface PlayerViewProps {
  /** The library file playing, for progress, Watch Together, subtitles and seek previews; `null` for a torrent stream. */
  catalog: { itemId: number; fileId: number } | null;
  /** A torrent stream's id: where its progress goes, for Continue Watching. */
  streamId?: string;
  title: string | undefined;
  year: number | null | undefined;
  episode: EpisodeDetail | undefined;
  onNext: (next: EpisodeRef) => void;
  source: Source | undefined;
  mode: Mode | undefined;
  /** Seconds. Read once: the URL keeps moving with playback after that. */
  startAt: number | undefined;
  /** Seconds, from the catalog: shown until the video reports its own. */
  durationHint?: number;
  about?: About;
  quality?: QualityChoice;
  subtitleTracks: readonly SubtitleTrack[];
  audioLanguage: string | null;
  /** What a subtitle choice is remembered for: the show for an episode, else the movie. */
  subtitleScope: SubtitleScope;
  notice: NoticeText | undefined;
  /** Before there is anything to play: what the player waits for (a torrent stream starting). */
  waiting?: string | null;
  /** Under the spinner while waiting or buffering: how the stream's download goes. */
  streamNote?: string | null;
  onBack: () => void;
  onFailed: () => void;
  /** Called once if the stream can't keep up (see `useTooSlow`), with the playhead in seconds. */
  onTooSlow?: (at: number) => void;
  onPosition: (positionMs: number) => void;
}

type PulseKind = "play" | "pause" | "back" | "forward" | "volume" | "subtitles-on" | "subtitles-off";

export function PlayerView({ catalog, streamId, title, year, episode, onNext, source, mode, startAt: initialStart, durationHint, about, quality, subtitleTracks, audioLanguage, subtitleScope, notice, waiting, streamNote, onBack, onFailed, onTooSlow, onPosition }: PlayerViewProps) {
  const [startAt] = useState(initialStart);
  const src = source?.url;
  const root = useRef<HTMLDivElement>(null);
  const [video, setVideo] = useState<HTMLVideoElement | null>(null);
  const media = useMediaState(video);
  const [fullscreen, setFullscreen] = useState(false);
  const [showDuration, setShowDuration] = useState(false);
  const [pulse, setPulse] = useState<{ kind: PulseKind; n: number; label?: string } | null>(null);
  const [held, setHeld] = useState({ focus: false, hover: false, drag: false, menu: false });
  const { awake, wake, sleep } = useAwake(AUTO_HIDE_MS);
  const [ready, setReady] = useState(false);
  const lastPointer = useRef("mouse");
  const progressTarget: ProgressTarget | null = catalog ? { kind: "file", ...catalog } : streamId ? { kind: "stream", id: streamId } : null;
  useProgressReporting(src ? video : null, progressTarget, onPosition);
  // On failure, keep the place first, so the server's stream picks up from here.
  const fail = () => {
    if (video && video.currentTime > 0) onPosition(Math.floor(video.currentTime * 1000));
    onFailed();
  };
  const hlsInfo = useHls(video, source?.kind === "hls" ? source.url : undefined, startAt, fail);
  useTooSlow(source?.kind === "hls" ? video : null, hlsInfo, onTooSlow, startAt ?? 0);
  useRewrap(video, source?.kind === "rewrap" ? source.url : undefined, startAt, fail);
  const party = usePartyPlayer(src ? video : null, catalog, startAt);
  // Only where the player feeds the buffer: `<video src>` never holds the seconds a hold waits for.
  const rebuffer = useRebuffer(src ? video : null, !party.inParty && source?.kind !== "direct");
  // The figures show once a load or a stall lasts long enough to notice, so a quick seek never flashes them.
  const loading = useHeldFor(!!src && !notice && party.countdown === null && (!ready || media.starved || rebuffer.holding), SPINNER_DELAY_MS);
  const loadStats = useLoadStats(src ? video : null, source?.kind === "hls" ? hlsInfo : null, about?.bitrate ?? null, loading);
  useReleaseStream(video);
  const subtitles = useSubtitles(catalog?.fileId ?? null, subtitleTracks, audioLanguage, subtitleScope);
  // Previews a new video doesn't have yet are made within a minute or so.
  const { data: previews } = useQuery({
    ...queries.previews(catalog?.fileId ?? 0),
    enabled: catalog !== null,
    refetchInterval: (q) => (q.state.data === null ? 15_000 : false),
  });
  const [subtitleStyle, setSubtitleStyle] = useState(loadSubtitleStyle);
  const changeSubtitleStyle = (style: SubtitleStyle) => {
    setSubtitleStyle(style);
    saveSubtitleStyle(style);
  };
  const holdForMenu = useCallback((open: boolean) => setHeld((h) => (h.menu === open ? h : { ...h, menu: open })), []);
  const bar = useBarHeight(root, !!src && !notice);

  const known = Number.isFinite(media.duration) && media.duration > 0;
  // iPhone Safari has no element full screen, only the video's own player.
  const videoFullscreen = (video as (HTMLVideoElement & { webkitEnterFullscreen?: () => void }) | null)?.webkitEnterFullscreen;
  const visible = !!notice || awake || media.paused || held.focus || held.hover || held.drag || held.menu;
  const hold = (key: keyof typeof held, value: boolean) => setHeld((h) => (h[key] === value ? h : { ...h, [key]: value }));

  // Volume is applied before the element can start playing, so a quiet setting never blips at full volume.
  const attach = useCallback((el: HTMLVideoElement | null) => {
    if (el) Object.assign(el, loadVolume());
    setVideo(el);
  }, []);

  useEffect(() => {
    const onChange = () => setFullscreen(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const pulseTimer = useRef<number>(undefined);
  const flash = (kind: PulseKind, label?: string) => {
    setPulse((p) => ({ kind, n: (p?.n ?? 0) + 1, ...(label && { label }) }));
    window.clearTimeout(pulseTimer.current);
    pulseTimer.current = window.setTimeout(() => setPulse(null), PULSE_MS);
  };
  useEffect(() => () => window.clearTimeout(pulseTimer.current), []);

  const actions = {
    toggle() {
      if (!video) return;
      if (party.blocked) {
        // The press itself lets the browser play; the party's clock then puts it in place.
        void video.play().catch(() => {});
        return;
      }
      if (party.active) {
        if (video.paused) {
          party.play(video.currentTime);
          flash("play");
        } else {
          video.pause();
          party.pause(video.currentTime);
          flash("pause");
        }
        return;
      }
      if (rebuffer.holding) {
        // Held for buffer, it looks like playing: a press means pause, and it stays paused.
        rebuffer.cancel();
        flash("pause");
      } else if (video.paused) {
        video.play().catch(() => {});
        flash("play");
      } else {
        video.pause();
        flash("pause");
      }
    },
    seek(seconds: number) {
      if (!video || !known) return;
      const to = Math.min(media.duration, Math.max(0, seconds));
      video.currentTime = to;
      if (party.active) party.seek(to);
    },
    skip(delta: number) {
      if (!video || !known) return;
      actions.seek(video.currentTime + delta);
      flash(delta < 0 ? "back" : "forward");
    },
    volume(level: number) {
      if (!video) return;
      video.volume = Math.min(1, Math.max(0, Math.round(level * 100) / 100));
      if (level > 0) video.muted = false;
      saveVolume({ volume: video.volume, muted: video.muted });
    },
    toggleMute() {
      if (!video) return;
      if (video.muted || video.volume === 0) {
        video.muted = false;
        if (video.volume === 0) video.volume = 0.5;
      } else {
        video.muted = true;
      }
      saveVolume({ volume: video.volume, muted: video.muted });
    },
    fullscreen() {
      if (document.fullscreenElement) void document.exitFullscreen();
      // The whole page, not this view: the next episode mounts a new view and full screen carries over.
      else if (document.fullscreenEnabled) void document.documentElement.requestFullscreen().catch(() => {});
      else if (videoFullscreen) {
        try {
          videoFullscreen.call(video);
        } catch {
          // Before the video has loaded, Safari refuses; the next press works.
        }
      }
    },
    subtitles(track: SubtitleTrack | OnlineSubtitleResult | null) {
      if (!track) flash("subtitles-off", "Subtitles off");
      else if ("same_release" in track) flash("subtitles-on", `${track.label} · online`);
      else flash("subtitles-on", subtitleLabel(track, languageName).name);
    },
    pip() {
      if (!video) return;
      if (document.pictureInPictureElement) void document.exitPictureInPicture();
      else void video.requestPictureInPicture().catch(() => {});
    },
  };

  const onKey = useRef<(e: KeyboardEvent) => void>(undefined);
  onKey.current = (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    wake();
    const target = e.target instanceof HTMLElement ? e.target : null;
    if ((e.key === " " || e.key === "Enter") && target?.closest("button")) return;
    if (target instanceof HTMLInputElement && e.key.startsWith("Arrow")) return;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (e.repeat && TOGGLE_KEYS.has(key)) return e.preventDefault();
    if (key === "Escape") {
      if (document.fullscreenElement) void document.exitFullscreen();
      else onBack();
    } else if (notice || !video) {
      return;
    } else if (key === " " || key === "k") {
      actions.toggle();
    } else if (key === "ArrowLeft" || key === "j") {
      actions.skip(-SKIP_SECONDS);
    } else if (key === "ArrowRight" || key === "l") {
      actions.skip(SKIP_SECONDS);
    } else if ((key === "ArrowUp" || key === "ArrowDown") && target?.getAttribute("role") === "slider") {
      // On the timeline, up and down move through time like any slider, not the volume.
      actions.skip(key === "ArrowUp" ? SKIP_SECONDS : -SKIP_SECONDS);
    } else if (key === "ArrowUp" || key === "ArrowDown") {
      actions.volume((video.muted ? 0 : video.volume) + (key === "ArrowUp" ? VOLUME_STEP : -VOLUME_STEP));
      flash("volume");
    } else if (key === "m") {
      actions.toggleMute();
      flash("volume");
    } else if (key === "f") {
      actions.fullscreen();
    } else if (key === "c" && subtitles.tracks.some((t) => t.renderable)) {
      actions.subtitles(e.shiftKey ? subtitles.cycle() : subtitles.toggle());
    } else if (/^[0-9]$/.test(key)) {
      actions.seek((media.duration * Number(key)) / 10);
    } else {
      return;
    }
    e.preventDefault();
  };
  useEffect(() => {
    const listener = (e: KeyboardEvent) => onKey.current?.(e);
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, []);

  // Before the first frame, the place it opens at and the catalog's length, not 0:00 of nothing.
  const shownTime = ready ? media.time : (startAt ?? 0);
  const shownDuration = known ? media.duration : (durationHint ?? Number.NaN);
  const lengthKnown = Number.isFinite(shownDuration) && shownDuration > 0;
  const remaining = Math.max(0, shownDuration - shownTime);
  const playing = !media.paused || rebuffer.holding;
  const level = media.muted || media.volume === 0 ? "muted" : media.volume < 0.5 ? "low" : "high";
  const pipSupported = document.pictureInPictureEnabled;
  const canFullscreen = document.fullscreenEnabled || !!videoFullscreen;

  return (
    <div
      ref={root}
      className="player"
      data-controls={visible ? "shown" : "hidden"}
      onPointerMove={(e) => e.pointerType !== "touch" && wake()}
      onPointerDown={(e) => {
        lastPointer.current = e.pointerType;
        if (e.pointerType !== "touch") wake();
      }}
    >
      {src && (
        <video
          ref={attach}
          className="player-video"
          data-ready={ready}
          src={source?.kind === "direct" ? (startAt ? `${src}#t=${startAt}` : src) : undefined}
          // In a party the party's clock starts it, everyone at once.
          autoPlay={!party.inParty}
          playsInline
          preload="auto"
          aria-label={title}
          onLoadedMetadata={(e) => {
            const v = e.currentTarget;
            if (!startAt) return;
            if (startAt >= v.duration - 1) {
              v.currentTime = 0;
              onPosition(0);
            } else if (Math.abs(v.currentTime - startAt) > 0.5) {
              // The `#t=` fragment normally starts there already; this covers a browser that ignored it.
              v.currentTime = startAt;
            }
          }}
          onLoadedData={(e) => !e.currentTarget.seeking && setReady(true)}
          onSeeked={() => setReady(true)}
          onError={(e) => source && source.kind !== "hls" && e.currentTarget.error?.code !== MediaError.MEDIA_ERR_ABORTED && fail()}
        />
      )}
      {src && (
        <div
          className="player-stage"
          aria-hidden
          onClick={() => lastPointer.current !== "touch" && actions.toggle()}
          onDoubleClick={() => lastPointer.current !== "touch" && actions.fullscreen()}
          onPointerUp={(e) => {
            if (e.pointerType !== "touch") return;
            if (visible) sleep();
            else wake();
          }}
        />
      )}

      <div className="player-scrim" data-edge="top" aria-hidden />
      {src && !notice && <div className="player-scrim" data-edge="bottom" aria-hidden />}
      {video && subtitles.render.kind === "plain" && (
        <PlainSubtitles video={video} cues={subtitles.render.cues} style={subtitleStyle} delay={subtitles.delay} />
      )}
      {video && subtitles.render.kind === "styled" && (
        <StyledSubtitles
          video={video}
          track={subtitles.render.track}
          content={subtitles.render.content}
          fonts={subtitles.render.fonts}
          delay={subtitles.delay}
          onReady={subtitles.styledReady}
          onFailed={subtitles.styledFailed}
        />
      )}

      {party.note && (
        <div key={party.note} className="party-note" role="status">
          {party.note}
        </div>
      )}
      {party.countdown !== null && !notice && (
        <div className="party-countdown" role="status" aria-live="polite">
          <span className="party-countdown-label">Starting together</span>
          <span key={party.countdown} className="party-countdown-number">
            {party.countdown}
          </span>
        </div>
      )}
      {party.blocked && !notice && (
        <div className="party-countdown">
          <button type="button" className="btn btn-primary party-tap" onClick={actions.toggle}>
            <PlayIcon size={18} />
            Join the playback
          </button>
        </div>
      )}

      <div className="player-center" aria-hidden>
        {loading && (
          <LoadingPanel
            phase={ready ? "rebuffer" : "start"}
            stats={loadStats}
            goal={rebuffer.goal}
            source={about?.source ?? null}
            mode={mode?.label ?? null}
            bitrate={loadStats?.fragment || source?.kind === "hls" ? (hlsInfo.current.bitrate ?? about?.bitrate ?? null) : (about?.bitrate ?? null)}
            hint={streamNote ?? about?.hint ?? null}
          />
        )}
        {pulse && (
          <span key={pulse.n} className="player-pulse" data-kind={pulse.kind}>
            {pulse.kind === "play" && <PlayIcon size={34} />}
            {pulse.kind === "pause" && <PauseIcon size={34} />}
            {pulse.kind === "back" && <Back10Icon size={36} />}
            {pulse.kind === "forward" && <Forward10Icon size={36} />}
            {(pulse.kind === "subtitles-on" || pulse.kind === "subtitles-off") && (
              <>
                <SubtitlesIcon on={pulse.kind === "subtitles-on"} size={26} />
                <span className="player-pulse-label">{pulse.label}</span>
              </>
            )}
            {pulse.kind === "volume" && (
              <>
                <VolumeIcon level={level} size={28} />
                <span className="player-pulse-label">{level === "muted" ? "Muted" : `${Math.round(media.volume * 100)}%`}</span>
              </>
            )}
          </span>
        )}
      </div>

      {episode?.next && src && !notice && <UpNext video={video} next={episode.next} controlsShown={visible} onPlay={onNext} />}

      {waiting && !src && !notice && (
        <div className="player-waiting" role="status" aria-live="polite">
          <span className="player-spinner">
            <SpinIcon size={44} />
          </span>
          <p className="player-waiting-label">{waiting}</p>
          {streamNote && <p className="player-stream-note">{streamNote}</p>}
        </div>
      )}

      {notice && (
        <div className="player-notice">
          <Notice text={notice} />
        </div>
      )}

      <div
        className="player-chrome"
        onFocus={(e) => hold("focus", e.target.matches(":focus-visible"))}
        onBlur={() => hold("focus", false)}
      >
        <header className="player-top" onPointerEnter={(e) => e.pointerType === "mouse" && hold("hover", true)} onPointerLeave={() => hold("hover", false)}>
          <button type="button" className="player-btn player-back" aria-label={title ? `Back to ${title}` : "Back"} data-tip="Back · Esc" onMouseDown={(e) => e.preventDefault()} onClick={onBack}>
            <ArrowLeftIcon />
          </button>
          <PlayerTitle title={title} year={year} episode={episode} />
          <PlaybackMode mode={mode} />
          {party.active && party.others.length > 0 && (
            <div className="party-with" title={`Watching with ${party.others.map((m) => m.display_name).join(", ")}`}>
              <span className="party-with-faces" aria-hidden>
                {party.others.slice(0, 3).map((m) => (
                  <span key={m.id} className="together-face" data-away={!m.online}>
                    {initials(m.display_name)}
                  </span>
                ))}
              </span>
              <span>
                With {party.others[0]!.display_name}
                {party.others.length > 1 && ` +${party.others.length - 1}`}
              </span>
            </div>
          )}
        </header>

        {src && !notice && (
          <div ref={bar} className="player-bottom" onPointerEnter={(e) => e.pointerType === "mouse" && hold("hover", true)} onPointerLeave={() => hold("hover", false)}>
            <div className="player-timeline">
              <span className="player-time">{formatClock(shownTime * 1000)}</span>
              <Scrubber video={video} duration={shownDuration} pendingTime={ready ? undefined : shownTime} buffered={media.buffered} previews={previews ?? null} onSeek={actions.seek} onDragChange={(d) => hold("drag", d)} />
              <button
                type="button"
                className="player-time player-time-end"
                aria-label={lengthKnown ? (showDuration ? `Duration ${formatClock(shownDuration * 1000)}. Show time remaining` : `${formatClock(remaining * 1000)} remaining. Show duration`) : "Duration"}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => setShowDuration((s) => !s)}
              >
                {!lengthKnown ? "–:––" : showDuration ? formatClock(shownDuration * 1000) : `−${formatClock(Math.ceil(remaining) * 1000)}`}
              </button>
            </div>
            <div className="player-controls">
              <div className="player-controls-start">
                <Volume volume={media.volume} muted={media.muted} onVolume={actions.volume} onToggleMute={actions.toggleMute} />
              </div>
              <div className="player-controls-center">
                <ControlButton label={`Back ${SKIP_SECONDS} seconds`} tip="Back 10 s · J" onClick={() => actions.skip(-SKIP_SECONDS)}>
                  <Back10Icon />
                </ControlButton>
                <ControlButton label={playing ? "Pause" : "Play"} tip={`${playing ? "Pause" : "Play"} · K`} onClick={actions.toggle} big>
                  {playing ? <PauseIcon size={30} /> : <PlayIcon size={30} />}
                </ControlButton>
                <ControlButton label={`Forward ${SKIP_SECONDS} seconds`} tip="Forward 10 s · L" onClick={() => actions.skip(SKIP_SECONDS)}>
                  <Forward10Icon />
                </ControlButton>
                {episode?.next && <NextEpisodeButton next={episode.next} onPlay={onNext} />}
              </div>
              <div className="player-controls-end">
                {quality && (
                  <button
                    type="button"
                    className="player-btn player-quality"
                    aria-label={quality.current === "copy" ? `Playing the ${quality.copyLabel}. Switch to the ${quality.originalLabel}` : `Playing the ${quality.originalLabel}. Switch to the ${quality.copyLabel}`}
                    data-tip={quality.current === "copy" ? `Switch to ${quality.originalLabel}` : `Switch to ${quality.copyLabel}`}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => quality.onSwitch(quality.current === "copy" ? "original" : "copy", video?.currentTime ?? shownTime)}
                  >
                    {quality.current === "copy" ? "1080p" : "Original"}
                  </button>
                )}
                {subtitles.tracks.length > 0 && (
                  <SubtitleMenu subtitles={subtitles} style={subtitleStyle} onStyle={changeSubtitleStyle} onOpenChange={holdForMenu} />
                )}
                {pipSupported && (
                  <ControlButton label={media.pip ? "Exit picture in picture" : "Picture in picture"} tip="Picture in picture" onClick={actions.pip} pressed={media.pip}>
                    <PipIcon />
                  </ControlButton>
                )}
                {canFullscreen && (
                  <ControlButton label={fullscreen ? "Exit full screen" : "Full screen"} tip={`${fullscreen ? "Exit full screen" : "Full screen"} · F`} onClick={actions.fullscreen}>
                    <FullscreenIcon exit={fullscreen} />
                  </ControlButton>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

interface ControlButtonProps {
  label: string;
  /** Shown on hover: the action and its key. */
  tip: string;
  onClick: () => void;
  big?: boolean;
  pressed?: boolean;
  children: ReactNode;
}

/** A player button. A mouse press doesn't take focus, so Space keeps meaning play and pause afterwards. */
function ControlButton({ label, tip, onClick, big, pressed, children }: ControlButtonProps) {
  return (
    <button
      type="button"
      className={big ? "player-btn player-btn-big" : "player-btn"}
      aria-label={label}
      aria-pressed={pressed}
      data-tip={tip}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/**
 * A `<video>` taken out of the page keeps its stream open until it is garbage
 * collected. Emptying it on the way out closes the connection now. Called after
 * `useProgressReporting`, so its cleanup runs second and the last report still
 * reads the real position.
 */
function useReleaseStream(video: HTMLVideoElement | null) {
  useEffect(() => {
    if (!video) return;
    return () => {
      video.pause();
      video.removeAttribute("src");
      video.load();
    };
  }, [video]);
}

/** Publishes the bottom bar's height as `--player-bar` on the player, so subtitles can rise just clear of it. */
function useBarHeight(root: RefObject<HTMLDivElement | null>, shown: boolean) {
  const bar = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = bar.current;
    const player = root.current;
    if (!shown || !el || !player) return;
    const observer = new ResizeObserver(() => player.style.setProperty("--player-bar", `${el.offsetHeight}px`));
    observer.observe(el);
    return () => observer.disconnect();
  }, [root, shown]);
  return bar;
}

/** Full screen belongs to the page while the player is open; leaving the player leaves it too. */
function useLeaveFullscreen() {
  useEffect(() => () => void (document.fullscreenElement && document.exitFullscreen().catch(() => {})), []);
}

/** True until `ms` pass without a `wake()`. */
function useAwake(ms: number) {
  const [awake, setAwake] = useState(true);
  const timer = useRef<number>(undefined);
  const wake = useCallback(() => {
    setAwake(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setAwake(false), ms);
  }, [ms]);
  const sleep = useCallback(() => {
    window.clearTimeout(timer.current);
    setAwake(false);
  }, []);
  useEffect(() => {
    wake();
    return () => window.clearTimeout(timer.current);
  }, [wake]);
  return { awake, wake, sleep };
}

/** True once `value` has stayed true for `ms`; false as soon as it isn't. */
function useHeldFor(value: boolean, ms: number) {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    if (!value) return setHeld(false);
    const timer = window.setTimeout(() => setHeld(true), ms);
    return () => window.clearTimeout(timer);
  }, [value, ms]);
  return held;
}

/**
 * Calls `onTooSlow` once when a stream can't keep up and less than 20 s is buffered: two segments came in
 * slower than they play (`fill` under 0.9), the one arriving is on course to take more than twice its
 * length, a segment hasn't started arriving after 15 s with under 2 s left (the server can't make it
 * in time, like 4K converted on a small server), or playback stalled twice within 90 s. Seeks don't
 * count. A 52 Mbit/s original over a 15 Mbit/s link shows within about 8 s, where waiting for whole
 * segments would take a minute. `start` is where playback began, for a stream that never got going.
 */
function useTooSlow(video: HTMLVideoElement | null, info: RefObject<HlsInfo>, onTooSlow: ((at: number) => void) | undefined, start: number) {
  const latest = useRef(onTooSlow);
  latest.current = onTooSlow;
  const watching = !!onTooSlow;
  useEffect(() => {
    if (!video || !watching) return;
    let reported = false;
    let started = false;
    let stalls: number[] = [];
    const report = () => {
      if (reported) return;
      reported = true;
      latest.current?.(video.readyState > HTMLMediaElement.HAVE_NOTHING ? video.currentTime : start);
    };
    const onPlaying = () => {
      started = true;
    };
    const onWaiting = () => {
      if (!started || video.seeking) return;
      const now = performance.now();
      stalls = [...stalls.filter((t) => now - t < 90_000), now];
      if (stalls.length >= 2) report();
    };
    const timer = window.setInterval(() => {
      const ahead = bufferedEnd(video) - video.currentTime;
      if (ahead >= 20) return;
      const { fragments, fill, fragment } = info.current;
      if (fragments >= 2 && fill !== null && fill < 0.9) report();
      if (fragment && fragment.loaded === 0 && ahead < 2 && performance.now() - fragment.requested > 15_000) report();
      if (fragment && fragment.duration > 0 && fragment.total > 0 && fragment.loaded > 0) {
        const elapsed = (performance.now() - fragment.requested) / 1000;
        const projected = elapsed * (fragment.total / fragment.loaded);
        if (elapsed > 6 && fragment.duration / projected < 0.5) report();
      }
    }, 2_000);
    video.addEventListener("playing", onPlaying);
    video.addEventListener("waiting", onWaiting);
    return () => {
      window.clearInterval(timer);
      video.removeEventListener("playing", onPlaying);
      video.removeEventListener("waiting", onWaiting);
    };
  }, [video, info, watching, start]);
}
