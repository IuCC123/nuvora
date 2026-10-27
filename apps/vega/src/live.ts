import { useSyncExternalStore } from 'react';
import { ServerClock, type LiveCommand, type LiveEvent, type PartyInvite, type PartyView } from '@nuvora/core';

/** What the live connection knows. */
export interface LiveSnapshot {
  /** The connection is up. */
  connected: boolean;
  party: PartyView | null;
  /** Invitations to me, oldest first. */
  invites: PartyInvite[];
}

/** Something the app acts on, beyond the snapshot changing. */
export type LiveNotice =
  /** Someone started something for everyone (a new `start`, not one already seen). */
  { kind: 'started'; party: PartyView };

const EMPTY: LiveSnapshot = { connected: false, party: null, invites: [] };
/** Reconnect delays, in ms; the last repeats. */
const BACKOFF = [1_000, 2_000, 5_000, 10_000];
const PING_EVERY_MS = 20_000;

/** A monotonic clock where there is one, so a change to the TV's wall clock doesn't move the party. */
const localNow = () => (globalThis as { performance?: { now(): number } }).performance?.now() ?? Date.now();

/**
 * The live connection to `/api/live` while signed in: Watch Together's
 * party and invitations, the commands that go back, and the server's clock.
 * Reconnects on its own.
 */
export class LiveLink {
  private snapshot: LiveSnapshot = EMPTY;
  private readonly listeners = new Set<() => void>();
  private readonly noticeListeners = new Set<(notice: LiveNotice) => void>();
  private socket: WebSocket | null = null;
  private attempts = 0;
  private reconnect: ReturnType<typeof setTimeout> | null = null;
  private pinger: ReturnType<typeof setInterval> | null = null;
  private clock = new ServerClock();
  private running = false;
  /** The file the player has open, so a start for it doesn't open it again. */
  playing: { itemId: number; fileId: number } | null = null;

  constructor(
    private readonly baseUrl: string,
    private readonly token: string,
  ) {}

  start() {
    if (this.running) return;
    this.running = true;
    this.open();
  }

  stop() {
    this.running = false;
    if (this.reconnect) clearTimeout(this.reconnect);
    if (this.pinger) clearInterval(this.pinger);
    this.socket?.close();
    this.socket = null;
    this.set(EMPTY);
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  getSnapshot = () => this.snapshot;

  onNotice(listener: (notice: LiveNotice) => void) {
    this.noticeListeners.add(listener);
    return () => void this.noticeListeners.delete(listener);
  }

  send(command: LiveCommand) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(command));
  }

  sendPlayback(type: 'play' | 'pause' | 'seek', positionMs: number) {
    this.send({ type, position_ms: Math.max(0, Math.round(positionMs)) });
  }

  sendStart(itemId: number, fileId: number, positionMs: number) {
    this.send({ type: 'start', item_id: itemId, file_id: fileId, position_ms: Math.max(0, Math.round(positionMs)) });
  }

  /** The server's clock, in unix ms. */
  serverNow() {
    return this.clock.now(localNow());
  }

  /** An answer from the REST calls (join, invite), without waiting for its live echo. */
  setParty(party: PartyView | null) {
    this.set({ ...this.snapshot, party });
  }

  setInvites(invites: PartyInvite[]) {
    this.set({ ...this.snapshot, invites });
  }

  private set(next: LiveSnapshot) {
    this.snapshot = next;
    for (const listener of this.listeners) listener();
  }

  private open() {
    if (!this.running) return;
    const url = this.baseUrl.replace(/^http/, 'ws') + '/api/live';
    const WS = WebSocket as unknown as new (url: string, protocols: undefined, options: { headers: Record<string, string> }) => WebSocket;
    const socket = new WS(url, undefined, { headers: { Authorization: `Bearer ${this.token}` } });
    this.socket = socket;
    const mine = () => this.socket === socket && this.running;
    socket.onopen = () => {
      if (!mine()) return;
      this.attempts = 0;
      if (this.pinger) clearInterval(this.pinger);
      this.ping();
      this.pinger = setInterval(() => this.ping(), PING_EVERY_MS);
    };
    socket.onmessage = (event: { data?: unknown }) => {
      const received = localNow();
      if (mine() && typeof event.data === 'string') this.handle(event.data, received);
    };
    const lost = () => {
      if (!mine()) return;
      this.socket = null;
      if (this.pinger) clearInterval(this.pinger);
      this.set({ ...this.snapshot, connected: false });
      const delay = BACKOFF[Math.min(this.attempts, BACKOFF.length - 1)]!;
      this.attempts++;
      this.reconnect = setTimeout(() => this.open(), delay);
    };
    socket.onclose = lost;
    socket.onerror = () => socket.close();
  }

  private ping() {
    this.send({ type: 'ping', client_time_ms: localNow() });
  }

  private handle(text: string, received: number) {
    let frame: LiveEvent;
    try {
      frame = JSON.parse(text) as LiveEvent;
    } catch {
      return;
    }
    switch (frame.type) {
      case 'hello':
        this.clock.seed(frame.server_time_ms, received);
        // Where things stand, not a change: nothing to follow.
        this.set({ connected: true, party: frame.status.party ?? null, invites: frame.status.invites });
        break;
      case 'pong':
        this.clock.sample(frame.client_time_ms, frame.server_time_ms, received);
        break;
      case 'party': {
        const before = this.snapshot.party?.id === frame.party.id ? this.snapshot.party.playback?.seq : undefined;
        this.set({ ...this.snapshot, party: frame.party });
        const playback = frame.party.playback;
        if (playback && playback.action === 'start' && playback.seq !== before) {
          for (const listener of this.noticeListeners) listener({ kind: 'started', party: frame.party });
        }
        break;
      }
      case 'party_left':
        if (this.snapshot.party?.id === frame.party_id) this.set({ ...this.snapshot, party: null });
        break;
      case 'invites':
        this.set({ ...this.snapshot, invites: frame.invites });
        break;
      // A frame this build doesn't understand: ignore it.
    }
  }
}

const NO_LINK: LiveSnapshot = EMPTY;
const noop = () => () => undefined;

/** The live snapshot, re-rendering when it changes; empty when signed out. */
export function useLive(link: LiveLink | null): LiveSnapshot {
  return useSyncExternalStore(link ? link.subscribe : noop, link ? link.getSnapshot : () => NO_LINK);
}
