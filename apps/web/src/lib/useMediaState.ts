import { useEffect, useState } from "react";

/** What the controls draw, read from the `<video>` on its own events. Times are seconds, as the element reports them. */
export interface MediaState {
  paused: boolean;
  time: number;
  /** `NaN` until metadata loads. */
  duration: number;
  buffered: readonly (readonly [start: number, end: number])[];
  volume: number;
  muted: boolean;
  /** Seeking, or out of data to play: the spinner shows once this has held for a moment. */
  starved: boolean;
  pip: boolean;
}

const INITIAL: MediaState = { paused: true, time: 0, duration: Number.NaN, buffered: [], volume: 1, muted: false, starved: true, pip: false };

const EVENTS = [
  "play",
  "pause",
  "playing",
  "waiting",
  "seeking",
  "seeked",
  "timeupdate",
  "durationchange",
  "loadedmetadata",
  "loadeddata",
  "canplay",
  "progress",
  "volumechange",
  "ended",
  "enterpictureinpicture",
  "leavepictureinpicture",
] as const;

export function useMediaState(video: HTMLVideoElement | null): MediaState {
  const [state, setState] = useState(INITIAL);
  useEffect(() => {
    if (!video) return;
    const read = () =>
      setState({
        paused: video.paused,
        time: video.currentTime,
        duration: video.duration,
        buffered: Array.from({ length: video.buffered.length }, (_, i) => [video.buffered.start(i), video.buffered.end(i)] as const),
        volume: video.volume,
        muted: video.muted,
        starved: !video.error && (video.seeking || (!video.paused && video.readyState < HTMLMediaElement.HAVE_FUTURE_DATA) || video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA),
        pip: document.pictureInPictureElement === video,
      });
    read();
    for (const e of EVENTS) video.addEventListener(e, read);
    return () => {
      for (const e of EVENTS) video.removeEventListener(e, read);
    };
  }, [video]);
  return state;
}
