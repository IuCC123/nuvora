import { useCallback, useEffect, useRef, useState } from "react";
import { formatClock, partyCorrection, partyPosition, type PartyMember, type PartyPlayback } from "@nuvora/core";
import { partyLink, useParty } from "../../lib/party";
import { useUser } from "../../lib/session";

/** How often a playing video is checked against the party's clock. */
const CHECK_MS = 500;
const NOTE_MS = 3_000;

export interface PartyPlayer {
  /** In a party, and it is playing this file: the party's clock drives the video. */
  active: boolean;
  /** In a party at all: the video waits for the party instead of starting on its own. */
  inParty: boolean;
  /** Everyone else in the party. */
  others: PartyMember[];
  /** Whole seconds until everyone starts together, while counting down. */
  countdown: number | null;
  /** What someone else just did: `"Bo paused"`. */
  note: string | null;
  /** The browser refused to start playing on its own; a press of Play fixes it. */
  blocked: boolean;
  play(position: number): void;
  pause(position: number): void;
  seek(position: number): void;
}

/**
 * Keeps `video` with the party. Opening the player while in a party starts
 * this file for everyone; after that every play, pause and seek goes through
 * the party and comes back for everyone (this player included) at the same
 * moment. Small drift is closed by playing a little faster or slower, large
 * drift by jumping. Without a library `file` (a torrent stream) the player
 * keeps to itself: the party can only watch what is in the library.
 */
export function usePartyPlayer(video: HTMLVideoElement | null, file: { itemId: number; fileId: number } | null, startAt: number | undefined): PartyPlayer {
  const me = useUser()?.id;
  const { party, connected } = useParty();
  const playback = party?.playback ?? null;
  const itemId = file?.itemId;
  const fileId = file?.fileId;
  // Without the live connection the player is on its own: it can't hear the party, nor tell it anything.
  const active = connected && !!playback && itemId !== undefined && playback.item_id === itemId && playback.file_id === fileId;
  const [countdown, setCountdown] = useState<number | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [blocked, setBlocked] = useState(false);
  const latest = useRef<PartyPlayback | null>(null);
  latest.current = active ? playback : null;

  // Opened while in a party on something else: this is what the party watches now.
  const started = useRef(false);
  useEffect(() => {
    if (itemId === undefined || fileId === undefined || started.current || !party || !connected || active) return;
    started.current = true;
    partyLink.send({ type: "start", item_id: itemId, file_id: fileId, position_ms: Math.round((startAt ?? 0) * 1000) });
  }, [party, connected, active, itemId, fileId, startAt]);

  const apply = useCallback(() => {
    const p = latest.current;
    if (!video || !p || video.readyState < HTMLMediaElement.HAVE_METADATA) return;
    const now = partyLink.serverNow();
    const target = Math.min(partyPosition(p, now), Number.isFinite(video.duration) ? video.duration : Infinity);
    if (p.playing && now < p.at_ms) {
      // Counting down to a shared start: hold at the start position.
      if (!video.paused) video.pause();
      if (!video.seeking && Math.abs(video.currentTime - target) > 0.3) video.currentTime = target;
      // A new start counts down on screen; a resume or jump just holds for its short lead.
      setCountdown(p.action === "start" ? Math.ceil((p.at_ms - now) / 1000) : null);
      return;
    }
    setCountdown(null);
    if (!p.playing) {
      video.playbackRate = 1;
      if (!video.paused) video.pause();
      if (!video.seeking && Math.abs(video.currentTime - target) > 0.5) video.currentTime = target;
      return;
    }
    // Still loading after a jump: let it catch up before judging the drift.
    if (video.seeking || video.readyState < HTMLMediaElement.HAVE_FUTURE_DATA) return;
    const correction = partyCorrection(video.currentTime, target);
    if (correction.kind === "seek") {
      video.playbackRate = 1;
      video.currentTime = correction.to;
    } else {
      video.playbackRate = correction.kind === "nudge" ? correction.rate : 1;
    }
    if (video.paused && !video.ended) {
      video.play().then(
        () => setBlocked(false),
        (e: unknown) => {
          if (e instanceof DOMException && e.name === "NotAllowedError") setBlocked(true);
        },
      );
    }
  }, [video]);

  // Every change, as it arrives; then on a steady beat.
  const seq = active ? playback.seq : null;
  useEffect(() => {
    if (seq === null) return;
    apply();
    const timer = window.setInterval(apply, CHECK_MS);
    return () => window.clearInterval(timer);
  }, [seq, apply]);
  useEffect(() => {
    if (!video || seq === null) return;
    video.addEventListener("loadedmetadata", apply);
    video.addEventListener("canplay", apply);
    return () => {
      video.removeEventListener("loadedmetadata", apply);
      video.removeEventListener("canplay", apply);
    };
  }, [video, seq, apply]);
  // Out of the party: back to normal speed.
  useEffect(() => {
    if (!active && video) video.playbackRate = 1;
  }, [active, video]);

  // Say what someone else did.
  const shown = useRef<number | null>(null);
  useEffect(() => {
    const p = latest.current;
    if (!p || shown.current === p.seq) return;
    shown.current = p.seq;
    if (!p.by || p.by.id === me) return;
    const who = p.by.display_name;
    const text =
      p.action === "pause"
        ? `${who} paused`
        : p.action === "play"
          ? `${who} pressed play`
          : p.action === "seek"
            ? `${who} jumped to ${formatClock(p.position_ms)}`
            : `${who} started this`;
    setNote(text);
    const timer = window.setTimeout(() => setNote(null), NOTE_MS);
    return () => window.clearTimeout(timer);
  }, [seq, me]);

  const send = useCallback((type: "play" | "pause" | "seek", position: number) => {
    partyLink.send({ type, position_ms: Math.max(0, Math.round(position * 1000)) });
  }, []);

  return {
    active,
    inParty: !!party && connected,
    others: party?.members.filter((m) => m.id !== me) ?? [],
    countdown: active ? countdown : null,
    note,
    blocked: active && blocked,
    play: (position) => {
      setBlocked(false);
      send("play", position);
    },
    pause: (position) => send("pause", position),
    seek: (position) => send("seek", position),
  };
}
