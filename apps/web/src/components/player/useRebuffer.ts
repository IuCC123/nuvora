import { useCallback, useEffect, useRef, useState } from "react";
import { bufferedEnd } from "./LoadingPanel";

/** Seconds of buffer a stall waits for before playing again. */
const GOAL_SECONDS = 8;
/** After a second stall this soon, wait for more: loading is slower than playback. */
const REPEAT_WINDOW_MS = 90_000;
const REPEAT_GOAL_SECONDS = 20;
const CHECK_MS = 200;

export interface Rebuffer {
  /** Paused by the player to build up a buffer; it plays again by itself. */
  holding: boolean;
  /** Seconds of buffer the hold waits for. */
  goal: number | null;
  /** The viewer paused or played: no automatic resume. */
  cancel: () => void;
}

/**
 * When playback runs dry, the browser plays again the moment a fraction of a
 * second is in, then stalls again: a stutter every few seconds on a slow
 * link. Instead, this holds playback until a few seconds are buffered (more
 * after repeated stalls), so pauses are fewer and playback after them steady.
 * Off in Watch Together, whose clock decides when everyone plays.
 */
export function useRebuffer(video: HTMLVideoElement | null, enabled: boolean): Rebuffer {
  const [goal, setGoal] = useState<number | null>(null);
  const lastStall = useRef(0);

  const cancel = useCallback(() => setGoal(null), []);

  useEffect(() => {
    if (!video || !enabled) return;
    let started = false;
    const onPlaying = () => {
      started = true;
    };
    const onWaiting = () => {
      // Only a stall mid-playback: not the first load, not a seek (it has its own wait).
      if (!started || video.seeking || video.paused || video.readyState === HTMLMediaElement.HAVE_NOTHING || video.ended) return;
      const now = performance.now();
      const repeat = now - lastStall.current < REPEAT_WINDOW_MS;
      lastStall.current = now;
      const remaining = video.duration - video.currentTime;
      if (!(remaining > 1)) return;
      video.pause();
      setGoal(Math.min(repeat ? REPEAT_GOAL_SECONDS : GOAL_SECONDS, Math.floor(remaining)));
    };
    video.addEventListener("playing", onPlaying);
    video.addEventListener("waiting", onWaiting);
    return () => {
      video.removeEventListener("playing", onPlaying);
      video.removeEventListener("waiting", onWaiting);
    };
  }, [video, enabled]);

  useEffect(() => {
    if (!video || goal === null) return;
    const check = () => {
      const ahead = bufferedEnd(video) - video.currentTime;
      const toEnd = bufferedEnd(video) >= video.duration - 0.5;
      if (ahead >= goal || toEnd) {
        setGoal(null);
        void video.play().catch(() => {});
      }
    };
    const timer = window.setInterval(check, CHECK_MS);
    // A seek moves the playhead somewhere else entirely: the browser's own wait takes over.
    const onSeeking = () => setGoal(null);
    video.addEventListener("seeking", onSeeking);
    return () => {
      window.clearInterval(timer);
      video.removeEventListener("seeking", onSeeking);
    };
  }, [video, goal]);

  return { holding: goal !== null, goal, cancel };
}
