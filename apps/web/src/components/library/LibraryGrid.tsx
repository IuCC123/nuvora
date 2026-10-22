import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import {
  anchorsOf,
  canGroup,
  cardsByOffset,
  groupOffsets,
  hasGroups,
  JUMP_LETTERS,
  showsGroups,
  layoutGrid,
  nextAnchor,
  offsetsInView,
  offsetTop,
  stepOffset,
  type Anchor,
  type GridGroup,
  type GridLayout,
  type GridPage,
  type LibraryView,
  type PosterCard,
} from "@nuvora/core";
import { queries } from "../../nuvora";
import { PosterCardView } from "../PosterCard";

const MIN_CARD = 150;
const GAP_X = 16;
/** Poster caption: 10px margin, a 20px title line and an 18px year line. */
const CAPTION = 48;
const GAP_Y = 28;
/** A group header: its 20px title sits 14px down, with 22px under it before the first row. */
const HEADER = 64;
const SECTION_GAP = 12;
/** Rows drawn beyond each edge of the window, so a quick scroll lands on painted posters. */
const OVERSCAN_ROWS = 2;
/** Rows of blank posters before the first page says how many there are. */
const SKELETON_ROWS = 3;

/**
 * The library grid. Every card has a fixed place (its offset in the sorted,
 * filtered list), so the whole grid is laid out at once from the index and the
 * first page's total, and only the rows near the window are in the page.
 * Missing cards are blank posters until their page arrives from the nearest
 * cursor. Group headers stick under the toolbar; the title sort gets an A to Z
 * jump bar. Arrow keys move between cards across groups; with the title sort, a
 * letter key jumps to that letter.
 */
export function LibraryGrid({
  libraryId,
  query,
  view,
  stickyTop,
  noun,
  empty,
}: {
  libraryId: number;
  /** `libraryQuery(view, movies)`; the grid is keyed on it, so a new query starts a fresh grid. */
  query: Record<string, string>;
  view: LibraryView;
  /** Where sticky things end under the top bar and toolbar, in px from the window's top. */
  stickyTop: number;
  noun: string;
  empty: ReactNode;
}) {
  const grouped = hasGroups(view.sort);
  const index = useQuery({ ...queries.libraryIndex(libraryId, query), enabled: grouped });
  const groups = useMemo<GridGroup[] | undefined>(() => (grouped ? index.data?.groups.map((g) => ({ ...g, after: g.after ?? null })) : []), [grouped, index.data]);
  const [requested, setRequested] = useState<Anchor[]>([{ offset: 0, after: null }]);
  const results = useQueries({ queries: requested.map((a) => queries.libraryPage(libraryId, query, a.after)) });
  const pages: GridPage<PosterCard>[] = requested.flatMap((a, i) => {
    const data = results[i]?.data;
    return data ? [{ offset: a.offset, items: data.items, next: data.next ?? null }] : [];
  });
  const first = results[0];
  const total = first?.data?.total ?? (groups && grouped ? groups.reduce((n, g) => n + g.count, 0) : undefined);
  const failed = results.some((r) => r.isError);

  const wrap = useRef<HTMLDivElement>(null);
  const width = useWidth(wrap);
  const columns = Math.max(2, Math.floor((width + GAP_X) / (MIN_CARD + GAP_X)));
  const cardWidth = width ? (width - GAP_X * (columns - 1)) / columns : MIN_CARD;
  const metrics = { columns, rowHeight: Math.round(cardWidth * 1.5 + CAPTION + GAP_Y), headerHeight: HEADER, sectionGap: SECTION_GAP };
  const headers = total !== undefined && showsGroups(view, total);
  const layout = useMemo(
    () => (total === undefined || !groups ? null : layoutGrid(groups, total, headers, metrics)),
    [groups, total, headers, metrics.columns, metrics.rowHeight],
  );
  const viewport = useViewport(wrap);
  const cards = useMemo(() => cardsByOffset(pages), [results.map((r) => r.dataUpdatedAt).join()]);
  const anchors = useMemo(() => anchorsOf(groups ?? [], pages), [groups, cards]);
  const [focused, setFocused] = useState<number | null>(null);
  const pendingFocus = useRef<number | null>(null);

  const overscan = OVERSCAN_ROWS * metrics.rowHeight;
  const range = layout ? offsetsInView(layout, viewport.top - overscan, viewport.bottom + overscan) : null;

  useEffect(() => {
    if (!layout || !range || total === undefined) return;
    const next = nextAnchor(range, total, anchors, cards, new Set(requested.map((a) => a.offset)));
    if (next) setRequested((r) => (r.some((a) => a.offset === next.offset) ? r : [...r, next]));
  });

  // A card that was focused by the keyboard before its page arrived takes focus once it has a link.
  useEffect(() => {
    const at = pendingFocus.current;
    if (at === null) return;
    const el = wrap.current?.querySelector<HTMLElement>(`[data-offset="${at}"] a.poster-card, [data-offset="${at}"] [tabindex]`);
    if (!el) return;
    if (el.tagName === "A") pendingFocus.current = null;
    if (document.activeElement !== el) el.focus({ preventScroll: true });
  });

  if (first?.isError && !first.data) {
    return (
      <div className="library-notice" role="alert">
        <p>Couldn't load this library.</p>
        <button type="button" className="btn btn-secondary btn-small" onClick={() => void first.refetch()}>
          Try again
        </button>
      </div>
    );
  }
  if (total === 0) return <>{empty}</>;

  const gridTop = () => (wrap.current?.getBoundingClientRect().top ?? 0) + window.scrollY;

  const reveal = (offset: number) => {
    if (!layout) return;
    const top = gridTop() + offsetTop(layout, offset);
    const sticky = stickyTop + (headers ? HEADER : 0);
    const bottom = top + metrics.rowHeight - GAP_Y;
    if (top < window.scrollY + sticky) window.scrollTo({ top: top - sticky - 8 });
    else if (bottom > window.scrollY + window.innerHeight) window.scrollTo({ top: bottom - window.innerHeight + 16 });
  };

  const focusOffset = (offset: number) => {
    pendingFocus.current = offset;
    setFocused(offset);
    reveal(offset);
  };

  const jumpTo = (key: string, focus: boolean) => {
    if (!layout || !groups) return;
    const i = groups.findIndex((g) => g.key === key);
    if (i < 0) return;
    const start = groupOffsets(groups)[i]!;
    const top = headers ? layout.sections[i]!.top : offsetTop(layout, start);
    window.scrollTo({ top: gridTop() + top - stickyTop - (headers ? 0 : 8) });
    if (focus) {
      pendingFocus.current = start;
      setFocused(start);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!layout || e.metaKey || e.ctrlKey || e.altKey) return;
    const cell = (e.target as HTMLElement).closest<HTMLElement>("[data-offset]");
    const at = cell ? Number(cell.dataset.offset) : null;
    const steps: Record<string, "left" | "right" | "up" | "down"> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };
    let target: number | null = null;
    if (at !== null && steps[e.key]) target = stepOffset(layout, at, steps[e.key]!);
    else if (at !== null && e.key === "Home") target = 0;
    else if (at !== null && e.key === "End" && total) target = total - 1;
    else if (view.sort === "title" && /^[a-z#]$/i.test(e.key) && groups?.some((g) => g.key === e.key.toUpperCase())) {
      e.preventDefault();
      return jumpTo(e.key.toUpperCase(), true);
    } else return;
    e.preventDefault();
    if (target !== null) focusOffset(target);
  };

  const activeKey = (() => {
    if (!layout || !groups?.length) return null;
    const { from } = offsetsInView(layout, viewport.top + 8, viewport.bottom);
    const offsets = groupOffsets(groups);
    let i = 0;
    while (i + 1 < offsets.length && offsets[i + 1]! <= from) i++;
    return groups[i]?.key ?? null;
  })();

  return (
    <div className="library-grid-area">
      <div
        ref={wrap}
        className="library-grid"
        style={{ height: layout ? layout.height : undefined, "--card-width": `${cardWidth}px`, "--columns": columns } as CSSProperties}
        onKeyDown={onKeyDown}
        onFocus={(e) => {
          const cell = (e.target as HTMLElement).closest<HTMLElement>("[data-offset]");
          if (cell) setFocused(Number(cell.dataset.offset));
        }}
      >
        {layout && total !== undefined ? (
          <Sections layout={layout} viewport={viewport} overscan={overscan} cards={cards} focused={focused} total={total} noun={noun} />
        ) : (
          <SkeletonGrid columns={columns} />
        )}
      </div>
      {view.sort === "title" && layout && groups && canGroup(view.sort, total ?? 0) && (
        <JumpBar groups={groups} descending={view.order === "desc"} active={activeKey} onJump={(key) => jumpTo(key, false)} />
      )}
      {failed && first?.data && (
        <div className="library-notice library-notice-float" role="alert">
          <p>Some posters couldn't load.</p>
          <button type="button" className="btn btn-secondary btn-small" onClick={() => results.forEach((r) => r.isError && void r.refetch())}>
            Try again
          </button>
        </div>
      )}
    </div>
  );
}

function Sections({
  layout,
  viewport,
  overscan,
  cards,
  focused,
  total,
  noun,
}: {
  layout: GridLayout;
  viewport: { top: number; bottom: number };
  overscan: number;
  cards: ReadonlyMap<number, PosterCard>;
  focused: number | null;
  total: number;
  noun: string;
}) {
  const { columns, rowHeight, headerHeight } = layout.metrics;
  const top = viewport.top - overscan;
  const bottom = viewport.bottom + overscan;
  return (
    <>
      {layout.sections.map((s) => {
        const inView = s.top + s.height > top && s.top < bottom;
        const holdsFocus = focused !== null && focused >= s.start && focused < s.start + s.count;
        if (!inView && !holdsFocus) return null;
        const rowsTop = s.label === null ? 0 : headerHeight;
        const firstRow = Math.max(0, Math.floor((top - s.top - rowsTop) / rowHeight));
        const lastRow = Math.min(s.rows, Math.ceil((bottom - s.top - rowsTop) / rowHeight));
        const rows = new Set<number>();
        if (inView) for (let r = firstRow; r < lastRow; r++) rows.add(r);
        if (holdsFocus) rows.add(Math.floor((focused! - s.start) / columns));
        const headId = `group-${s.key}`;
        return (
          <section key={s.key} className="grid-group" style={{ top: s.top, height: s.height }} aria-labelledby={s.label === null ? undefined : headId} aria-label={s.label === null ? noun : undefined}>
            {s.label !== null && (
              <h2 id={headId} className="grid-group-head">
                <span>{s.label}</span>
                <span className="grid-group-count">{s.count.toLocaleString()}</span>
              </h2>
            )}
            <div role="list" className="grid-rows">
              {[...rows].map((r) => {
                const start = s.start + r * columns;
                const end = Math.min(s.start + s.count, start + columns);
                return Array.from({ length: end - start }, (_, c) => {
                  const offset = start + c;
                  const card = cards.get(offset);
                  return (
                    <div
                      key={offset}
                      role="listitem"
                      aria-posinset={offset + 1}
                      aria-setsize={total}
                      data-offset={offset}
                      className="grid-cell"
                      style={{ top: rowsTop + r * rowHeight, left: `calc(${c} * (var(--card-width) + ${GAP_X}px))` }}
                    >
                      {card ? (
                        <PosterCardView card={card} />
                      ) : (
                        <div className="poster-card grid-blank" tabIndex={-1} aria-label="Loading">
                          <span className="poster-frame" />
                          <span className="poster-caption">
                            <span className="grid-blank-line" />
                            <span className="grid-blank-line grid-blank-line-short" />
                          </span>
                        </div>
                      )}
                    </div>
                  );
                });
              })}
            </div>
          </section>
        );
      })}
    </>
  );
}

function SkeletonGrid({ columns }: { columns: number }) {
  return (
    <div className="grid-skeleton" aria-busy="true" aria-label="Loading">
      {Array.from({ length: columns * SKELETON_ROWS }, (_, i) => (
        <div key={i} className="grid-blank" aria-hidden>
          <span className="poster-frame" />
          <span className="poster-caption">
            <span className="grid-blank-line" />
            <span className="grid-blank-line grid-blank-line-short" />
          </span>
        </div>
      ))}
    </div>
  );
}

/**
 * Letters down the right edge. Clicking or dragging along them jumps to the
 * letter's group; letters with nothing in them are dimmed and skipped by Tab.
 * Up and Down move between the letters that have items.
 */
function JumpBar({ groups, descending, active, onJump }: { groups: GridGroup[]; descending: boolean; active: string | null; onJump: (key: string) => void }) {
  const present = new Set(groups.map((g) => g.key));
  const letters = descending ? [...JUMP_LETTERS].reverse() : [...JUMP_LETTERS];
  const dragging = useRef(false);
  const last = useRef<string | null>(null);
  const jumpAt = (x: number, y: number) => {
    const key = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>("[data-letter]")?.dataset.letter;
    if (key && present.has(key) && key !== last.current) {
      last.current = key;
      onJump(key);
    }
  };
  return (
    <nav
      className="jump-bar"
      aria-label="Jump to letter"
      onPointerDown={(e) => {
        if (e.pointerType === "touch" || e.button !== 0) return;
        dragging.current = true;
        last.current = null;
        e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => dragging.current && jumpAt(e.clientX, e.clientY)}
      onPointerUp={() => (dragging.current = false)}
      onPointerCancel={() => (dragging.current = false)}
      onKeyDown={(e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        const buttons = [...e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
        const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
        buttons[at + (e.key === "ArrowDown" ? 1 : -1)]?.focus();
        e.preventDefault();
      }}
    >
      {letters.map((letter) => (
        <button
          key={letter}
          type="button"
          data-letter={letter}
          className="jump-letter"
          disabled={!present.has(letter)}
          aria-current={active === letter ? "true" : undefined}
          aria-label={letter === "#" ? "Numbers and symbols" : letter}
          onClick={() => onJump(letter)}
        >
          {letter}
        </button>
      ))}
    </nav>
  );
}

function useWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    const observer = new ResizeObserver(() => setWidth(el.clientWidth));
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

/** The window's span in the grid's own coordinates, updated once a frame while scrolling. */
function useViewport(ref: RefObject<HTMLElement | null>): { top: number; bottom: number } {
  const [viewport, setViewport] = useState({ top: 0, bottom: typeof window === "undefined" ? 800 : window.innerHeight });
  useLayoutEffect(() => {
    let frame = 0;
    const measure = () => {
      frame = 0;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const top = -rect.top;
      setViewport((v) => (Math.abs(v.top - top) < 1 && v.bottom - v.top === window.innerHeight ? v : { top, bottom: top + window.innerHeight }));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    measure();
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    };
  }, [ref]);
  return viewport;
}
