import type { ReactNode } from "react";
import type { LibraryKind } from "@nuvora/core";

/**
 * The preset library icons, in the order the picker shows them. The names
 * match the server's list; it rejects anything else. Same grid and stroke as
 * the rest of the icon set.
 */
export const LIBRARY_ICONS: { name: string; label: string; shape: ReactNode }[] = [
  { name: "film", label: "Film", shape: <><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M7 3v18M3 7.5h4M3 12h18M3 16.5h4M17 3v18M17 7.5h4M17 16.5h4" /></> },
  { name: "tv", label: "TV", shape: <><rect width="20" height="14" x="2" y="6" rx="2" /><path d="m16 2-4 4-4-4" /></> },
  { name: "anime", label: "Anime", shape: <><circle cx="12" cy="12" r="2.6" /><path d="M12 9.4a3.4 3.4 0 1 1 3.4-3.4M14.6 12a3.4 3.4 0 1 1 3.4 3.4M12 14.6a3.4 3.4 0 1 1-3.4 3.4M9.4 12A3.4 3.4 0 1 1 6 8.6" /></> },
  { name: "sparkles", label: "Sparkles", shape: <><path d="M11 3l1.9 5.1L18 10l-5.1 1.9L11 17l-1.9-5.1L4 10l5.1-1.9z" /><path d="M19 14.5l.75 2L22 17.25l-2.25.75L19 20l-.75-2L16 17.25l2.25-.75z" /></> },
  { name: "kids", label: "Kids", shape: <><path d="M12 15c3.3 0 6-3 6-6.7a6 6 0 0 0-12 0C6 12 8.7 15 12 15z" /><path d="M12 15v1.6M12 16.6c0 2.4-2.5 2.4-2.5 4.9" /></> },
  { name: "documentary", label: "Documentary", shape: <><circle cx="12" cy="12" r="9" /><path d="m15.5 8.5-2.2 4.8-4.8 2.2 2.2-4.8z" /></> },
  { name: "music", label: "Music", shape: <><path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" /></> },
  { name: "concert", label: "Concerts", shape: <><rect x="9" y="2" width="6" height="12" rx="3" /><path d="M19 10v1a7 7 0 0 1-14 0v-1M12 18v4M8 22h8" /></> },
  { name: "sports", label: "Sports", shape: <><circle cx="12" cy="12" r="9" /><path d="M12 3a9 9 0 0 0 0 18M5 6.5c3 1.6 4.5 3.6 4.5 5.5S8 15.9 5 17.5M19 6.5c-3 1.6-4.5 3.6-4.5 5.5s1.5 3.9 4.5 5.5" /></> },
  { name: "star", label: "Favorites", shape: <path d="M12 2.8l2.85 5.78 6.38.93-4.62 4.5 1.09 6.35L12 17.36l-5.7 3 1.09-6.35-4.62-4.5 6.38-.93z" /> },
  { name: "heart", label: "Romance", shape: <path d="M19 14c1.5-1.46 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7z" /> },
  { name: "globe", label: "World", shape: <><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></> },
  { name: "popcorn", label: "Popcorn", shape: <><path d="M6 9h12l-1.6 12H7.6z" /><path d="M10 9l.5 12M14 9l-.5 12" /><path d="M6.6 9A2.6 2.6 0 0 1 8 4.2a3 3 0 0 1 4-1.4 3 3 0 0 1 4 1.4A2.6 2.6 0 0 1 17.4 9" /></> },
  { name: "ghost", label: "Horror", shape: <><path d="M12 2a8 8 0 0 0-8 8v12l3-3 2.5 2.5L12 19l2.5 2.5L17 19l3 3V10a8 8 0 0 0-8-8z" /><path d="M9 10h.01M15 10h.01" /></> },
  { name: "rocket", label: "Sci-fi", shape: <><path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" /><path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" /><path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" /></> },
  { name: "book", label: "Stories", shape: <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" /> },
  { name: "camera", label: "Home videos", shape: <><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z" /><circle cx="12" cy="13" r="3" /></> },
  { name: "family", label: "Family", shape: <><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" /></> },
  { name: "trophy", label: "Awards", shape: <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6M18 9h1.5a2.5 2.5 0 0 0 0-5H18M4 22h16M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22M18 2H6v7a6 6 0 0 0 12 0V2z" /> },
  { name: "laugh", label: "Comedy", shape: <><circle cx="12" cy="12" r="9" /><path d="M7.5 13h9a4.5 4.5 0 0 1-9 0z" /><path d="M9 9h.01M15 9h.01" /></> },
  { name: "sword", label: "Action", shape: <path d="M14.5 17.5 3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2" /> },
  { name: "leaf", label: "Nature", shape: <><path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10z" /><path d="M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12" /></> },
];

const BY_NAME = new Map(LIBRARY_ICONS.map((i) => [i.name, i]));

export function defaultLibraryIcon(kind: LibraryKind): string {
  return kind === "shows" ? "tv" : "film";
}

/** A library's icon: its chosen preset, else its kind's default. */
export function LibraryIcon({ icon, kind, size = 20, className }: { icon?: string | null; kind: LibraryKind; size?: number; className?: string }) {
  const preset = (icon && BY_NAME.get(icon)) || BY_NAME.get(defaultLibraryIcon(kind))!;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable={false}
      className={className}
    >
      {preset.shape}
    </svg>
  );
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
