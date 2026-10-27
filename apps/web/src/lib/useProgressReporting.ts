import { useEffect, useRef } from "react";
import { createProgressReporter, queryKeys, watchStateKeys, type ProgressRequest } from "@nuvora/core";
import { client, queryClient } from "../nuvora";

/** What playback reports to: a library file, or a torrent stream (kept for Continue Watching). */
export type ProgressTarget = { kind: "file"; itemId: number; fileId: number } | { kind: "stream"; id: string };

const timers = {
  setTimeout: (fn: () => void, ms: number) => window.setTimeout(fn, ms),
  clearTimeout: (handle: number) => window.clearTimeout(handle),
};

/**
 * Reports where `video` is through the shared reporter (`@nuvora/core`), plus
 * the one web-only path: when the tab is hidden or the page closes, a keepalive
 * request carries the position even if the page never comes back. Leaving the
 * player sends the last position, then refreshes home and the detail page.
 * `onReported` hears every position sent while the player is still open.
 * A stream reports to its own place; `null` reports nothing.
 */
export function useProgressReporting(video: HTMLVideoElement | null, target: ProgressTarget | null, onReported: (positionMs: number) => void) {
  const reported = useRef(onReported);
  reported.current = onReported;
  const itemId = target?.kind === "file" ? target.itemId : undefined;
  const fileId = target?.kind === "file" ? target.fileId : undefined;
  const streamId = target?.kind === "stream" ? target.id : undefined;
  useEffect(() => {
    if (!video || (itemId === undefined && streamId === undefined)) return;
    const send = (body: ProgressRequest, opts?: { keepalive: boolean }) =>
      streamId !== undefined ? client.admin.streamProgress(streamId, body, opts) : client.reportProgress(itemId!, body, opts);
    let leaving = false;
    const told = (body: ProgressRequest) => !leaving && reported.current(body.position_ms);
    const reporter = createProgressReporter({
      fileId: fileId ?? 0,
      read: () => ({ positionMs: video.currentTime * 1000, durationMs: video.duration * 1000 }),
      send: (body) => {
        told(body);
        return send(body);
      },
      timers,
    });
    const keepalive = (body: ProgressRequest) => {
      told(body);
      void send(body, { keepalive: true }).catch(() => {});
    };
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
      leaving = true;
      void reporter.leave().then(() => {
        const keys = itemId !== undefined ? watchStateKeys(itemId) : [queryKeys.watchedStreams];
        for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey });
      });
    };
  }, [video, itemId, fileId, streamId]);
}
