/**
 * Watch-progress reporting, shared by every player so the TV apps keep the
 * web's cadence. The player tells the reporter what happened (playing,
 * paused, seeked, leaving); the reporter decides when to send. The server owns
 * every rule about what a position means (too early, in progress, finished).
 */

import type { ProgressRequest } from "./client";

export interface PlaybackPosition {
  positionMs: number;
  durationMs: number;
}

/** The platform's timers, injected so tests drive the clock. */
export interface Timers<H = unknown> {
  setTimeout(fn: () => void, ms: number): H;
  clearTimeout(handle: H): void;
}

export interface ProgressReporterOptions<H> {
  fileId: number;
  /** Where playback is now; `null` while the duration is still unknown. */
  read: () => PlaybackPosition | null;
  send: (report: ProgressRequest) => Promise<unknown> | void;
  timers: Timers<H>;
  /** While playing, a report this often. */
  intervalMs?: number;
  /** A report this long after the last seek, so a drag across the scrubber sends once. */
  settleMs?: number;
}

export interface ProgressReporter {
  playing(): void;
  paused(): void;
  seeked(): void;
  /** Report now through another transport (the web's page-hide beacon) and keep going. */
  flush(via: (report: ProgressRequest) => void): void;
  /** The final report. Resolves once it is sent; later calls do nothing. */
  leave(): Promise<void>;
}

export const PROGRESS_INTERVAL_MS = 10_000;
export const PROGRESS_SETTLE_MS = 1_000;

/** Reports within a second of the last one carry no news (a pause right after a periodic report). */
const SAME_POSITION_MS = 1_000;

export function createProgressReporter<H>(options: ProgressReporterOptions<H>): ProgressReporter {
  const { timers, intervalMs = PROGRESS_INTERVAL_MS, settleMs = PROGRESS_SETTLE_MS } = options;
  let phase: "paused" | "playing" | "left" = "paused";
  let cadence: { handle: H } | null = null;
  let settle: { handle: H } | null = null;
  let last: ProgressRequest | null = null;

  const stop = (timer: { handle: H } | null) => {
    if (timer) timers.clearTimeout(timer.handle);
    return null;
  };

  function report(): ProgressRequest | null {
    const at = options.read();
    if (!at || !Number.isFinite(at.durationMs) || at.durationMs <= 0 || !Number.isFinite(at.positionMs)) return null;
    const duration_ms = Math.round(at.durationMs);
    const next = { file_id: options.fileId, position_ms: Math.min(Math.max(0, Math.round(at.positionMs)), duration_ms), duration_ms };
    if (last && last.duration_ms === next.duration_ms && Math.abs(last.position_ms - next.position_ms) < SAME_POSITION_MS) return null;
    last = next;
    return next;
  }

  function sendNow(): Promise<void> {
    const next = report();
    if (!next) return Promise.resolve();
    // A lost report is not something the viewer can act on; the next one carries the position anyway.
    return Promise.resolve(options.send(next)).then(
      () => undefined,
      () => undefined,
    );
  }

  function schedule() {
    cadence = stop(cadence);
    cadence = {
      handle: timers.setTimeout(() => {
        cadence = null;
        void sendNow();
        schedule();
      }, intervalMs),
    };
  }

  return {
    playing() {
      if (phase !== "paused") return;
      phase = "playing";
      schedule();
    },
    paused() {
      if (phase !== "playing") return;
      phase = "paused";
      cadence = stop(cadence);
      settle = stop(settle);
      void sendNow();
    },
    seeked() {
      if (phase === "left") return;
      settle = stop(settle);
      settle = {
        handle: timers.setTimeout(() => {
          settle = null;
          void sendNow();
          if (phase === "playing") schedule();
        }, settleMs),
      };
    },
    flush(via) {
      if (phase === "left") return;
      const next = report();
      if (next) via(next);
    },
    leave() {
      if (phase === "left") return Promise.resolve();
      phase = "left";
      cadence = stop(cadence);
      settle = stop(settle);
      return sendNow();
    },
  };
}
