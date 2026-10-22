import { useEffect } from "react";
import { createProgressReporter, watchStateKeys, type ProgressRequest } from "@nuvora/core";
import { client, queryClient } from "../nuvora";

const timers = {
  setTimeout: (fn: () => void, ms: number) => window.setTimeout(fn, ms),
  clearTimeout: (handle: number) => window.clearTimeout(handle),
};

/**
 * Reports where `video` is through the shared reporter (`@nuvora/core`), plus
 * the one web-only path: when the tab is hidden or the page closes, a keepalive
 * request carries the position even if the page never comes back. Leaving the
 * player sends the last position, then refreshes home and the detail page.
 */
export function useProgressReporting(video: HTMLVideoElement | null, itemId: number, fileId: number) {
  useEffect(() => {
    if (!video) return;
    const reporter = createProgressReporter({
      fileId,
      read: () => ({ positionMs: video.currentTime * 1000, durationMs: video.duration * 1000 }),
      send: (body) => client.reportProgress(itemId, body),
      timers,
    });
    const keepalive = (body: ProgressRequest) => void client.reportProgress(itemId, body, { keepalive: true }).catch(() => {});
    const onPlaying = () => reporter.playing();
    const onPause = () => reporter.paused();
    const onSeeked = () => reporter.seeked();
    const onHidden = () => document.visibilityState === "hidden" && reporter.flush(keepalive);
    const onPageHide = () => reporter.flush(keepalive);
    video.addEventListener("playing", onPlaying);
    video.addEventListener("pause", onPause);
    video.addEventListener("seeked", onSeeked);
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", onPageHide);
    return () => {
      video.removeEventListener("playing", onPlaying);
      video.removeEventListener("pause", onPause);
      video.removeEventListener("seeked", onSeeked);
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", onPageHide);
      void reporter.leave().then(() => {
        for (const queryKey of watchStateKeys(itemId)) void queryClient.invalidateQueries({ queryKey });
      });
    };
  }, [video, itemId, fileId]);
}
