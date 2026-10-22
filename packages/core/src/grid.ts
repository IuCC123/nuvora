/**
 * The library grid as a sparse list: every card has a fixed offset (its place
 * in the sorted, filtered library), known before it loads, so the grid lays out
 * thousands of cells at once and fetches only the ones in view.
 *
 * Paging is keyset, so a page can only start where a cursor points: the very
 * start, a group's `after` from the index, or a loaded page's `next`. Those are
 * the anchors. To fill a gap, fetch from the last anchor at or before it.
 */

export interface GridGroup {
  key: string;
  label: string;
  count: number;
  /** The cursor whose page starts with this group; `null` for the first one. */
  after: string | null;
}

/** A loaded page, placed at the offset of the anchor it was fetched from. */
export interface GridPage<T> {
  offset: number;
  items: readonly T[];
  next: string | null;
}

export interface Anchor {
  offset: number;
  after: string | null;
}

/** Group start offsets, in grid order. */
export function groupOffsets(groups: readonly GridGroup[]): number[] {
  const offsets: number[] = [];
  let at = 0;
  for (const g of groups) {
    offsets.push(at);
    at += g.count;
  }
  return offsets;
}

export function anchorsOf<T>(groups: readonly GridGroup[], pages: readonly GridPage<T>[]): Anchor[] {
  const byOffset = new Map<number, string | null>([[0, null]]);
  groupOffsets(groups).forEach((offset, i) => {
    if (offset > 0) byOffset.set(offset, groups[i]!.after);
  });
  for (const p of pages) if (p.next !== null) byOffset.set(p.offset + p.items.length, p.next);
  return [...byOffset].map(([offset, after]) => ({ offset, after })).sort((a, b) => a.offset - b.offset);
}

/**
 * Cards by offset. Where two pages overlap (a chain from the start reached a
 * group that had loaded from its own cursor) the offsets agree, so the later
 * page simply lands on the same cells and no card appears twice.
 */
export function cardsByOffset<T>(pages: readonly GridPage<T>[]): Map<number, T> {
  const cards = new Map<number, T>();
  for (const p of pages) p.items.forEach((item, i) => cards.set(p.offset + i, item));
  return cards;
}

/**
 * The next page to request so `[from, to)` fills in, or `null` when it is
 * loaded or already on its way. `requested` holds the anchor offsets asked for
 * so far, loaded or in flight.
 */
export function nextAnchor<T>(
  range: { from: number; to: number },
  total: number,
  anchors: readonly Anchor[],
  cards: ReadonlyMap<number, T>,
  requested: ReadonlySet<number>,
): Anchor | null {
  const end = Math.min(range.to, total);
  for (let offset = Math.max(0, range.from); offset < end; offset++) {
    if (cards.has(offset)) continue;
    let best: Anchor | undefined;
    for (const a of anchors) {
      if (a.offset > offset) break;
      best = a;
    }
    // A requested anchor at or before the gap is still loading; its page or its `next` will cover it.
    if (!best || requested.has(best.offset)) return null;
    return best;
  }
  return null;
}

export interface GridMetrics {
  columns: number;
  /** One row of cards, its caption and the gap below it. */
  rowHeight: number;
  headerHeight: number;
  /** Space after a group's last row, before the next header. */
  sectionGap: number;
}

export interface GridSection {
  key: string;
  /** `null` when headers are off: one section holds the whole grid. */
  label: string | null;
  /** Offset of the section's first card. */
  start: number;
  count: number;
  top: number;
  height: number;
  rows: number;
}

export interface GridLayout {
  sections: GridSection[];
  height: number;
  metrics: GridMetrics;
}

export function layoutGrid(groups: readonly GridGroup[], total: number, headers: boolean, metrics: GridMetrics): GridLayout {
  const parts = headers && groups.length ? groups.map((g) => ({ key: g.key, label: g.label as string | null, count: g.count })) : [{ key: "all", label: null, count: total }];
  const sections: GridSection[] = [];
  let top = 0;
  let start = 0;
  for (const part of parts) {
    const rows = Math.ceil(part.count / metrics.columns);
    const height = (part.label === null ? 0 : metrics.headerHeight) + rows * metrics.rowHeight + (part.label === null ? 0 : metrics.sectionGap);
    sections.push({ ...part, start, top, height, rows });
    top += height;
    start += part.count;
  }
  return { sections, height: top, metrics };
}

/** The section holding `offset` (the first one for offsets before it). */
function sectionIndex(layout: GridLayout, offset: number): number {
  let i = 0;
  while (i + 1 < layout.sections.length && layout.sections[i + 1]!.start <= offset) i++;
  return i;
}

/** Where a card's row starts, from the top of the grid. */
export function offsetTop(layout: GridLayout, offset: number): number {
  const section = layout.sections[sectionIndex(layout, offset)];
  if (!section) return 0;
  const header = section.label === null ? 0 : layout.metrics.headerHeight;
  return section.top + header + Math.floor((offset - section.start) / layout.metrics.columns) * layout.metrics.rowHeight;
}

/** The cards whose rows overlap `[top, bottom)` of the grid, as a range of offsets. */
export function offsetsInView(layout: GridLayout, top: number, bottom: number): { from: number; to: number } {
  let from = Infinity;
  let to = 0;
  const { columns, rowHeight, headerHeight } = layout.metrics;
  for (const s of layout.sections) {
    if (s.top + s.height <= top || s.top >= bottom) continue;
    const rowsTop = s.top + (s.label === null ? 0 : headerHeight);
    const first = Math.max(0, Math.floor((top - rowsTop) / rowHeight));
    const last = Math.min(s.rows, Math.ceil((bottom - rowsTop) / rowHeight));
    if (last <= first) continue;
    from = Math.min(from, s.start + first * columns);
    to = Math.max(to, Math.min(s.start + s.count, s.start + last * columns));
  }
  return from === Infinity ? { from: 0, to: 0 } : { from, to };
}

/**
 * The offset one step away in grid terms: Left and Right walk the list, Up and
 * Down keep the column and cross into the neighbouring group's rows. `null` at an edge.
 */
export function stepOffset(layout: GridLayout, offset: number, step: "left" | "right" | "up" | "down"): number | null {
  const total = layout.sections.reduce((n, s) => n + s.count, 0);
  if (step === "left") return offset > 0 ? offset - 1 : null;
  if (step === "right") return offset + 1 < total ? offset + 1 : null;
  const { columns } = layout.metrics;
  const i = sectionIndex(layout, offset);
  const s = layout.sections[i];
  if (!s) return null;
  const row = Math.floor((offset - s.start) / columns);
  const column = (offset - s.start) % columns;
  if (step === "down") {
    if (row + 1 < s.rows) return Math.min(s.start + s.count - 1, s.start + (row + 1) * columns + column);
    const next = layout.sections[i + 1];
    return next ? next.start + Math.min(column, next.count - 1) : null;
  }
  if (row > 0) return s.start + (row - 1) * columns + column;
  const prev = layout.sections[i - 1];
  if (!prev) return null;
  const lastRow = prev.rows - 1;
  return prev.start + Math.min(prev.count - 1, lastRow * columns + column);
}

/** The A to Z jump bar's letters, `#` first. Letters without a group are present but dimmed. */
export const JUMP_LETTERS = ["#", ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"] as const;
