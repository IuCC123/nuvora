import { useEffect, useRef, useState } from 'react';
import type { VideoPlayer } from '@amazon-devices/react-native-w3cmedia';
import { formatClock, partyCorrection, partyPosition, type PartyMember, type PartyPlayback } from '@nuvora/core';
import { useLive, type LiveLink } from '../live';
import type { VideoState } from './engine';

/** How often a playing video is checked against the party's clock. */
const CHECK_MS = 500;
const NOTE_MS = 3_000;

export interface PartyPlayer {
  /** In a party, and it is playing this file: the party's clock drives the video. */
  active: boolean;
  /** Everyone else in the party. */
  others: PartyMember[];
  /** Whole seconds until everyone starts together, while counting down. */
  countdown: number | null;
  /** What someone else just did: `"Bo paused"`. */
  note: string | null;
  play(positionMs: number): void;
  pause(positionMs: number): void;
  seek(positionMs: number): void;
  /** Play or pause happened on the video; one the party didn't ask for goes to the party. */
  heard(playing: boolean, positionMs: number): void;
}

/**
 * Keeps the video with the party. Opening the player while in a party starts
 * this file for everyone; after that every play, pause and seek goes through
 * the party and comes back for everyone (this player included) at the same
 * moment. Small drift is closed by playing a little faster or slower, large
 * drift by jumping.
 */
export function usePartyPlayer(link: LiveLink, video: React.MutableRefObject<VideoPlayer | null>, state: VideoState, me: number, itemId: number, fileId: number, startMs: number): PartyPlayer {
  const snapshot = useLive(link);
  const party = snapshot.party;
  const playback = party?.playback ?? null;
  // Without the live connection the player is on its own: it can't hear the party, nor tell it anything.
  const active = snapshot.connected && !!playback && playback.item_id === itemId && playback.file_id === fileId;
  const [countdown, setCountdown] = useState<number | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const latest = useRef<PartyPlayback | null>(null);
  latest.current = active ? playback : null;
  const stateRef = useRef(state);
  stateRef.current = state;
  /** What the party last asked of the video, to tell its own changes from the viewer's. */
  const wanted = useRef<'playing' | 'paused' | null>(null);

  // The app knows what's open, so a start for this file doesn't open it again.
  useEffect(() => {
    const open = { itemId, fileId };
    link.playing = open;
    return () => {
      if (link.playing === open) link.playing = null;
    };
  }, [link, itemId, fileId]);

  // Opened while in a party on something else: this is what the party watches now.
  // Decided once, on opening: when the party later moves on, this player is the one leaving.
  const [starts] = useState(() => snapshot.connected && !!party && !active);
  useEffect(() => {
    if (starts) link.sendStart(itemId, fileId, startMs);
  }, [starts, link, itemId, fileId, startMs]);

  useEffect(() => {
    const v = video.current;
    if (!active) {
      // Out of the party: back to normal speed.
      if (v) v.playbackRate = 1;
      setCountdown(null);
      wanted.current = null;
      return;
    }
    const apply = () => {
      const p = latest.current;
      const v = video.current;
      if (!p || !v) return;
      const now = link.serverNow();
      const duration = v.duration > 0 && Number.isFinite(v.duration) ? v.duration : null;
      const target = duration ? Math.min(partyPosition(p, now), duration) : partyPosition(p, now);
      const current = v.currentTime;
      if (p.playing && now < p.at_ms) {
        // Counting down to a shared start: hold at the start position.
        wanted.current = 'paused';
        if (!v.paused) v.pause();
        if (Math.abs(current - target) > 0.3) v.currentTime = target;
        // A new start counts down on screen; a resume or jump just holds for its short lead.
        setCountdown(p.action === 'start' ? Math.ceil((p.at_ms - now) / 1000) : null);
        return;
      }
      setCountdown(null);
      if (!p.playing) {
        v.playbackRate = 1;
        wanted.current = 'paused';
        if (!v.paused) v.pause();
        if (Math.abs(current - target) > 0.5) v.currentTime = target;
        return;
      }
      wanted.current = 'playing';
      if (stateRef.current.ended) return;
      // Still loading after a jump: let it catch up before judging the drift.
      if (stateRef.current.buffering) {
        if (v.paused) void v.play();
        return;
      }
      const correction = partyCorrection(current, target);
      if (correction.kind === 'seek') {
        v.playbackRate = 1;
        v.currentTime = correction.to;
      } else {
        v.playbackRate = correction.kind === 'nudge' ? correction.rate : 1;
      }
      if (v.paused) void v.play();
    };
    apply();
    const timer = setInterval(apply, CHECK_MS);
    return () => clearInterval(timer);
  }, [active, playback?.seq, link, video]);

  // Say what someone else did.
  useEffect(() => {
    const p = latest.current;
    if (!p || !p.by || p.by.id === me) return;
    const who = p.by.display_name;
    setNote(
      p.action === 'pause' ? `${who} paused` : p.action === 'play' ? `${who} pressed play` : p.action === 'seek' ? `${who} jumped to ${formatClock(p.position_ms)}` : `${who} started this`,
    );
    const timer = setTimeout(() => setNote(null), NOTE_MS);
    return () => clearTimeout(timer);
  }, [playback?.seq, me]);

  return {
    active,
    others: party?.members.filter((m) => m.id !== me) ?? [],
    countdown,
    note,
    play: (ms) => link.sendPlayback('play', ms),
    pause: (ms) => link.sendPlayback('pause', ms),
    seek: (ms) => link.sendPlayback('seek', ms),
    heard: (playing, ms) => {
      if (!active || wanted.current === null) return;
      if (playing && wanted.current === 'paused') link.sendPlayback('play', ms);
      if (!playing && wanted.current === 'playing' && !stateRef.current.ended) link.sendPlayback('pause', ms);
    },
  };
}
