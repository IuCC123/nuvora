import { useEffect, useMemo, useState } from 'react';
import { activeCues, audioLanguage, chooseSubtitle, parseVtt, preferredLanguage, rememberSubtitle, type AudioTrack, type Cue, type SubtitleTrack } from '@nuvora/core';
import { deviceLanguage } from '../server';
import { useSignedIn } from '../state';
import { saveSubtitleChoice, subtitleChoice } from '../storage';

export interface Subtitles {
  /** The tracks the server can deliver as text. */
  tracks: readonly SubtitleTrack[];
  selected: SubtitleTrack | null;
  /** The viewer's pick, remembered for this show or movie. */
  select(track: SubtitleTrack | null): void;
  /** The lines to show at `positionMs`. */
  linesAt(positionMs: number): string[];
}

/**
 * The player's subtitles: the track chosen as the web and Fire TV apps choose
 * it (the viewer's last pick for this title, else the account setting), drawn
 * from the server's WebVTT. Styled (ASS) tracks come as their WebVTT conversion.
 */
export function useSubtitles(fileId: number, all: readonly SubtitleTrack[], audio: readonly AudioTrack[], scope: string): Subtitles {
  const { client, session } = useSignedIn();
  const user = session.user;
  const tracks = useMemo(() => all.filter((t) => t.renderable), [all]);
  const [selected, setSelected] = useState<SubtitleTrack | null>(() =>
    chooseSubtitle(
      tracks,
      audioLanguage(audio),
      { mode: user.subtitle_mode, language: preferredLanguage(user.subtitle_language, deviceLanguage()) },
      subtitleChoice(user.id, scope),
    ),
  );
  const [cues, setCues] = useState<{ index: number; cues: Cue[] } | null>(null);

  useEffect(() => {
    if (!selected) return;
    let current = true;
    fetch(client.subtitleUrl(fileId, selected.index, 'vtt'), { headers: { Authorization: `Bearer ${session.token}` } })
      .then((res) => (res.ok ? res.text() : Promise.reject(new Error(String(res.status)))))
      .then(
        (text) => current && setCues({ index: selected.index, cues: parseVtt(text) }),
        () => current && setCues({ index: selected.index, cues: [] }),
      );
    return () => {
      current = false;
    };
  }, [client, fileId, selected, session.token]);

  return {
    tracks,
    selected,
    select: (track) => {
      setSelected(track);
      saveSubtitleChoice(user.id, scope, rememberSubtitle(track));
    },
    linesAt: (positionMs) => {
      if (!selected || cues?.index !== selected.index) return [];
      return activeCues(cues.cues, positionMs / 1000).flatMap((cue) => cue.lines.map((runs) => runs.map((r) => r.text).join('')));
    },
  };
}
