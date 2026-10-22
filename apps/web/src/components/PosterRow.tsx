import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { rowCards, type HomeRow } from "@nuvora/core";
import { queries } from "../nuvora";
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
  const headingId = `row-${row.kind}`;
  const strip = useRef<HTMLUListElement>(null);
  const [armed, setArmed] = useState(false);
  const pages = useInfiniteQuery({ ...queries.rowPages(row.kind, row.next ?? ""), enabled: armed && row.next != null });
  const { cards, hasMore } = rowCards(row, pages.data?.pages);
  const pending = hasMore ? Math.min(MAX_PENDING_CARDS, Math.max(1, row.total - cards.length)) : 0;
  const { atStart, atEnd } = useScrollEdges(strip, cards.length + pending);

  const loadMore = useCallback(() => {
    if (!armed) setArmed(true);
    else if (pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }, [armed, pages]);
  const sentinel = useNearEnd(strip, hasMore && !pages.isFetching, loadMore);

  return (
    <section className="row" aria-labelledby={`${headingId}-title`} id={headingId}>
      <div className="row-head">
        <h2 id={`${headingId}-title`}>{row.title}</h2>
        {row.kind === "all_movies" && <span className="row-count">{row.total}</span>}
        <div className="row-arrows">
          <button type="button" className="round-btn" disabled={atStart} onClick={() => page(strip.current, -1)} aria-label={`Scroll ${row.title} back`}>
            <ChevronLeftIcon />
          </button>
          <button type="button" className="round-btn" disabled={atEnd && !hasMore} onClick={() => page(strip.current, 1)} aria-label={`Scroll ${row.title} forward`}>
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
function page(strip: HTMLElement | null, direction: 1 | -1) {
  const first = strip?.querySelector<HTMLElement>(".row-item");
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

function useScrollEdges(strip: RefObject<HTMLElement | null>, itemCount: number) {
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
