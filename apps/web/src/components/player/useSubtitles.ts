import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  chooseSubtitle,
  findLanguage,
  languageCode,
  nextSubtitle,
  parseVtt,
  preferredLanguage,
  rememberSubtitle,
  type Cue,
  type FontAttachment,
  type OnlineSubtitleResult,
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

/** Subtitles found online in the viewer's language. */
export interface OnlineSearch {
  /** The language searched, by name; `null` when the viewer's language is unknown. */
  language: string | null;
  searching: boolean;
  results: readonly OnlineSubtitleResult[];
  /** Why there are none to offer (no key, title unknown, searches used up). */
  unavailable: string | null;
}

export interface Subtitles {
  tracks: readonly SubtitleTrack[];
  /** The file's track that's on, if one is. */
  selected: SubtitleTrack | null;
  /** The online subtitle that's on, if one is. */
  online: OnlineSubtitleResult | null;
  onlineSearch: OnlineSearch;
  render: SubtitleRender;
  note: SubtitleNote;
  /** Seconds each line shows later than the subtitle says; per choice, for this playback. */
  delay: number;
  setDelay: (seconds: number) => void;
  /** The viewer's choice; remembered for this show or movie. */
  select: (track: SubtitleTrack | null) => void;
  selectOnline: (result: OnlineSubtitleResult) => void;
  /** Look online now (the menu opened), rather than only when the file has nothing in the viewer's language. */
  searchOnline: () => void;
  /** C: off, or back on with the last track (or the best one for the viewer's language). */
  toggle: () => SubtitleTrack | OnlineSubtitleResult | null;
  /** Shift+C: the next track, then Off. */
  cycle: () => SubtitleTrack | null;
  /** The styled renderer drew its first frame, or gave up. */
  styledReady: (track: SubtitleTrack) => void;
  styledFailed: (track: SubtitleTrack) => void;
}

type Pick = { kind: "track"; index: number | null } | { kind: "online"; result: OnlineSubtitleResult };

const text = (url: string) => async ({ signal }: { signal: AbortSignal }) => {
  const res = await fetch(url, { signal, credentials: "same-origin" });
  if (!res.ok) throw new Error(`${url} -> ${res.status}`);
  return res.text();
};

/**
 * The player's subtitle state: which track is on (chosen by `chooseSubtitle`
 * until the viewer picks), and the text it needs. A styled track that can't be
 * drawn falls back to its WebVTT conversion for the rest of the session.
 *
 * When the viewer wants subtitles (always, or for audio in another language)
 * and the file has none in their language that plays here, the best online
 * match comes on by itself: the same release as the file first, so it's
 * timed for it. `fileId` is `null` for a torrent stream, which has no tracks
 * to fetch.
 */
export function useSubtitles(fileId: number | null, tracks: readonly SubtitleTrack[], audio: string | null, scope: SubtitleScope): Subtitles {
  const user = useUser();
  const [remembered] = useState(() => (user ? loadSubtitleChoice(user.id, scope) : null));
  const mode = user?.subtitle_mode ?? "auto";
  const language = preferredLanguage(user?.subtitle_language, navigator.language);
  const auto = useMemo(() => chooseSubtitle(tracks, audio, { mode, language }, remembered), [tracks, audio, mode, language, remembered]);

  // Online, in the viewer's language: wanted when the file has nothing to show them.
  const found = findLanguage(language);
  const wantsSubtitles = mode === "always" || (mode === "auto" && languageCode(audio) !== null && languageCode(audio) !== language);
  const hasOwnLanguage = tracks.some((t) => t.renderable && languageCode(t.language) === language);
  const [picked, setPicked] = useState<Pick>();
  const autoOnline = picked === undefined && remembered?.kind !== "off" && wantsSubtitles && !hasOwnLanguage;
  const [searchWanted, setSearchWanted] = useState(false);
  const search = useQuery({
    queryKey: ["subtitles-online", fileId, found?.short],
    queryFn: ({ signal }) => client.onlineSubtitles(fileId ?? 0, found?.short ?? "", signal),
    enabled: fileId !== null && !!found?.short && (searchWanted || autoOnline),
    staleTime: 60 * 60_000,
    retry: false,
  });
  const results = search.data?.results ?? NO_RESULTS;

  const online = picked?.kind === "online" ? picked.result : autoOnline ? (results[0] ?? null) : null;
  const selected = online
    ? null
    : picked === undefined
      ? auto
      : picked.kind === "track"
        ? (tracks.find((t) => t.index === picked.index) ?? null)
        : null;
  const lastOn = useRef<SubtitleTrack | OnlineSubtitleResult | null>(null);
  if (selected ?? online) lastOn.current = selected ?? online;

  const [styled, setStyled] = useState<Record<number, "ready" | "failed">>({});
  const styledState = selected ? styled[selected.index] : undefined;
  const wantStyled = selected?.format === "ass" && styledState !== "failed";

  const vtt = useQuery({
    queryKey: ["subtitle", fileId, selected?.index, "vtt"],
    queryFn: text(client.subtitleUrl(fileId ?? 0, selected?.index ?? 0, "vtt")),
    enabled: fileId !== null && !!selected && !wantStyled,
    staleTime: Infinity,
    select: parseVtt,
  });
  const onlineVtt = useQuery({
    queryKey: ["subtitle-online", fileId, online?.id],
    queryFn: text(client.onlineSubtitleUrl(fileId ?? 0, online?.id ?? "")),
    enabled: fileId !== null && !!online,
    staleTime: Infinity,
    select: parseVtt,
  });
  const ass = useQuery({
    queryKey: ["subtitle", fileId, selected?.index, "ass"],
    queryFn: text(client.subtitleUrl(fileId ?? 0, selected?.index ?? 0, "ass")),
    enabled: fileId !== null && wantStyled,
    staleTime: Infinity,
  });
  const fonts = useQuery({
    queryKey: ["fonts", fileId],
    queryFn: ({ signal }) => client.fonts(fileId ?? 0, signal),
    enabled: fileId !== null && wantStyled,
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
  if (online) {
    render = onlineVtt.data ? { kind: "plain", cues: onlineVtt.data } : { kind: "loading" };
    if (!onlineVtt.data) note = onlineVtt.isError ? "failed" : "loading";
  } else if (!selected) {
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

  // A delay belongs to the subtitle it was set for: another one is timed differently.
  const choiceKey = online ? `online:${online.id}` : selected ? `track:${selected.index}` : "off";
  const [delayFor, setDelayFor] = useState({ key: "off", seconds: 0 });
  const delay = delayFor.key === choiceKey ? delayFor.seconds : 0;
  const setDelay = (seconds: number) => setDelayFor({ key: choiceKey, seconds: Math.round(seconds * 10) / 10 });

  const select = (track: SubtitleTrack | null) => {
    setPicked({ kind: "track", index: track?.index ?? null });
    if (user) saveSubtitleChoice(user.id, scope, rememberSubtitle(track));
    if (track && track.index === selected?.index && vtt.isError) void vtt.refetch();
  };
  const selectOnline = (result: OnlineSubtitleResult) => {
    setPicked({ kind: "online", result });
    if (result.id === online?.id && onlineVtt.isError) void onlineVtt.refetch();
  };
  const toggle = () => {
    if (selected || online) {
      select(null);
      return null;
    }
    const last = lastOn.current;
    if (last && "same_release" in last) {
      selectOnline(last);
      return last;
    }
    const next = last ?? chooseSubtitle(tracks, audio, { mode: "always", language }, null) ?? tracks.find((t) => t.renderable) ?? null;
    if (!next && results[0]) {
      selectOnline(results[0]);
      return results[0];
    }
    select(next);
    return next;
  };
  const cycle = () => {
    const next = nextSubtitle(tracks, selected);
    select(next);
    return next;
  };

  const searchOnline = useCallback(() => setSearchWanted(true), []);
  const onlineSearch: OnlineSearch = {
    language: found?.name ?? null,
    searching: search.isFetching,
    results,
    unavailable: search.data?.unavailable ?? (search.isError ? "The subtitle search didn't answer. Try again in a minute." : null),
  };
  return {
    tracks,
    selected,
    online,
    onlineSearch,
    render,
    note,
    delay,
    setDelay,
    select,
    selectOnline,
    searchOnline,
    toggle,
    cycle,
    styledReady: ready,
    styledFailed: fail,
  };
}

const NO_FONTS: string[] = [];
const NO_RESULTS: OnlineSubtitleResult[] = [];
