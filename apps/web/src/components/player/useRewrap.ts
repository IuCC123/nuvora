import { useEffect, useRef } from "react";

/**
 * Plays a Matroska or WebM file this browser decodes but can't open by
 * rewrapping it here (see `platform/rewrap`), so it still plays straight from
 * the file. The code loads on first use, like hls.js.
 *
 * `startAt` (seconds) is read once: the rewrap starts at the keyframe before it.
 */
export function useRewrap(video: HTMLVideoElement | null, url: string | undefined, startAt: number | undefined, onFailed: () => void) {
  const failed = useRef(onFailed);
  failed.current = onFailed;
  const start = useRef(startAt);

  useEffect(() => {
    if (!video || !url) return;
    let cancelled = false;
    let stop: (() => void) | undefined;
    void import("../../platform/rewrap")
      .then(({ rewrap }) => rewrap(video, url, start.current ?? 0, () => failed.current()))
      .then(
        (release) => {
          if (cancelled) release();
          else stop = release;
        },
        (error: unknown) => {
          if (cancelled) return;
          console.warn("Can't rewrap this file here:", error);
          failed.current();
        },
      );
    return () => {
      cancelled = true;
      stop?.();
    };
  }, [video, url]);
}
