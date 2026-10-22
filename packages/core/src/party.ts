import type { PartyPlayback } from "./client";

/**
 * Watch Together's shared clock, client side. The server describes playback
 * as "at server time `at_ms` the position is `position_ms`, playing or
 * paused"; every client works out where it should be from that and its own
 * estimate of the server's clock. Pure, so every app syncs the same way.
 */

/** Where the party is, in seconds, at server time `serverNowMs`. Before `at_ms` (a countdown) it is `position_ms`. */
export function partyPosition(playback: PartyPlayback, serverNowMs: number): number {
  const elapsed = playback.playing ? Math.max(0, serverNowMs - playback.at_ms) : 0;
  return (playback.position_ms + elapsed) / 1000;
}

/** Milliseconds until a scheduled start or resume; 0 when it has begun or the party is paused. */
export function partyStartsIn(playback: PartyPlayback, serverNowMs: number): number {
  return playback.playing ? Math.max(0, playback.at_ms - serverNowMs) : 0;
}

/** How a player should follow the party, given where it is now (seconds). */
export type PartyCorrection =
  /** Close enough: play at normal speed. */
  | { kind: "in_sync" }
  /** A little off: play slightly faster or slower until caught up, without a visible jump. */
  | { kind: "nudge"; rate: number }
  /** Too far off to nudge: jump to `to`. */
  | { kind: "seek"; to: number };

/** Under this many seconds apart counts as together. */
const IN_SYNC = 0.25;
/** Past this many seconds apart, jump instead of nudging. */
const SEEK_PAST = 2;

export function partyCorrection(current: number, target: number): PartyCorrection {
  const ahead = current - target;
  if (Math.abs(ahead) <= IN_SYNC) return { kind: "in_sync" };
  if (Math.abs(ahead) > SEEK_PAST) return { kind: "seek", to: target };
  return { kind: "nudge", rate: ahead > 0 ? 0.95 : 1.05 };
}

/**
 * Estimates the server clock from ping round trips: the offset from the
 * fastest round trip seen, which brackets the server's time most tightly.
 */
export class ServerClock {
  private best = Number.POSITIVE_INFINITY;
  private offset = 0;

  /** A `pong`: `sent` and `received` are local times around the round trip. */
  sample(sent: number, serverTime: number, received: number): void {
    const rtt = received - sent;
    if (rtt < 0 || rtt > this.best) return;
    this.best = rtt;
    this.offset = serverTime - (sent + rtt / 2);
  }

  /** A server time with no round trip to go by (the first `hello`). */
  seed(serverTime: number, received: number): void {
    if (this.best === Number.POSITIVE_INFINITY) this.offset = serverTime - received;
  }

  now(localNow: number): number {
    return localNow + this.offset;
  }
}
