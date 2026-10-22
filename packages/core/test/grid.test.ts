import { expect, test } from "bun:test";
import { anchorsOf, cardsByOffset, layoutGrid, nextAnchor, offsetsInView, offsetTop, stepOffset, type GridGroup, type GridPage } from "../src";

const groups: GridGroup[] = [
  { key: "A", label: "A", count: 5, after: null },
  { key: "B", label: "B", count: 3, after: "cB" },
  { key: "C", label: "C", count: 10, after: "cC" },
];
const metrics = { columns: 4, rowHeight: 100, headerHeight: 40, sectionGap: 20 };

test("anchors are the start, each group's cursor and each loaded page's next", () => {
  const pages: GridPage<number>[] = [{ offset: 0, items: [0, 1, 2, 3], next: "p4" }];
  expect(anchorsOf(groups, pages)).toEqual([
    { offset: 0, after: null },
    { offset: 4, after: "p4" },
    { offset: 5, after: "cB" },
    { offset: 8, after: "cC" },
  ]);
});

test("a gap fetches from the last anchor before it, and waits while that anchor loads", () => {
  const anchors = anchorsOf(groups, []);
  expect(nextAnchor({ from: 9, to: 12 }, 18, anchors, new Map(), new Set())).toEqual({ offset: 8, after: "cC" });
  expect(nextAnchor({ from: 9, to: 12 }, 18, anchors, new Map(), new Set([8]))).toBeNull();
  expect(nextAnchor({ from: 0, to: 2 }, 18, anchors, new Map(), new Set())).toEqual({ offset: 0, after: null });
});

test("a loaded range asks for nothing, and paging past the end asks for nothing", () => {
  const pages: GridPage<number>[] = [{ offset: 8, items: [8, 9, 10, 11, 12, 13, 14, 15, 16, 17], next: null }];
  const cards = cardsByOffset(pages);
  expect(nextAnchor({ from: 8, to: 30 }, 18, anchorsOf(groups, pages), cards, new Set([8]))).toBeNull();
});

test("overlapping pages land on the same offsets, so no card appears twice", () => {
  const fromStart: GridPage<string> = { offset: 0, items: ["a0", "a1", "a2", "a3", "a4", "b0", "b1"], next: "p7" };
  const fromB: GridPage<string> = { offset: 5, items: ["b0", "b1", "b2"], next: "cC" };
  const cards = cardsByOffset([fromStart, fromB]);
  expect([...cards.values()]).toEqual(["a0", "a1", "a2", "a3", "a4", "b0", "b1", "b2"]);
});

test("layout stacks a header, whole rows and a gap per group", () => {
  const layout = layoutGrid(groups, 18, true, metrics);
  expect(layout.sections.map((s) => [s.key, s.start, s.top, s.rows, s.height])).toEqual([
    ["A", 0, 0, 2, 260],
    ["B", 5, 260, 1, 160],
    ["C", 8, 420, 3, 360],
  ]);
  expect(layout.height).toBe(780);
  expect(offsetTop(layout, 8)).toBe(460);
  expect(offsetTop(layout, 13)).toBe(560);
});

test("without headers the grid is one run of rows, and a letter still has a place in it", () => {
  const layout = layoutGrid(groups, 18, false, metrics);
  expect(layout.sections).toHaveLength(1);
  expect(layout.height).toBe(500);
  expect(offsetTop(layout, 8)).toBe(200);
});

test("the cards in view span every section the window overlaps", () => {
  const layout = layoutGrid(groups, 18, true, metrics);
  expect(offsetsInView(layout, 0, 150)).toEqual({ from: 0, to: 5 });
  expect(offsetsInView(layout, 250, 470)).toEqual({ from: 5, to: 12 });
  expect(offsetsInView(layout, 230, 470)).toEqual({ from: 4, to: 12 });
  expect(offsetsInView(layout, 900, 1000)).toEqual({ from: 0, to: 0 });
});

test("Up and Down keep the column and cross group boundaries", () => {
  const layout = layoutGrid(groups, 18, true, metrics);
  expect(stepOffset(layout, 1, "down")).toBe(4);
  expect(stepOffset(layout, 3, "down")).toBe(4);
  expect(stepOffset(layout, 4, "down")).toBe(5);
  expect(stepOffset(layout, 6, "down")).toBe(9);
  expect(stepOffset(layout, 9, "up")).toBe(6);
  expect(stepOffset(layout, 8, "up")).toBe(5);
  expect(stepOffset(layout, 5, "up")).toBe(4);
  expect(stepOffset(layout, 2, "up")).toBeNull();
  expect(stepOffset(layout, 17, "down")).toBeNull();
  expect(stepOffset(layout, 17, "right")).toBeNull();
  expect(stepOffset(layout, 0, "left")).toBeNull();
});
