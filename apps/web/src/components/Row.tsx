import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { ChevronLeftIcon, ChevronRightIcon } from "./Icon";
import { reducedMotion } from "../lib/spatialNav";
import "../styles/rows.css";

/**
 * A titled shelf that scrolls sideways: the strip runs to the window's right
 * edge, snaps card by card, and leaves room for a lifted card's scale and
 * shadow. Glass paddles at its edges page it for a mouse; touch swipes, and
 * keys move focus (`useSpatialNav` on the screen). `variant` sizes the cards.
 */
export function Row({
  id,
  title,
  action,
  variant,
  itemCount,
  children,
  stripRef,
}: {
  id: string;
  title: ReactNode;
  action?: ReactNode;
  variant: "poster" | "landscape" | "ranked" | "tile";
  itemCount: number;
  children: ReactNode;
  stripRef?: RefObject<HTMLUListElement | null>;
}) {
  const own = useRef<HTMLUListElement>(null);
  const strip = stripRef ?? own;
  const { atStart, atEnd } = useScrollEdges(strip, itemCount);
  const label = typeof title === "string" ? title : "row";
  return (
    <section className="row" data-variant={variant} aria-labelledby={`${id}-title`} id={id}>
      <div className="row-head">
        <h2 id={`${id}-title`}>{title}</h2>
        {action}
      </div>
      <div className="row-body">
        <ul className="row-strip" ref={strip} role="list" data-nav-group>
          {children}
        </ul>
        <button type="button" className="row-paddle glass" data-side="start" hidden={atStart} tabIndex={-1} onClick={() => pageStrip(strip.current, -1)} aria-label={`Scroll ${label} back`}>
          <ChevronLeftIcon size={22} />
        </button>
        <button type="button" className="row-paddle glass" data-side="end" hidden={atEnd} tabIndex={-1} onClick={() => pageStrip(strip.current, 1)} aria-label={`Scroll ${label} forward`}>
          <ChevronRightIcon size={22} />
        </button>
      </div>
    </section>
  );
}

/** Scroll by the number of whole cards that fit, so a page never cuts a card in half. */
export function pageStrip(strip: HTMLElement | null, direction: 1 | -1) {
  const first = strip?.firstElementChild as HTMLElement | null | undefined;
  if (!strip || !first) return;
  const style = getComputedStyle(strip);
  const gap = parseFloat(style.columnGap) || 0;
  const step = first.offsetWidth + gap;
  const padding = parseFloat(style.paddingLeft) || 0;
  const perPage = Math.max(1, Math.floor((strip.clientWidth - 2 * padding + gap) / step));
  strip.scrollBy({ left: direction * perPage * step, behavior: reducedMotion() ? "auto" : "smooth" });
}

export function useScrollEdges(strip: RefObject<HTMLElement | null>, itemCount: number) {
  const [edges, setEdges] = useState({ atStart: true, atEnd: true });
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const update = () => setEdges({ atStart: el.scrollLeft <= 1, atEnd: el.scrollLeft + el.clientWidth >= el.scrollWidth - 1 });
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
