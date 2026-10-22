import { useQueries } from "@tanstack/react-query";
import { plural, type LibraryKind } from "@nuvora/core";
import { queries } from "../nuvora";
import { FilmIcon, TvIcon } from "./Icon";
import "../styles/library-type.css";

/** Each library type: its icon, its name, what its files look like, and what it holds. */
export const LIBRARY_TYPES: Record<LibraryKind, { icon: typeof FilmIcon; label: string; detail: string; noun: string }> = {
  movies: { icon: FilmIcon, label: "Movies", detail: "One film per file or folder", noun: "movie" },
  shows: { icon: TvIcon, label: "TV Shows", detail: "Series, with seasons and episodes", noun: "show" },
};

export function LibraryTypeIcon({ kind }: { kind: LibraryKind }) {
  const Icon = LIBRARY_TYPES[kind].icon;
  return <Icon className="library-icon" />;
}

/** Movies or TV Shows, as two cards with their icons. A radio group, so arrow keys switch between them. */
export function TypePicker({ value, onChange }: { value: LibraryKind; onChange: (kind: LibraryKind) => void }) {
  return (
    <fieldset className="type-picker">
      <legend>Type</legend>
      <div className="type-options">
        {(Object.keys(LIBRARY_TYPES) as LibraryKind[]).map((kind) => {
          const { icon: Icon, label, detail } = LIBRARY_TYPES[kind];
          return (
            <label key={kind} className="type-option" data-checked={value === kind}>
              <input type="radio" name="library-kind" value={kind} checked={value === kind} onChange={() => onChange(kind)} className="sr-only" />
              <Icon size={24} className="type-icon" />
              <span className="type-text">
                <span className="type-label">{label}</span>
                <span className="type-detail">{detail}</span>
              </span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

/**
 * What the chosen folders look like, from a quick count on the server: when
 * most videos are named like episodes the other type fits better, and the
 * banner offers to switch. Silent while counting and when the choice fits.
 */
export function TypeSuggestion({ paths, kind, onUse }: { paths: readonly string[]; kind: LibraryKind; onUse: (kind: LibraryKind) => void }) {
  const results = useQueries({ queries: paths.map((path) => queries.adminInspect(path)) });
  if (!paths.length || results.some((r) => !r.data)) return null;
  const videos = results.reduce((n, r) => n + r.data!.video_files, 0);
  const episodes = results.reduce((n, r) => n + r.data!.episode_like, 0);
  const truncated = results.some((r) => r.data!.truncated);
  if (videos === 0) return null;
  const suggested: LibraryKind = episodes * 2 > videos ? "shows" : "movies";
  if (suggested === kind) return null;
  const counted = `${truncated ? "Of the first " : ""}${plural(videos, "video")}${truncated ? " counted" : " in these folders"}`;
  return (
    <div className="type-suggestion" role="status">
      <LibraryTypeIcon kind={suggested} />
      <p>
        <strong>{LIBRARY_TYPES[suggested].label} fits these folders better.</strong>{" "}
        {suggested === "shows"
          ? `${counted}, ${episodes.toLocaleString()} are named like episodes (S01E02 or "- 02").`
          : `${counted}, only ${episodes.toLocaleString()} ${episodes === 1 ? "is" : "are"} named like an episode.`}
      </p>
      <button type="button" className="btn btn-secondary btn-small" onClick={() => onUse(suggested)}>
        Use {LIBRARY_TYPES[suggested].label}
      </button>
    </div>
  );
}
