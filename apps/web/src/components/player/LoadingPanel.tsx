import { useEffect, useRef, useState, type RefObject } from "react";
import type { HlsInfo } from "./useHls";

/** How often the panel reads the video and the loader. */
const TICK_MS = 250;
/** How quickly the measured rates follow a change: the weight of each new reading. */
const SMOOTHING = 0.25;
/** A segment with no answer for this long is waiting on the server, not the network. */
const SERVER_WAIT_MS = 700;

/** One reading of what the player is loading and how fast. */
export interface LoadStats {
  /** Seconds buffered ahead of the playhead. */
  ahead: number;
  /** Seconds of video arriving per second; `null` until measured. Below 1, playback outruns loading. */
  fill: number | null;
  /** Bytes per second arriving, measured or estimated from `fill`. */
  byteRate: number | null;
  /** The HLS segment on its way, with how long since it was asked for. */
  fragment: (NonNullable<HlsInfo["fragment"]> & { waitedMs: number }) | null;
  /** Seconds since loading began. */
  elapsed: number;
}

/** Bit/s of the video playing, to turn seconds of buffer into bytes and back. */
export function useLoadStats(video: HTMLVideoElement | null, hls: RefObject<HlsInfo> | null, bitrate: number | null, active: boolean): LoadStats | null {
  const [stats, setStats] = useState<LoadStats | null>(null);
  const last = useRef<{ at: number; end: number; loaded: number; fill: number | null; byteRate: number | null; since: number } | null>(null);

  useEffect(() => {
    if (!video || !active) {
      last.current = null;
      setStats(null);
      return;
    }
    const read = () => {
      const now = performance.now();
      const end = bufferedEnd(video);
      const ahead = Math.max(0, end - video.currentTime);
      const info = hls?.current;
      const fragment = info?.fragment ?? null;
      const loaded = fragment?.loaded ?? 0;
      const prev = last.current;
      let fill = prev?.fill ?? null;
      let byteRate = prev?.byteRate ?? null;
      if (prev) {
        const dt = (now - prev.at) / 1000;
        const grew = end - prev.end;
        // A jump (a seek, a new range) is not loading speed. HLS measures whole segments instead (below).
        if (!info && dt > 0 && grew >= 0 && grew < 30) fill = blend(fill, grew / dt);
        // Bytes of the segment on its way, where the loader reports them.
        if (dt > 0 && fragment && loaded >= prev.loaded && loaded > 0) byteRate = blend(byteRate, (loaded - prev.loaded) / dt);
      }
      if (info) {
        // Recent segments' pace, held down by a segment taking longer than that pace allows.
        const waited = fragment ? (now - fragment.requested) / 1000 : 0;
        const bound = fragment && waited > 1 ? fragment.duration / waited : Infinity;
        // Before any segment is in: the share of this one arrived so far, once bytes flow.
        const partial = fragment && fragment.total > 0 && fragment.loaded > 0 && waited > 0.5 ? ((fragment.loaded / fragment.total) * fragment.duration) / waited : null;
        fill = info.fill === null ? partial : Math.min(info.fill, bound);
      }
      const rate = info?.bitrate ?? bitrate;
      if (byteRate === null && fill !== null && rate) byteRate = (fill * rate) / 8;
      if (byteRate === null && info?.bandwidth) byteRate = info.bandwidth / 8;
      const since = prev?.since ?? now;
      last.current = { at: now, end, loaded, fill, byteRate, since };
      setStats({
        ahead,
        fill,
        byteRate,
        fragment: fragment && { ...fragment, loaded: fragment.loaded, total: fragment.total, waitedMs: now - fragment.requested },
        elapsed: (now - since) / 1000,
      });
    };
    read();
    const timer = window.setInterval(read, TICK_MS);
    return () => window.clearInterval(timer);
  }, [video, hls, bitrate, active]);

  return stats;
}

function blend(old: number | null, next: number): number {
  return old === null ? next : old + (next - old) * SMOOTHING;
}

/** The end of the buffered range the playhead is in (or about to enter), else the playhead. */
export function bufferedEnd(video: HTMLVideoElement): number {
  const t = video.currentTime;
  for (let i = 0; i < video.buffered.length; i++) {
    if (video.buffered.start(i) <= t + 0.5 && video.buffered.end(i) >= t) return video.buffered.end(i);
  }
  return t;
}

export interface LoadingPanelProps {
  /** `start`: the first frame; `rebuffer`: playback ran dry. */
  phase: "start" | "rebuffer";
  stats: LoadStats | null;
  /** Seconds of buffer playback waits for, when it waits for more than the browser's minimum. */
  goal: number | null;
  /** What is playing: `"1080p copy · H.264"`, `"Original · 4K · HEVC Main 10 · HDR10"`. */
  source: string | null;
  /** How it reaches the screen: `"Direct play"`, `"Converting audio"`. */
  mode: string | null;
  /** Bit/s of what is playing. */
  bitrate: number | null;
  /** Said under the figures: a lighter copy to switch to, or one on its way. */
  hint: string | null;
}

/**
 * What the player waits for, in figures: what is loading, how fast, how much
 * is in hand and when playback (re)starts. Shown in place of a bare spinner.
 */
export function LoadingPanel({ phase, stats, goal, source, mode, bitrate, hint }: LoadingPanelProps) {
  const ahead = stats?.ahead ?? 0;
  const fragment = stats?.fragment ?? null;
  const serverBusy = fragment !== null && fragment.loaded === 0 && fragment.waitedMs > SERVER_WAIT_MS;
  const eta = estimate(stats, goal);
  const share = goal ? Math.min(1, ahead / goal) : fragment && fragment.total > 0 ? fragment.loaded / fragment.total : null;
  const slow = stats?.fill != null && stats.fill < 0.95 && stats.elapsed > 4;

  return (
    <div className="loading-panel" role="status" aria-live="polite">
      <div className="loading-head">
        <span className="loading-spinner" aria-hidden />
        <span className="loading-title">{phase === "start" ? "Loading" : "Buffering"}</span>
        <span className="loading-eta">{eta !== null ? (eta < 1 ? "ready in a moment" : `ready in ${clock(eta)}`) : stats ? `${clock(stats.elapsed)} so far` : ""}</span>
      </div>
      <div className="loading-bar" data-indeterminate={share === null} aria-hidden>
        <span style={share === null ? undefined : { width: `${share * 100}%` }} />
      </div>
      <dl className="loading-figures">
        {source && <Row label="Playing" value={bitrate ? `${source} · ${mbit(bitrate)}` : source} />}
        {mode && <Row label="Stream" value={mode} />}
        {fragment && (
          <Row
            label="Loading"
            value={
              fragment.number < 0
                ? "Stream header"
                : `Segment ${fragment.number + 1}${fragment.count ? ` of ${fragment.count}` : ""} · ${fragment.total > 0 ? `${mb(fragment.loaded)} of ${mb(fragment.total)}` : serverBusy ? `server preparing it, ${clock(fragment.waitedMs / 1000)}` : "asking"}`
            }
          />
        )}
        <Row
          label="Speed"
          value={stats?.byteRate ? `${mb(stats.byteRate)}/s · ${mbit(stats.byteRate * 8)}${stats.fill != null ? ` · ${stats.fill.toFixed(1)}× playback speed` : ""}` : "waiting for the first bytes"}
        />
        <Row label="Buffer" value={goal ? `${ahead.toFixed(1)} s of ${goal} s` : `${ahead.toFixed(1)} s ahead`} />
      </dl>
      {slow && bitrate !== null && (
        <p className="loading-hint" data-tone="warn">
          This video arrives slower than it plays, so playback will stop to catch up.{hint ? ` ${hint}` : ""}
        </p>
      )}
      {!slow && hint && <p className="loading-hint">{hint}</p>}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="loading-row">
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}

/** Seconds until there is enough to play: the buffer still missing over the rate it fills, or the segment's bytes left over the byte rate. */
function estimate(stats: LoadStats | null, goal: number | null): number | null {
  if (!stats) return null;
  const want = goal ?? 2;
  if (stats.fill !== null && stats.fill > 0.05 && stats.ahead > 0) return Math.max(0, (want - stats.ahead) / stats.fill);
  const f = stats.fragment;
  if (f && f.total > 0 && stats.byteRate) return Math.max(0, (f.total - f.loaded) / stats.byteRate);
  return null;
}

function clock(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${s % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

function mb(bytes: number): string {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.round(bytes / 1e3)} KB`;
}

function mbit(bitsPerSecond: number): string {
  return `${(bitsPerSecond / 1e6).toFixed(bitsPerSecond < 1e7 ? 1 : 0)} Mbit/s`;
}

export { clock as durationText };
