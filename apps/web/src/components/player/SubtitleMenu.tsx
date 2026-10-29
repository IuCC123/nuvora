import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { subtitleDescription, subtitleLabel, type OnlineSubtitleResult, type SubtitleTrack } from "@nuvora/core";
import { CheckIcon, SubtitlesIcon } from "../Icon";
import { languageName } from "../../lib/languageName";
import { SUBTITLE_BACKGROUNDS, SUBTITLE_SIZES, type SubtitleStyle } from "../../lib/subtitleMemory";
import type { Subtitles } from "./useSubtitles";

interface SubtitleMenuProps {
  subtitles: Subtitles;
  style: SubtitleStyle;
  onStyle: (style: SubtitleStyle) => void;
  /** The menu holds the controls on screen while it's open. */
  onOpenChange: (open: boolean) => void;
}

const SIZE_LABELS: Record<SubtitleStyle["size"], string> = { small: "Small", medium: "Medium", large: "Large" };
const BACKGROUND_LABELS: Record<SubtitleStyle["background"], string> = { none: "None", shadow: "Shadow", box: "Box" };

const NOTES = {
  loading: "Loading subtitles…",
  "plain-fallback": "Styled subtitles couldn't load, so this track shows as plain text.",
  failed: "These subtitles couldn't be loaded. Pick the track again to retry.",
} as const;

/** Keys a menu control handles itself; the player's shortcuts must not see them. */
const MENU_KEYS = new Set([" ", "Enter", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "Tab"]);

/** Online results shown before "Show more". */
const ONLINE_SHOWN = 4;
const DELAY_STEP = 0.5;

/**
 * The CC button and its menu: Off, the file's text tracks and subtitles found
 * online in one scrolling list, then timing and the subtitle look, which stay
 * in view. Image tracks (Blu-ray PGS) fold into one row: this player can't
 * draw them over a stream. Choices are radio groups, so arrows move through
 * them and the choice applies at once. A pointer pick closes the menu.
 */
export function SubtitleMenu({ subtitles, style, onStyle, onOpenChange }: SubtitleMenuProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const openedByKeyboard = useRef(false);
  const id = useId();
  const { tracks, selected, online, onlineSearch, note } = subtitles;
  const [allOnline, setAllOnline] = useState(false);
  const textTracks = tracks.filter((t) => t.renderable);
  const imageTracks = tracks.filter((t) => !t.renderable);
  const shownOnline = allOnline ? onlineSearch.results : onlineSearch.results.slice(0, ONLINE_SHOWN);
  const on = !!(selected ?? online);
  const searchOnline = subtitles.searchOnline;
  useEffect(() => {
    if (open) searchOnline();
  }, [open, searchOnline]);

  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  useEffect(() => onOpenChange(open), [open, onOpenChange]);
  useEffect(() => {
    if (!open) return;
    // From the keyboard, focus lands on the current track; from a pointer, on the menu itself, so Escape still closes it.
    const target = openedByKeyboard.current ? root.current?.querySelector<HTMLInputElement>(`input[name="${id}-track"]:checked`) : menu.current;
    target?.focus();
    const outside = (e: PointerEvent) => !root.current?.contains(e.target as Node) && close(false);
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open, id]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close(true);
    } else if (MENU_KEYS.has(e.key) && e.target !== button.current) {
      e.stopPropagation();
    }
  };

  const pick = (track: SubtitleTrack | null, pointer: boolean) => {
    subtitles.select(track);
    if (pointer) close(false);
  };
  const pickOnline = (result: OnlineSubtitleResult, pointer: boolean) => {
    subtitles.selectOnline(result);
    if (pointer) close(false);
  };
  const describe = (track: SubtitleTrack) => subtitleLabel(track, languageName);
  return (
    <div className="subs-anchor" ref={root} onKeyDown={onKeyDown}>
      <button
        ref={button}
        type="button"
        className="player-btn"
        aria-label={selected ? `Subtitles: ${subtitleDescription(describe(selected))}` : online ? `Subtitles: ${online.label}, online` : "Subtitles: Off"}
        aria-expanded={open}
        aria-controls={open ? `${id}-menu` : undefined}
        data-tip={open ? undefined : "Subtitles · C"}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => {
          openedByKeyboard.current = e.detail === 0;
          if (open) close(openedByKeyboard.current);
          else setOpen(true);
        }}
      >
        <SubtitlesIcon on={on} />
      </button>
      {open && (
        <div ref={menu} id={`${id}-menu`} className="subs-menu" role="dialog" aria-label="Subtitles" tabIndex={-1}>
          <fieldset className="subs-tracks">
            <legend className="subs-heading">Subtitles</legend>
            <div className="subs-track-list">
              <TrackOption name={`${id}-track`} checked={!on} onPick={(pointer) => pick(null, pointer)}>
                <span className="subs-track-name">Off</span>
              </TrackOption>
              {textTracks.map((track) => {
                const label = describe(track);
                return (
                  <TrackOption key={track.index} name={`${id}-track`} checked={selected?.index === track.index} onPick={(pointer) => pick(track, pointer)}>
                    <span className="subs-track-main">
                      <span className="subs-track-name">{label.name}</span>
                      {label.tags.map((tag) => (
                        <span key={tag} className="subs-tag">
                          {tag}
                        </span>
                      ))}
                    </span>
                    {label.title && <span className="subs-track-title">{label.title}</span>}
                  </TrackOption>
                );
              })}
              {onlineSearch.language && (
                <div className="subs-group" role="group" aria-label={`Online, ${onlineSearch.language}`}>
                  <p className="subs-group-heading" aria-hidden>
                    Online · {onlineSearch.language}
                  </p>
                  {shownOnline.map((result) => (
                    <TrackOption key={result.id} name={`${id}-track`} checked={online?.id === result.id} onPick={(pointer) => pickOnline(result, pointer)}>
                      <span className="subs-track-main">
                        <span className="subs-track-name">{result.label}</span>
                        {result.same_release && <span className="subs-tag subs-tag-match">Timed for this file</span>}
                        {result.hearing_impaired && <span className="subs-tag">SDH</span>}
                      </span>
                      {result.release && (
                        <span className="subs-track-title" title={result.release}>
                          {result.release}
                        </span>
                      )}
                    </TrackOption>
                  ))}
                  {!allOnline && onlineSearch.results.length > ONLINE_SHOWN && (
                    <button type="button" className="subs-more" onClick={() => setAllOnline(true)}>
                      Show {onlineSearch.results.length - ONLINE_SHOWN} more
                    </button>
                  )}
                  {onlineSearch.searching && onlineSearch.results.length === 0 && <p className="subs-group-note">Searching…</p>}
                  {!onlineSearch.searching && onlineSearch.unavailable && <p className="subs-group-note">{onlineSearch.unavailable}</p>}
                  {!onlineSearch.searching && !onlineSearch.unavailable && onlineSearch.results.length === 0 && (
                    <p className="subs-group-note">Nothing found online.</p>
                  )}
                </div>
              )}
              {imageTracks.length > 0 && (
                <details className="subs-images">
                  <summary>
                    {imageTracks.length} image {imageTracks.length === 1 ? "track" : "tracks"} this player can't show
                  </summary>
                  <p className="subs-group-note">
                    Blu-ray subtitles are pictures, which this player can't draw over a stream. Subtitles found online cover most of them.
                  </p>
                  <ul className="subs-image-list">
                    {imageTracks.map((track) => {
                      const label = describe(track);
                      return (
                        <li key={track.index}>
                          {label.name}
                          {label.tags.length > 0 && ` · ${label.tags.join(", ")}`}
                        </li>
                      );
                    })}
                  </ul>
                </details>
              )}
            </div>
          </fieldset>
          {note && (
            <p className="subs-note" role="status">
              {NOTES[note]}
            </p>
          )}
          <div className="subs-style">
            <div className="subs-timing" role="group" aria-label="Timing">
              <span className="subs-style-label">Timing</span>
              <div className="subs-timing-controls">
                <button
                  type="button"
                  className="subs-step"
                  disabled={!on}
                  aria-label={`Subtitles ${DELAY_STEP} seconds earlier`}
                  onClick={() => subtitles.setDelay(subtitles.delay - DELAY_STEP)}
                >
                  −
                </button>
                <output className="subs-delay" aria-live="polite">
                  {subtitles.delay === 0 ? "On time" : `${subtitles.delay > 0 ? "+" : "−"}${Math.abs(subtitles.delay).toFixed(1)} s`}
                </output>
                <button
                  type="button"
                  className="subs-step"
                  disabled={!on}
                  aria-label={`Subtitles ${DELAY_STEP} seconds later`}
                  onClick={() => subtitles.setDelay(subtitles.delay + DELAY_STEP)}
                >
                  +
                </button>
                {subtitles.delay !== 0 && (
                  <button type="button" className="subs-reset" onClick={() => subtitles.setDelay(0)}>
                    Reset
                  </button>
                )}
              </div>
            </div>
            <Segmented
              legend="Size"
              name={`${id}-size`}
              options={SUBTITLE_SIZES}
              labels={SIZE_LABELS}
              value={style.size}
              onChange={(size) => onStyle({ ...style, size })}
            />
            <Segmented
              legend="Background"
              name={`${id}-background`}
              options={SUBTITLE_BACKGROUNDS}
              labels={BACKGROUND_LABELS}
              value={style.background}
              onChange={(background) => onStyle({ ...style, background })}
            />
          </div>
        </div>
      )}
    </div>
  );
}

interface TrackOptionProps {
  name: string;
  checked: boolean;
  disabled?: boolean;
  /** `pointer` is true for a click, false for the keyboard's arrows and Space. */
  onPick: (pointer: boolean) => void;
  children: ReactNode;
}

function TrackOption({ name, checked, disabled, onPick, children }: TrackOptionProps) {
  const pointer = useRef(false);
  return (
    <label className="subs-track" data-disabled={disabled || undefined} onPointerDown={() => (pointer.current = true)}>
      <input
        type="radio"
        className="subs-radio"
        name={name}
        checked={checked}
        disabled={disabled}
        onChange={() => {
          onPick(pointer.current);
          pointer.current = false;
        }}
        // A checked radio fires no change; clicking the current track again still picks it, which retries a failed load.
        onClick={() => {
          if (!checked) return;
          onPick(pointer.current);
          pointer.current = false;
        }}
        onKeyDown={() => (pointer.current = false)}
      />
      <span className="subs-check" aria-hidden>
        {checked && <CheckIcon size={16} />}
      </span>
      <span className="subs-track-text">{children}</span>
    </label>
  );
}

interface SegmentedProps<T extends string> {
  legend: string;
  name: string;
  options: readonly T[];
  labels: Record<T, string>;
  value: T;
  onChange: (value: T) => void;
}

function Segmented<T extends string>({ legend, name, options, labels, value, onChange }: SegmentedProps<T>) {
  return (
    <fieldset className="subs-segmented">
      <legend className="subs-style-label">{legend}</legend>
      <div className="subs-segments">
        {options.map((option) => (
          <label key={option} className="subs-segment">
            <input type="radio" className="subs-radio" name={name} checked={value === option} onChange={() => onChange(option)} />
            <span>{labels[option]}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
