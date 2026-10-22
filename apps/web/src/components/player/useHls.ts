import { useEffect, useRef } from "react";

/**
 * Plays a server HLS stream (`playback.kind === "hls"`) in `video`. hls.js
 * (loaded on first use) drives Media Source Extensions wherever they exist;
 * a browser without them but with native HLS (iPhone Safari) gets the
 * playlist as its `src`. Each mount is one playback session on the server,
 * so its seeks restart only its own conversion.
 *
 * `startAt` (seconds) is read once: the server starts converting there, and
 * the player opens there.
 */
export function useHls(video: HTMLVideoElement | null, url: string | undefined, startAt: number | undefined, onFailed: () => void) {
  const failed = useRef(onFailed);
  failed.current = onFailed;
  const start = useRef(startAt);

  useEffect(() => {
    if (!video || !url) return;
    const at = start.current;
    const playlist = `${url}?session=${sessionId()}${at ? `&start=${at}` : ""}`;
    let cancelled = false;
    let destroy: (() => void) | undefined;

    const native = () => {
      const onError = () => video.error?.code !== MediaError.MEDIA_ERR_ABORTED && failed.current();
      video.addEventListener("error", onError);
      video.src = playlist;
      destroy = () => video.removeEventListener("error", onError);
    };

    if (typeof MediaSource === "undefined" && video.canPlayType("application/vnd.apple.mpegurl")) {
      native();
    } else {
      void import("hls.js/light").then(({ default: Hls }) => {
        if (cancelled) return;
        if (!Hls.isSupported()) {
          if (video.canPlayType("application/vnd.apple.mpegurl")) native();
          else failed.current();
          return;
        }
        // A segment after a long seek can take a few seconds while the server restarts its conversion.
        const patient = {
          maxTimeToFirstByteMs: 60_000,
          maxLoadTimeMs: 120_000,
          timeoutRetry: { maxNumRetry: 2, retryDelayMs: 0, maxRetryDelayMs: 0 },
          errorRetry: { maxNumRetry: 3, retryDelayMs: 1_000, maxRetryDelayMs: 8_000 },
        };
        const hls = new Hls({
          startPosition: at ?? -1,
          maxBufferLength: 30,
          maxMaxBufferLength: 120,
          backBufferLength: 90,
          fragLoadPolicy: { default: patient },
        });
        let recovered = false;
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (!data.fatal) return;
          console.warn("Playback stopped:", data.details, data.error?.message ?? "");
          if (data.type === Hls.ErrorTypes.MEDIA_ERROR && !recovered) {
            recovered = true;
            hls.recoverMediaError();
          } else {
            failed.current();
          }
        });
        hls.loadSource(playlist);
        hls.attachMedia(video);
        destroy = () => hls.destroy();
      });
    }
    return () => {
      cancelled = true;
      destroy?.();
    };
  }, [video, url]);
}

/** 128 random bits as hex. `crypto.randomUUID` needs a secure context, which a LAN address isn't. */
function sessionId(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}
