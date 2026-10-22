import { expect, test } from "bun:test";
import { hasViewParams, libraryQuery, librarySearch, parseLibraryView, showsGroups, withSort, DEFAULT_VIEW } from "../src";

test("a plain URL is the default view, and the default view writes no params", () => {
  expect(parseLibraryView({})).toEqual(DEFAULT_VIEW);
  expect(librarySearch(DEFAULT_VIEW)).toEqual({});
  expect(hasViewParams({})).toBe(false);
});

test("a view round-trips through search params", () => {
  const view = { sort: "year", order: "asc", filter: "unwatched", res: "4k", hdr: true, group: false } as const;
  const search = librarySearch(view);
  expect(search).toEqual({ sort: "year", order: "asc", filter: "unwatched", res: "4k", hdr: "1", group: "0" });
  expect(parseLibraryView(search)).toEqual(view);
  expect(hasViewParams(search)).toBe(true);
});

test("bad values fall back one by one, and a missing order takes the sort's own", () => {
  expect(parseLibraryView({ sort: "added", order: "sideways", filter: 7, res: "8k" })).toEqual({ ...DEFAULT_VIEW, sort: "added", order: "desc" });
});

test("a new sort starts in its natural direction and keeps the filters", () => {
  const view = { ...DEFAULT_VIEW, filter: "watched" as const, order: "desc" as const };
  expect(withSort(view, "year")).toEqual({ ...view, sort: "year", order: "desc" });
  expect(withSort(view, "title").order).toBe("asc");
});

test("groups show only for sorts that have them", () => {
  expect(showsGroups({ ...DEFAULT_VIEW, sort: "runtime" })).toBe(false);
  expect(showsGroups({ ...DEFAULT_VIEW, sort: "year" })).toBe(true);
  expect(showsGroups({ ...DEFAULT_VIEW, group: false })).toBe(false);
});

test("resolution and HDR reach the server only for movie libraries", () => {
  const view = { ...DEFAULT_VIEW, res: "1080p" as const, hdr: true };
  expect(libraryQuery(view, true)).toEqual({ sort: "title", order: "asc", filter: "all", res: "1080p", hdr: "true" });
  expect(libraryQuery(view, false)).toEqual({ sort: "title", order: "asc", filter: "all" });
});
