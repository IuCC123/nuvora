import { useSyncExternalStore } from "react";
import { ServerClock, type LiveCommand, type LiveEvent, type PartyInvite, type PartyView } from "@nuvora/core";

export interface PartySnapshot {
  /** The live connection is up. */
  connected: boolean;
  party: PartyView | null;
  /** Invitations to me, oldest first. */
  invites: PartyInvite[];
}

const CLOSED: PartySnapshot = { connected: false, party: null, invites: [] };
/** Reconnect delays, in seconds; the last repeats. */
const BACKOFF = [1, 2, 5, 10];
const PING_EVERY_MS = 20_000;

/**
 * The live connection to `/api/live` while someone is signed in: Watch
 * Together's party and invitations as a store React reads with
 * `useParty()`, the commands that go back, and the server's clock. One per
 * tab, reconnecting on its own.
 */
class PartyLink {
  private socket: WebSocket | null = null;
  private wanted = false;
  private attempts = 0;
  private reconnect: number | undefined;
  private pinger: number | undefined;
  private clock = new ServerClock();
  private snapshot: PartySnapshot = CLOSED;
  private readonly listeners = new Set<() => void>();
  private readonly events = new Set<(event: LiveEvent) => void>();

  start() {
    if (this.wanted) return;
    this.wanted = true;
    this.open();
  }

  stop() {
    this.wanted = false;
    window.clearTimeout(this.reconnect);
    window.clearInterval(this.pinger);
    this.socket?.close();
    this.socket = null;
    this.set(CLOSED);
  }

  send(command: LiveCommand) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(command));
  }

  /** The server's clock, in unix ms. */
  serverNow(): number {
    return this.clock.now(Date.now());
  }

  /** Apply an answer from the REST calls (join, invite) without waiting for its live echo. */
  setParty(party: PartyView | null) {
    this.set({ ...this.snapshot, party });
  }

  setInvites(invites: PartyInvite[]) {
    this.set({ ...this.snapshot, invites });
  }

  /** Every event as it arrives, after the store has taken it in. */
  onEvent(listener: (event: LiveEvent) => void): () => void {
    this.events.add(listener);
    return () => this.events.delete(listener);
  }

  readonly subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  readonly getSnapshot = () => this.snapshot;

  private open() {
    const socket = new WebSocket(`${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/live`);
    this.socket = socket;
    socket.onopen = () => {
      this.attempts = 0;
      this.ping();
      window.clearInterval(this.pinger);
      this.pinger = window.setInterval(() => this.ping(), PING_EVERY_MS);
    };
    socket.onmessage = (message) => {
      if (typeof message.data !== "string") return;
      try {
        this.handle(JSON.parse(message.data) as LiveEvent);
      } catch {
        // A frame this build doesn't understand.
      }
    };
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      window.clearInterval(this.pinger);
      this.set({ ...this.snapshot, connected: false });
      if (!this.wanted) return;
      const delay = BACKOFF[Math.min(this.attempts, BACKOFF.length - 1)]!;
      this.attempts += 1;
      this.reconnect = window.setTimeout(() => this.open(), delay * 1000);
    };
  }

  private ping() {
    this.send({ type: "ping", client_time_ms: Date.now() });
  }

  private handle(event: LiveEvent) {
    switch (event.type) {
      case "hello":
        this.clock.seed(event.server_time_ms, Date.now());
        this.set({ connected: true, party: event.status.party ?? null, invites: event.status.invites });
        break;
      case "pong":
        this.clock.sample(event.client_time_ms, event.server_time_ms, Date.now());
        break;
      case "party":
        this.set({ ...this.snapshot, party: event.party });
        break;
      case "party_left":
        if (this.snapshot.party?.id === event.party_id) this.set({ ...this.snapshot, party: null });
        break;
      case "invites":
        this.set({ ...this.snapshot, invites: event.invites });
        break;
    }
    for (const listener of this.events) listener(event);
  }

  private set(next: PartySnapshot) {
    this.snapshot = next;
    for (const listener of this.listeners) listener();
  }
}

export const partyLink = new PartyLink();

export function useParty(): PartySnapshot {
  return useSyncExternalStore(partyLink.subscribe, partyLink.getSnapshot);
}
