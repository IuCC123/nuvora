import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouter, useSearch } from "@tanstack/react-router";
import { formatClock } from "@nuvora/core";
import { client, queries } from "../nuvora";
import { ArrowLeftIcon, Back10Icon, Forward10Icon, FullscreenIcon, PauseIcon, PipIcon, PlayIcon, SpinIcon, VolumeIcon } from "../components/Icon";
import { Notice, type NoticeText } from "../components/Notice";
import { Scrubber } from "../components/player/Scrubber";
import { Volume } from "../components/player/Volume";
import { fileGone, playerFailed, playState } from "../lib/playState";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useMediaState } from "../lib/useMediaState";
import { useProgressReporting } from "../lib/useProgressReporting";
import { loadVolume, saveVolume } from "../lib/volumeMemory";
import "../styles/player.css";

/** Controls fade after this long of playback without input. */
const AUTO_HIDE_MS = 3_000;
/** A stall shorter than this shows nothing, so a quick seek never flashes a spinner. */
const SPINNER_DELAY_MS = 500;
const PULSE_MS = 650;
const SKIP_SECONDS = 10;
const VOLUME_STEP = 0.1;

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
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data: movie } = useQuery(queries.item(id));
  const [failed, setFailed] = useState(false);
  useDocumentTitle(movie?.title);

  // Only the detail page links here, so stepping back returns to it without a new history entry.
  // A player opened from a pasted link has nothing behind it and swaps itself for the detail page.
  const back = useCallback(
    () => (canGoBack ? router.history.back() : void navigate({ to: "/movie/$id", params: { id }, replace: true })),
    [canGoBack, router, navigate, id],
  );
  const keepPlace = useCallback(
    (positionMs: number) => {
      const seconds = Math.floor(positionMs / 1000);
      void navigate({ to: "/movie/$id/play", params: { id }, search: { file, ...(seconds > 0 && { t: seconds }) }, replace: true, resetScroll: false });
    },
    [navigate, id, file],
  );
  const version = movie?.versions.find((v) => v.file_id === file);
  const decided = version ? playState([version]) : undefined;
  const notice = failed ? playerFailed : movie && !version ? fileGone : decided?.kind === "blocked" ? decided.notice : undefined;
  const src = !notice && version?.playback.kind === "direct_play" ? client.resolve(version.playback.url) : undefined;

  return (
    <PlayerView
      key={`${id}:${file}`}
      itemId={id}
      fileId={file}
      title={movie?.title}
      year={movie?.year}
      src={src}
      startAt={t}
      notice={notice}
      onBack={back}
      onFailed={() => setFailed(true)}
      onPosition={keepPlace}
    />
  );
}

interface PlayerViewProps {
  itemId: number;
  fileId: number;
  title: string | undefined;
  year: number | null | undefined;
  src: string | undefined;
  /** Seconds. Read once: the URL keeps moving with playback after that. */
  startAt: number | undefined;
  notice: NoticeText | undefined;
  onBack: () => void;
  onFailed: () => void;
  onPosition: (positionMs: number) => void;
}

type PulseKind = "play" | "pause" | "back" | "forward" | "volume";

function PlayerView({ itemId, fileId, title, year, src, startAt: initialStart, notice, onBack, onFailed, onPosition }: PlayerViewProps) {
  const [startAt] = useState(initialStart);
  const root = useRef<HTMLDivElement>(null);
  const [video, setVideo] = useState<HTMLVideoElement | null>(null);
  const media = useMediaState(video);
  const [ready, setReady] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [showDuration, setShowDuration] = useState(false);
  const [pulse, setPulse] = useState<{ kind: PulseKind; n: number } | null>(null);
  const [held, setHeld] = useState({ focus: false, hover: false, drag: false });
  const { awake, wake, sleep } = useAwake(AUTO_HIDE_MS);
  const buffering = useHeldFor(media.starved && !!src, SPINNER_DELAY_MS);
  const lastPointer = useRef("mouse");
  useProgressReporting(src ? video : null, itemId, fileId, onPosition);
  useReleaseStream(video);

  const known = Number.isFinite(media.duration) && media.duration > 0;
  const visible = !!notice || awake || media.paused || held.focus || held.hover || held.drag;
  const hold = (key: keyof typeof held, value: boolean) => setHeld((h) => (h[key] === value ? h : { ...h, [key]: value }));

  // Volume is applied before the element can start playing, so a quiet setting never blips at full volume.
  const attach = useCallback((el: HTMLVideoElement | null) => {
    if (el) Object.assign(el, loadVolume());
    setVideo(el);
  }, []);

  useEffect(() => {
    const onChange = () => setFullscreen(document.fullscreenElement === root.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const pulseTimer = useRef<number>(undefined);
  const flash = (kind: PulseKind) => {
    setPulse((p) => ({ kind, n: (p?.n ?? 0) + 1 }));
    window.clearTimeout(pulseTimer.current);
    pulseTimer.current = window.setTimeout(() => setPulse(null), PULSE_MS);
  };
  useEffect(() => () => window.clearTimeout(pulseTimer.current), []);

  const actions = {
    toggle() {
      if (!video) return;
      if (video.paused) {
        video.play().catch(() => {});
        flash("play");
      } else {
        video.pause();
        flash("pause");
      }
    },
    seek(seconds: number) {
      if (video && known) video.currentTime = Math.min(media.duration, Math.max(0, seconds));
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
      else void root.current?.requestFullscreen().catch(() => {});
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
    } else if (key === "ArrowUp" || key === "ArrowDown") {
      actions.volume((video.muted ? 0 : video.volume) + (key === "ArrowUp" ? VOLUME_STEP : -VOLUME_STEP));
      flash("volume");
    } else if (key === "m") {
      actions.toggleMute();
      flash("volume");
    } else if (key === "f") {
      actions.fullscreen();
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

  const remaining = Math.max(0, media.duration - media.time);
  const playing = !media.paused;
  const level = media.muted || media.volume === 0 ? "muted" : media.volume < 0.5 ? "low" : "high";
  const pipSupported = document.pictureInPictureEnabled;

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
          src={startAt ? `${src}#t=${startAt}` : src}
          autoPlay
          playsInline
          preload="auto"
          aria-label={title}
          onLoadedMetadata={(e) => {
            const v = e.currentTarget;
            // The `#t=` fragment normally starts there already; this covers a browser that ignored it.
            if (startAt && Math.abs(v.currentTime - startAt) > 0.5) v.currentTime = Math.min(startAt, v.duration);
          }}
          onLoadedData={(e) => !e.currentTarget.seeking && setReady(true)}
          onSeeked={() => setReady(true)}
          onError={(e) => e.currentTarget.error?.code !== MediaError.MEDIA_ERR_ABORTED && onFailed()}
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

      <div className="player-center" aria-hidden>
        {buffering && !notice && (
          <span className="player-spinner">
            <SpinIcon size={44} strokeWidth={2} />
          </span>
        )}
        {pulse && (
          <span key={pulse.n} className="player-pulse" data-kind={pulse.kind}>
            {pulse.kind === "play" && <PlayIcon size={34} />}
            {pulse.kind === "pause" && <PauseIcon size={34} />}
            {pulse.kind === "back" && <Back10Icon size={36} />}
            {pulse.kind === "forward" && <Forward10Icon size={36} />}
            {pulse.kind === "volume" && (
              <>
                <VolumeIcon level={level} size={28} />
                <span className="player-pulse-label">{level === "muted" ? "Muted" : `${Math.round(media.volume * 100)}%`}</span>
              </>
            )}
          </span>
        )}
      </div>

      {notice && (
        <div className="player-notice">
          <h1>{title}</h1>
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
          {title && (
            <div className="player-heading">
              <h1>{title}</h1>
              {year != null && <span className="player-year">{year}</span>}
            </div>
          )}
        </header>

        {src && !notice && (
          <div className="player-bottom" onPointerEnter={(e) => e.pointerType === "mouse" && hold("hover", true)} onPointerLeave={() => hold("hover", false)}>
            <div className="player-timeline">
              <span className="player-time">{formatClock(media.time * 1000)}</span>
              <Scrubber video={video} duration={media.duration} buffered={media.buffered} onSeek={actions.seek} onDragChange={(d) => hold("drag", d)} />
              <button
                type="button"
                className="player-time player-time-end"
                aria-label={known ? (showDuration ? `Duration ${formatClock(media.duration * 1000)}. Show time remaining` : `${formatClock(remaining * 1000)} remaining. Show duration`) : "Duration"}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => setShowDuration((s) => !s)}
              >
                {!known ? "0:00" : showDuration ? formatClock(media.duration * 1000) : `−${formatClock(Math.ceil(remaining) * 1000)}`}
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
              </div>
              <div className="player-controls-end">
                {pipSupported && (
                  <ControlButton label={media.pip ? "Exit picture in picture" : "Picture in picture"} tip="Picture in picture" onClick={actions.pip} pressed={media.pip}>
                    <PipIcon />
                  </ControlButton>
                )}
                <ControlButton label={fullscreen ? "Exit full screen" : "Full screen"} tip={`${fullscreen ? "Exit full screen" : "Full screen"} · F`} onClick={actions.fullscreen}>
                  <FullscreenIcon exit={fullscreen} />
                </ControlButton>
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
