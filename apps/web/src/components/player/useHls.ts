import { useEffect, useRef, type RefObject } from "react";

/** What hls.js is fetching, for the loading panel. Mutated in place; read it on a timer. */
export interface HlsInfo {
  /** The segment being fetched now. `loaded` grows as its bytes arrive; `total` is 0 until the server answers. */
  fragment: { number: number; count: number; duration: number; loaded: number; total: number; requested: number } | null;
  /** hls.js's running estimate of the connection, bit/s. */
  bandwidth: number | null;
  /** Average bit/s of the segments fetched so far. */
  bitrate: number | null;
  /** Seconds of video per second of fetching, over recent segments, the server's wait included. Below 1, playback outruns the stream. */
  fill: number | null;
  /** Media segments fetched so far: how much `fill` has to go on. */
  fragments: number;
}

/** Tries after a network error before playback gives up: 2, 4, 8, 16 s, then every 30 s, about 2½ minutes in all. */
const NETWORK_RETRIES = 8;

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
export function useHls(video: HTMLVideoElement | null, url: string | undefined, startAt: number | undefined, onFailed: () => void): RefObject<HlsInfo> {
  const failed = useRef(onFailed);
  failed.current = onFailed;
  const start = useRef(startAt);
  const info = useRef<HlsInfo>({ fragment: null, bandwidth: null, bitrate: null, fill: null, fragments: 0 });

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
          // A minute ahead rides out a slow patch; the back buffer stays short,
          // since it counts against the browser's memory for a high-bitrate stream.
          maxBufferLength: 60,
          maxMaxBufferLength: 300,
          backBufferLength: 30,
          // The next segment is asked for while the current one appends.
          startFragPrefetch: true,
          fragLoadPolicy: { default: patient },
        });
        let bytes = 0;
        let seconds = 0;
        hls.on(Hls.Events.FRAG_LOADING, (_event, { frag }) => {
          const stats = frag.stats;
          info.current.fragment = {
            number: frag.sn === "initSegment" ? -1 : frag.sn,
            count: hls.levels[0]?.details?.fragments.length ?? 0,
            duration: frag.duration,
            get loaded() {
              return stats.loaded;
            },
            get total() {
              return stats.total;
            },
            requested: performance.now(),
          };
        });
        let networkRetries = 0;
        let retry: ReturnType<typeof setTimeout> | undefined;
        hls.on(Hls.Events.FRAG_LOADED, (_event, { frag }) => {
          networkRetries = 0;
          if (frag.sn !== "initSegment" && frag.duration > 0) {
            info.current.fragments += 1;
            bytes += frag.stats.loaded;
            seconds += frag.duration;
            info.current.bitrate = (bytes * 8) / seconds;
            const took = (frag.stats.loading.end - frag.stats.loading.start) / 1000;
            if (took > 0) {
              const fill = frag.duration / took;
              const old = info.current.fill;
              info.current.fill = old === null ? fill : old + (fill - old) * 0.4;
            }
          }
          if (frag.sn !== "initSegment") info.current.bandwidth = Number.isFinite(hls.bandwidthEstimate) ? hls.bandwidthEstimate : null;
          info.current.fragment = null;
        });
        let recovered = false;
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (!data.fatal) return;
          // A dropped connection or a segment that ran out of retries isn't the end: what's
          // buffered keeps playing while hls.js tries again, more patiently each time.
          if (data.type === Hls.ErrorTypes.NETWORK_ERROR && networkRetries < NETWORK_RETRIES) {
            networkRetries += 1;
            console.warn("Network trouble, trying again:", data.details);
            clearTimeout(retry);
            retry = setTimeout(
              () => (data.details.startsWith("manifest") ? hls.loadSource(playlist) : hls.startLoad()),
              Math.min(30_000, 1_000 * 2 ** networkRetries),
            );
            return;
          }
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
        destroy = () => {
          clearTimeout(retry);
          hls.destroy();
        };
      });
    }
    return () => {
      cancelled = true;
      destroy?.();
    };
  }, [video, url]);
  return info;
}

/** 128 random bits as hex. `crypto.randomUUID` needs a secure context, which a LAN address isn't. */
function sessionId(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}
