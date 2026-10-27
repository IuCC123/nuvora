import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { homeRowQueryParams, librarySearch, parseLibraryView, rowCards, type HomeRow, type RowPage } from "@nuvora/core";
import { queries } from "../nuvora";
import { LibraryIcon } from "./LibraryIcon";
import { ChevronLeftIcon, ChevronRightIcon } from "./Icon";
import { PosterCardView } from "./PosterCard";

/** Cards still to load are drawn as blank posters, so the strip never ends early or shows a spinner. */
const MAX_PENDING_CARDS = 6;

/**
 * A titled, horizontally scrolling row. Its first page comes with home; the
 * rest loads through the row cursor when the strip nears its end. Every card
 * is a link in tab order, and Left/Right move focus within the row.
 */
export function PosterRow({ row }: { row: HomeRow }) {
  const library = row.kind === "library" ? (row.library_id ?? undefined) : undefined;
  const headingId = `row-${row.id ?? (library === undefined ? row.kind : `library-${library}`)}`;
  const strip = useRef<HTMLUListElement>(null);
  const [armed, setArmed] = useState(false);
  const { data: libraries } = useQuery(queries.libraries());
  const kind = libraries?.find((l) => l.id === library)?.kind ?? "movies";
  // A custom row's later pages come from the library page with its query; built-in rows page through the row cursor.
  const params = row.query && library !== undefined ? homeRowQueryParams(row.query) : null;
  const rowPages = useInfiniteQuery({ ...queries.rowPages(row.kind, row.next ?? "", library), enabled: armed && row.next != null && !params });
  const queryPages = useInfiniteQuery({ ...queries.queryRowPages(library ?? 0, params ?? {}, row.next ?? ""), enabled: armed && row.next != null && !!params });
  const pages = params ? queryPages : rowPages;
  const seeAll = row.query ? librarySearch({ ...parseLibraryView({ ...row.query, res: row.query.resolution }), group: true }) : {};
  const { cards, hasMore } = rowCards(row, pages.data?.pages as RowPage[] | undefined);
  const failed = pages.isError && !pages.isFetching;
  const pending = hasMore && !failed ? Math.min(MAX_PENDING_CARDS, Math.max(1, row.total - cards.length)) : 0;
  const { atStart, atEnd } = useScrollEdges(strip, cards.length + pending);

  const loadMore = useCallback(() => {
    if (!armed) setArmed(true);
    else if (pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }, [armed, pages]);
  // A failed page waits for Try again; re-arming on the error would refetch in a loop.
  const sentinel = useNearEnd(strip, hasMore && !pages.isFetching && !pages.isError, loadMore);
  const retry = () => void (pages.data ? pages.fetchNextPage() : pages.refetch());

  return (
    <section className="row" aria-labelledby={`${headingId}-title`} id={headingId}>
      <div className="row-head">
        <h2 id={`${headingId}-title`}>
          {library === undefined ? (
            row.title
          ) : (
            <Link to="/library/$id" params={{ id: library }} search={seeAll} className="row-title-link">
              <LibraryIcon icon={row.icon} kind={kind} size={20} className="row-icon" />
              {row.title}
            </Link>
          )}
        </h2>
        {(row.kind === "all_movies" || library !== undefined) && <span className="row-count">{row.total.toLocaleString()}</span>}
        <div className="row-arrows">
          {library !== undefined && (
            <Link to="/library/$id" params={{ id: library }} search={seeAll} className="row-see-all" aria-label={`See all of ${row.title}`}>
              See all
            </Link>
          )}
          <button type="button" className="round-btn" disabled={atStart} onClick={() => pageStrip(strip.current, -1)} aria-label={`Scroll ${row.title} back`}>
            <ChevronLeftIcon />
          </button>
          <button type="button" className="round-btn" disabled={atEnd && !hasMore} onClick={() => pageStrip(strip.current, 1)} aria-label={`Scroll ${row.title} forward`}>
            <ChevronRightIcon />
          </button>
        </div>
      </div>
      <ul className="row-strip" ref={strip} role="list" onKeyDown={moveFocus}>
        {cards.map((card) => (
          <li key={card.id} className="row-item">
            <PosterCardView card={card} />
          </li>
        ))}
        {failed && (
          <li className="row-item row-failed">
            <div className="poster-frame">
              <p>Couldn't load more.</p>
              <button type="button" className="btn btn-secondary btn-small" onClick={retry}>
                Try again
              </button>
            </div>
          </li>
        )}
        {Array.from({ length: pending }, (_, i) => (
          <li key={`pending-${i}`} className="row-item row-pending" aria-hidden ref={i === 0 ? sentinel : undefined}>
            <span className="poster-frame" />
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Scroll by the number of whole cards that fit, so a page never cuts a poster in half. */
export function pageStrip(strip: HTMLElement | null, direction: 1 | -1) {
  const first = strip?.firstElementChild as HTMLElement | null | undefined;
  if (!strip || !first) return;
  const gap = parseFloat(getComputedStyle(strip).columnGap) || 0;
  const step = first.offsetWidth + gap;
  const padding = parseFloat(getComputedStyle(strip).paddingLeft) || 0;
  const perPage = Math.max(1, Math.floor((strip.clientWidth - 2 * padding + gap) / step));
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  strip.scrollBy({ left: direction * perPage * step, behavior: reduce ? "auto" : "smooth" });
}

function moveFocus(e: KeyboardEvent<HTMLUListElement>) {
  if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
  const links = [...e.currentTarget.querySelectorAll<HTMLAnchorElement>("a.poster-card")];
  const at = links.indexOf(document.activeElement as HTMLAnchorElement);
  const next = links[at + (e.key === "ArrowRight" ? 1 : -1)];
  if (at < 0 || !next) return;
  e.preventDefault();
  next.focus();
}

export function useScrollEdges(strip: RefObject<HTMLElement | null>, itemCount: number) {
  const [edges, setEdges] = useState({ atStart: true, atEnd: true });
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const update = () =>
      setEdges({ atStart: el.scrollLeft <= 1, atEnd: el.scrollLeft + el.clientWidth >= el.scrollWidth - 1 });
    update();
    el.addEventListener("scroll", update, { passive: true });
    const resize = new ResizeObserver(update);
    resize.observe(el);
    return () => {
      el.removeEventListener("scroll", update);
      resize.disconnect();
    };
  }, [strip, itemCount]);
  return edges;
}

/** Calls `onNear` when the element it is attached to comes within a screen of the strip's visible edge. */
function useNearEnd(strip: RefObject<HTMLElement | null>, active: boolean, onNear: () => void) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  const callback = useRef(onNear);
  callback.current = onNear;
  useEffect(() => {
    if (!active || !target || !strip.current) return;
    const observer = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && callback.current(), {
      root: strip.current,
      rootMargin: "0px 100% 0px 0px",
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [active, target, strip]);
  return setTarget;
}
