import { useEffect, type RefObject } from "react";

/**
 * Remote-style focus movement inside `root`, the way a TV app moves: Left and
 * Right step through the focused item's group (a row), Up and Down jump to
 * the neighbouring group's item nearest in x. Groups are `[data-nav-group]`
 * in document order, items `[data-nav-item]` inside them. Keys typed into a
 * field, or with a modifier, are left alone.
 */
export function useSpatialNav(root: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) return;
      const current = (e.target as HTMLElement).closest<HTMLElement>("[data-nav-item]");
      const group = current?.closest<HTMLElement>("[data-nav-group]");
      if (!current || !group || !el.contains(group)) return;
      const items = (g: Element) => [...g.querySelectorAll<HTMLElement>("[data-nav-item]")].filter((i) => i.offsetParent !== null);
      let target: HTMLElement | undefined;
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        const row = items(group);
        target = row[row.indexOf(current) + (e.key === "ArrowRight" ? 1 : -1)];
      } else {
        const groups = [...el.querySelectorAll<HTMLElement>("[data-nav-group]")].filter((g) => items(g).length > 0);
        const next = groups[groups.indexOf(group) + (e.key === "ArrowDown" ? 1 : -1)];
        if (next) target = nearest(items(next), current);
      }
      if (!target) return;
      e.preventDefault();
      target.focus({ preventScroll: true });
      target.scrollIntoView({ block: e.key === "ArrowUp" || e.key === "ArrowDown" ? "center" : "nearest", inline: "nearest", behavior: reducedMotion() ? "auto" : "smooth" });
    };
    el.addEventListener("keydown", onKey);
    return () => el.removeEventListener("keydown", onKey);
  }, [root]);
}

function nearest(candidates: HTMLElement[], from: HTMLElement): HTMLElement | undefined {
  const x = center(from);
  return candidates.reduce<HTMLElement | undefined>((best, c) => (!best || Math.abs(center(c) - x) < Math.abs(center(best) - x) ? c : best), undefined);
}

function center(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  return r.left + r.width / 2;
}

export const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
