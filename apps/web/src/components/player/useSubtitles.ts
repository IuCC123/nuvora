import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  chooseSubtitle,
  nextSubtitle,
  parseVtt,
  preferredLanguage,
  rememberSubtitle,
  type Cue,
  type FontAttachment,
  type SubtitleTrack,
} from "@nuvora/core";
import { client } from "../../nuvora";
import { useUser } from "../../lib/session";
import { loadSubtitleChoice, saveSubtitleChoice, type SubtitleScope } from "../../lib/subtitleMemory";

/** How the selected track is drawn right now. */
export type SubtitleRender =
  | { kind: "off" }
  | { kind: "loading" }
  | { kind: "plain"; cues: Cue[] }
  | { kind: "styled"; track: SubtitleTrack; content: string; fonts: string[] };

/** A quiet line under the track list. Never drawn over the picture. */
export type SubtitleNote = "loading" | "plain-fallback" | "failed" | null;

export interface Subtitles {
  tracks: readonly SubtitleTrack[];
  selected: SubtitleTrack | null;
  render: SubtitleRender;
  note: SubtitleNote;
  /** The viewer's choice; remembered for this show or movie. */
  select: (track: SubtitleTrack | null) => void;
  /** C: off, or back on with the last track (or the best one for the viewer's language). */
  toggle: () => SubtitleTrack | null;
  /** Shift+C: the next track, then Off. */
  cycle: () => SubtitleTrack | null;
  /** The styled renderer drew its first frame, or gave up. */
  styledReady: (track: SubtitleTrack) => void;
  styledFailed: (track: SubtitleTrack) => void;
}

const text = (url: string) => async ({ signal }: { signal: AbortSignal }) => {
  const res = await fetch(url, { signal, credentials: "same-origin" });
  if (!res.ok) throw new Error(`${url} -> ${res.status}`);
  return res.text();
};

/**
 * The player's subtitle state: which track is on (chosen by `chooseSubtitle`
 * until the viewer picks), and the text it needs. A styled track that can't be
 * drawn falls back to its WebVTT conversion for the rest of the session.
 */
export function useSubtitles(fileId: number, tracks: readonly SubtitleTrack[], audio: string | null, scope: SubtitleScope): Subtitles {
  const user = useUser();
  const [remembered] = useState(() => (user ? loadSubtitleChoice(user.id, scope) : null));
  const mode = user?.subtitle_mode ?? "auto";
  const language = preferredLanguage(user?.subtitle_language, navigator.language);
  const auto = useMemo(() => chooseSubtitle(tracks, audio, { mode, language }, remembered), [tracks, audio, mode, language, remembered]);

  const [picked, setPicked] = useState<{ index: number | null }>();
  const selected = picked === undefined ? auto : (tracks.find((t) => t.index === picked.index) ?? null);
  const lastOn = useRef<SubtitleTrack | null>(null);
  if (selected) lastOn.current = selected;

  const [styled, setStyled] = useState<Record<number, "ready" | "failed">>({});
  const styledState = selected ? styled[selected.index] : undefined;
  const wantStyled = selected?.format === "ass" && styledState !== "failed";

  const vtt = useQuery({
    queryKey: ["subtitle", fileId, selected?.index, "vtt"],
    queryFn: text(client.subtitleUrl(fileId, selected?.index ?? 0, "vtt")),
    enabled: !!selected && !wantStyled,
    staleTime: Infinity,
    select: parseVtt,
  });
  const ass = useQuery({
    queryKey: ["subtitle", fileId, selected?.index, "ass"],
    queryFn: text(client.subtitleUrl(fileId, selected?.index ?? 0, "ass")),
    enabled: wantStyled,
    staleTime: Infinity,
  });
  const fonts = useQuery({
    queryKey: ["fonts", fileId],
    queryFn: ({ signal }) => client.fonts(fileId, signal),
    enabled: wantStyled,
    staleTime: Infinity,
    // Absolute: the renderer fetches them from its worker, where a path would resolve against the worker's own URL.
    select: (list: FontAttachment[]) => list.map((f) => new URL(client.resolve(f.url), location.href).href),
  });

  const fail = useCallback((track: SubtitleTrack) => setStyled((s) => ({ ...s, [track.index]: "failed" })), []);
  const ready = useCallback((track: SubtitleTrack) => setStyled((s) => (s[track.index] ? s : { ...s, [track.index]: "ready" })), []);
  // A styled track whose ASS text can't be fetched still has its VTT conversion to try.
  const assFailed = wantStyled && ass.isError ? selected : null;
  useEffect(() => {
    if (assFailed) fail(assFailed);
  }, [assFailed, fail]);

  let render: SubtitleRender;
  let note: SubtitleNote = null;
  if (!selected) {
    render = { kind: "off" };
  } else if (wantStyled) {
    const fontsSettled = fonts.isSuccess || fonts.isError;
    render = ass.data !== undefined && fontsSettled ? { kind: "styled", track: selected, content: ass.data, fonts: fonts.data ?? NO_FONTS } : { kind: "loading" };
    if (styledState !== "ready") note = "loading";
  } else if (vtt.data) {
    render = { kind: "plain", cues: vtt.data };
    if (styledState === "failed") note = "plain-fallback";
  } else {
    render = { kind: "loading" };
    note = vtt.isError ? "failed" : "loading";
  }

  const select = useCallback(
    (track: SubtitleTrack | null) => {
      setPicked({ index: track?.index ?? null });
      if (user) saveSubtitleChoice(user.id, scope, rememberSubtitle(track));
    },
    [user, scope],
  );
  const toggle = () => {
    const next = selected
      ? null
      : (lastOn.current ?? chooseSubtitle(tracks, audio, { mode: "always", language }, null) ?? tracks.find((t) => t.renderable) ?? null);
    select(next);
    return next;
  };
  const cycle = () => {
    const next = nextSubtitle(tracks, selected);
    select(next);
    return next;
  };

  return { tracks, selected, render, note, select, toggle, cycle, styledReady: ready, styledFailed: fail };
}

const NO_FONTS: string[] = [];
