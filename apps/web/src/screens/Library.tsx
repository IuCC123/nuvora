import { LibraryIcon } from "../components/LibraryIcon";
import { useLayoutEffect, useState, type CSSProperties, type KeyboardEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import {
  canGroup,
  FILTER_LABELS,
  isFiltered,
  LIBRARY_SORTS,
  libraryQuery,
  ORDER_LABELS,
  plural,
  RESOLUTION_LABELS,
  RESOLUTIONS,
  SORT_LABELS,
  WATCH_FILTERS,
  withSort,
  type LibrarySummary,
  type LibraryView,
} from "@nuvora/core";
import { tokens } from "@nuvora/tokens";
import { queries } from "../nuvora";
import { CheckIcon, ChevronDownIcon, CloseIcon, SortDirectionIcon } from "../components/Icon";
import { Menu, MenuCheckbox, MenuGroup, MenuRadio } from "../components/Menu";
import { LibraryGrid } from "../components/library/LibraryGrid";
import { useUser } from "../lib/session";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useLibraryView } from "../lib/useLibraryView";
import { useStartScan } from "../lib/useStartScan";
import { MissingLibrary } from "./ErrorState";
import "../styles/home.css";
import "../styles/library.css";

const GROUP_NAMES = { title: "letter", year: "decade", added: "month" } as const;

/**
 * One library's movies or shows as a grid: sort, filter and group from the
 * toolbar, which stays under the top bar while the grid scrolls. The view
 * lives in the URL and on this device (`useLibraryView`).
 */
export function LibraryScreen() {
  const { id } = useParams({ from: "/shell/library/$id" });
  const { data: libraries } = useQuery(queries.libraries());
  const library = libraries?.find((l) => l.id === id);
  const [view, setView] = useLibraryView(id);
  useDocumentTitle(library?.name);
  const [toolbar, setToolbar] = useState<HTMLDivElement | null>(null);
  const stickyTop = tokens.layout.topBar + useHeight(toolbar);

  const total = useQuery({ ...queries.libraryPage(id, library ? libraryQuery(view, library.kind === "movies") : {}, null), enabled: !!library }).data?.total;

  if (!library) return libraries ? <MissingLibrary /> : null;
  const movies = library.kind === "movies";
  const query = libraryQuery(view, movies);
  const noun = movies ? "movie" : "show";
  const filtered = isFiltered(view, movies);
  return (
    <div className="library" style={{ "--library-sticky": `${stickyTop}px` } as CSSProperties}>
      <header className="library-head">
        <h1 className="library-heading">
          <LibraryIcon icon={library.icon} kind={library.kind} size={26} className="library-heading-icon" />
          {library.name}
        </h1>
        <FilteredCount total={total} library={library} filtered={filtered} noun={noun} />
      </header>
      <div className="library-toolbar" ref={setToolbar}>
        <Toolbar view={view} movies={movies} groupable={canGroup(view.sort, total ?? library.item_count)} onChange={setView} />
      </div>
      <LibraryGrid
        key={JSON.stringify(query)}
        libraryId={id}
        query={query}
        view={view}
        stickyTop={stickyTop}
        noun={movies ? "Movies" : "Shows"}
        empty={
          filtered ? (
            <FilteredEmpty view={view} movies={movies} library={library} onClear={() => setView({ ...view, filter: "all", res: null, hdr: false })} />
          ) : (
            <LibraryEmpty library={library} />
          )
        }
      />
    </div>
  );
}

/** "592 movies", or "41 of 592 movies" while a filter narrows them. The status role announces each change. */
function FilteredCount({ total, library, filtered, noun }: { total: number | undefined; library: LibrarySummary; filtered: boolean; noun: string }) {
  const all = library.item_count;
  const text = filtered && total !== undefined ? `${total.toLocaleString()} of ${plural(all, noun)}` : plural(all, noun);
  return (
    <p className="library-total" role="status">
      {text}
    </p>
  );
}

function Toolbar({ view, movies, groupable, onChange }: { view: LibraryView; movies: boolean; groupable: boolean; onChange: (v: LibraryView) => void }) {
  const flip = view.order === "asc" ? "desc" : "asc";
  return (
    <div className="toolbar">
      <div className="toolbar-sort">
        <Menu
          label="Sort by"
          triggerLabel={`Sort by ${SORT_LABELS[view.sort]}`}
          trigger={
            <>
              <span className="tool-btn-label">Sort</span>
              {SORT_LABELS[view.sort]}
              <ChevronDownIcon />
            </>
          }
        >
          {LIBRARY_SORTS.map((sort) => (
            <MenuRadio key={sort} checked={view.sort === sort} onSelect={() => onChange(withSort(view, sort))}>
              {SORT_LABELS[sort]}
            </MenuRadio>
          ))}
        </Menu>
        <button
          type="button"
          className="tool-btn"
          aria-label={`${ORDER_LABELS[view.sort][view.order]}. Switch to ${ORDER_LABELS[view.sort][flip]}`}
          title={`Switch to ${ORDER_LABELS[view.sort][flip]}`}
          onClick={() => onChange({ ...view, order: flip })}
        >
          <SortDirectionIcon descending={view.order === "desc"} />
          {ORDER_LABELS[view.sort][view.order]}
        </button>
      </div>
      <FilterChips value={view.filter} onChange={(filter) => onChange({ ...view, filter })} />
      <div className="toolbar-end">
        {movies && <QualityMenu view={view} onChange={onChange} />}
        {groupable && (
          <button
            type="button"
            className="tool-btn group-toggle"
            aria-pressed={view.group}
            onClick={() => onChange({ ...view, group: !view.group })}
          >
            <span className="group-box" aria-hidden>
              {view.group && <CheckIcon size={12} strokeWidth={3} />}
            </span>
            Group by {GROUP_NAMES[view.sort as keyof typeof GROUP_NAMES]}
          </button>
        )}
      </div>
    </div>
  );
}

/** Watch-state chips: one choice of four, so a radio group; Left and Right move and choose. */
function FilterChips({ value, onChange }: { value: LibraryView["filter"]; onChange: (f: LibraryView["filter"]) => void }) {
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = WATCH_FILTERS[(WATCH_FILTERS.indexOf(value) + step + WATCH_FILTERS.length) % WATCH_FILTERS.length]!;
    onChange(next);
    e.currentTarget.querySelector<HTMLElement>(`[data-filter="${next}"]`)?.focus();
  };
  return (
    <div role="radiogroup" aria-label="Show" className="filter-chips" onKeyDown={onKeyDown}>
      {WATCH_FILTERS.map((f) => (
        <button
          key={f}
          type="button"
          role="radio"
          data-filter={f}
          aria-checked={value === f}
          tabIndex={value === f ? 0 : -1}
          className="filter-chip"
          onClick={() => onChange(f)}
        >
          {FILTER_LABELS[f]}
        </button>
      ))}
    </div>
  );
}

function QualityMenu({ view, onChange }: { view: LibraryView; onChange: (v: LibraryView) => void }) {
  const summary = [view.res ? RESOLUTION_LABELS[view.res] : null, view.hdr ? "HDR" : null].filter(Boolean).join(" · ");
  return (
    <Menu
      label="Quality"
      align="end"
      triggerClassName={summary ? "tool-btn tool-btn-set" : "tool-btn"}
      triggerLabel={summary ? `Quality: ${summary}` : "Quality: any"}
      trigger={
        <>
          <span className="tool-btn-label">Quality</span>
          {summary || "Any"}
          <ChevronDownIcon />
        </>
      }
    >
      <MenuGroup label="Resolution">
        <MenuRadio checked={view.res === null} onSelect={() => onChange({ ...view, res: null })}>
          Any resolution
        </MenuRadio>
        {RESOLUTIONS.map((res) => (
          <MenuRadio key={res} checked={view.res === res} onSelect={() => onChange({ ...view, res })}>
            {RESOLUTION_LABELS[res]}
          </MenuRadio>
        ))}
      </MenuGroup>
      <MenuGroup label="Dynamic range">
        <MenuCheckbox checked={view.hdr} onToggle={() => onChange({ ...view, hdr: !view.hdr })}>
          HDR only
        </MenuCheckbox>
      </MenuGroup>
    </Menu>
  );
}

/** "No unwatched 4K HDR movies", with the one action that brings the grid back. */
function FilteredEmpty({ view, movies, library, onClear }: { view: LibraryView; movies: boolean; library: LibrarySummary; onClear: () => void }) {
  const words = [
    view.filter === "all" ? null : FILTER_LABELS[view.filter].toLowerCase(),
    movies && view.res ? RESOLUTION_LABELS[view.res] : null,
    movies && view.hdr ? "HDR" : null,
    movies ? "movies" : "shows",
  ].filter(Boolean);
  return (
    <div className="empty library-empty">
      <h2>No {words.join(" ")}</h2>
      <p>
        Nothing in {library.name} matches these filters. Clear them to see all {plural(library.item_count, movies ? "movie" : "show")}.
      </p>
      <button type="button" className="btn btn-secondary" onClick={onClear}>
        <CloseIcon />
        Clear filters
      </button>
    </div>
  );
}

function LibraryEmpty({ library }: { library: LibrarySummary }) {
  const admin = useUser()?.role === "admin";
  const scan = useStartScan();
  const files = library.kind === "movies" ? "movie files" : "episode files, one folder per show";
  return (
    <div className="empty library-empty">
      <h2>{library.name} is empty</h2>
      {admin ? (
        <>
          <p>Add {files} to this library's folders, then scan to bring them in.</p>
          <div className="library-empty-actions">
            <button type="button" className="btn btn-secondary" onClick={() => scan.mutate()} disabled={scan.isPending}>
              Scan library
            </button>
            <Link to="/settings/libraries" className="btn btn-quiet">
              Library settings
            </Link>
          </div>
        </>
      ) : (
        <p>{library.kind === "movies" ? "Movies" : "Shows"} show up here once an admin adds them to this server.</p>
      )}
    </div>
  );
}

/** The toolbar's height, so group headers stick right under it however its controls wrap. */
function useHeight(el: HTMLElement | null): number {
  const [height, setHeight] = useState(64);
  useLayoutEffect(() => {
    if (!el) return;
    setHeight(el.offsetHeight);
    const observer = new ResizeObserver(() => setHeight(el.offsetHeight));
    observer.observe(el);
    return () => observer.disconnect();
  }, [el]);
  return height;
}
