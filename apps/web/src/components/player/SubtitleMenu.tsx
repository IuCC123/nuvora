import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { subtitleDescription, subtitleLabel, type SubtitleTrack } from "@nuvora/core";
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

/**
 * The CC button and its menu: Off and every track, then the subtitle look.
 * Choices are radio groups, so arrows move through them and the choice
 * applies at once. A pointer pick of a track closes the menu.
 */
export function SubtitleMenu({ subtitles, style, onStyle, onOpenChange }: SubtitleMenuProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const openedByKeyboard = useRef(false);
  const id = useId();
  const { tracks, selected, note } = subtitles;

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
  const describe = (track: SubtitleTrack) => subtitleLabel(track, languageName);
  return (
    <div className="subs-anchor" ref={root} onKeyDown={onKeyDown}>
      <button
        ref={button}
        type="button"
        className="player-btn"
        aria-label={selected ? `Subtitles: ${subtitleDescription(describe(selected))}` : "Subtitles: Off"}
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
        <SubtitlesIcon on={!!selected} />
      </button>
      {open && (
        <div ref={menu} id={`${id}-menu`} className="subs-menu" role="dialog" aria-label="Subtitles" tabIndex={-1}>
          <fieldset className="subs-tracks">
            <legend className="subs-heading">Subtitles</legend>
            <div className="subs-track-list">
              <TrackOption name={`${id}-track`} checked={!selected} onPick={(pointer) => pick(null, pointer)}>
                <span className="subs-track-name">Off</span>
              </TrackOption>
              {tracks.map((track) => {
                const label = describe(track);
                return (
                  <TrackOption
                    key={track.index}
                    name={`${id}-track`}
                    checked={selected?.index === track.index}
                    disabled={label.needsConversion}
                    onPick={(pointer) => pick(track, pointer)}
                  >
                    <span className="subs-track-main">
                      <span className="subs-track-name">{label.name}</span>
                      {label.tags.map((tag) => (
                        <span key={tag} className="subs-tag">
                          {tag}
                        </span>
                      ))}
                    </span>
                    {(label.title || label.needsConversion) && (
                      <span className="subs-track-title">{[label.title, label.needsConversion && "Needs conversion"].filter(Boolean).join(" · ")}</span>
                    )}
                  </TrackOption>
                );
              })}
            </div>
          </fieldset>
          {note && (
            <p className="subs-note" role="status">
              {NOTES[note]}
            </p>
          )}
          <div className="subs-style">
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
          <label key={option} className="subs-segment" data-checked={value === option || undefined}>
            <input type="radio" className="subs-radio" name={name} checked={value === option} onChange={() => onChange(option)} />
            <span>{labels[option]}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
