import * as Ph from "@phosphor-icons/react";
import type { LibraryKind } from "@nuvora/core";
import { opticalWeight } from "./Icon";

/**
 * The preset library icons, in the order the picker shows them. The names
 * match the server's list; it rejects anything else. Phosphor, like the rest
 * of the icon set.
 */
export const LIBRARY_ICONS: { name: string; label: string; Glyph: Ph.Icon }[] = [
  { name: "film", label: "Film", Glyph: Ph.FilmSlateIcon },
  { name: "tv", label: "TV", Glyph: Ph.TelevisionIcon },
  { name: "anime", label: "Anime", Glyph: Ph.FlowerIcon },
  { name: "sparkles", label: "Sparkles", Glyph: Ph.SparkleIcon },
  { name: "kids", label: "Kids", Glyph: Ph.BalloonIcon },
  { name: "documentary", label: "Documentary", Glyph: Ph.CompassIcon },
  { name: "music", label: "Music", Glyph: Ph.MusicNotesIcon },
  { name: "concert", label: "Concerts", Glyph: Ph.MicrophoneStageIcon },
  { name: "sports", label: "Sports", Glyph: Ph.SoccerBallIcon },
  { name: "star", label: "Favorites", Glyph: Ph.StarIcon },
  { name: "heart", label: "Romance", Glyph: Ph.HeartIcon },
  { name: "globe", label: "World", Glyph: Ph.GlobeHemisphereWestIcon },
  { name: "popcorn", label: "Popcorn", Glyph: Ph.PopcornIcon },
  { name: "ghost", label: "Horror", Glyph: Ph.GhostIcon },
  { name: "rocket", label: "Sci-fi", Glyph: Ph.RocketLaunchIcon },
  { name: "book", label: "Stories", Glyph: Ph.BookOpenIcon },
  { name: "camera", label: "Home videos", Glyph: Ph.VideoCameraIcon },
  { name: "family", label: "Family", Glyph: Ph.UsersThreeIcon },
  { name: "trophy", label: "Awards", Glyph: Ph.TrophyIcon },
  { name: "laugh", label: "Comedy", Glyph: Ph.SmileyIcon },
  { name: "sword", label: "Action", Glyph: Ph.SwordIcon },
  { name: "leaf", label: "Nature", Glyph: Ph.LeafIcon },
];

const BY_NAME = new Map(LIBRARY_ICONS.map((i) => [i.name, i]));

export function defaultLibraryIcon(kind: LibraryKind): string {
  return kind === "shows" ? "tv" : "film";
}

/** A library's icon: its chosen preset, else its kind's default. */
export function LibraryIcon({ icon, kind, size = 20, className }: { icon?: string | null; kind: LibraryKind; size?: number; className?: string }) {
  const { Glyph } = (icon && BY_NAME.get(icon)) || BY_NAME.get(defaultLibraryIcon(kind))!;
  return <Glyph size={size} weight={opticalWeight(size)} aria-hidden focusable={false} className={className} />;
}

/** The preset icons as a radio grid. */
export function IconPicker({ value, kind, onChange }: { value: string | null; kind: LibraryKind; onChange: (icon: string) => void }) {
  const current = value ?? defaultLibraryIcon(kind);
  return (
    <fieldset className="icon-picker">
      <legend>Icon</legend>
      <div className="icon-grid">
        {LIBRARY_ICONS.map((i) => (
          <label key={i.name} className="icon-option" data-checked={current === i.name} title={i.label}>
            <input type="radio" name="library-icon" value={i.name} checked={current === i.name} onChange={() => onChange(i.name)} className="sr-only" />
            <LibraryIcon icon={i.name} kind={kind} size={22} />
            <span className="sr-only">{i.label}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
