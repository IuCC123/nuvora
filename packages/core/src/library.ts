/**
 * The library page's view: how its grid is sorted, filtered and grouped. One
 * value type, kept in the URL and in device storage, and turned into the
 * server's query by `libraryQuery`. Every client parses and labels it here, so
 * the TV apps offer the same choices in the same words.
 */

export const LIBRARY_SORTS = ["title", "year", "added", "watched", "runtime"] as const;
export type LibrarySort = (typeof LIBRARY_SORTS)[number];

export const SORT_ORDERS = ["asc", "desc"] as const;
export type SortOrder = (typeof SORT_ORDERS)[number];

export const WATCH_FILTERS = ["all", "unwatched", "in_progress", "watched"] as const;
export type WatchFilter = (typeof WATCH_FILTERS)[number];

export const RESOLUTIONS = ["4k", "1080p", "720p", "sd"] as const;
export type Resolution = (typeof RESOLUTIONS)[number];

export interface LibraryView {
  sort: LibrarySort;
  order: SortOrder;
  filter: WatchFilter;
  /** Movie libraries only; a TV library ignores both. */
  res: Resolution | null;
  hdr: boolean;
  /** Group headers on. Only the title, year and added sorts have groups. */
  group: boolean;
}

/** What the server orders by when no order is given, and what a fresh sort starts with. */
export const DEFAULT_ORDER: Record<LibrarySort, SortOrder> = { title: "asc", year: "desc", added: "desc", watched: "desc", runtime: "desc" };

export const DEFAULT_VIEW: LibraryView = { sort: "title", order: "asc", filter: "all", res: null, hdr: false, group: true };

export const SORT_LABELS: Record<LibrarySort, string> = {
  title: "Title",
  year: "Year",
  added: "Date added",
  watched: "Last watched",
  runtime: "Runtime",
};

/** Each sort names its directions in its own terms: "A to Z" reads better than "Ascending". */
export const ORDER_LABELS: Record<LibrarySort, Record<SortOrder, string>> = {
  title: { asc: "A to Z", desc: "Z to A" },
  year: { asc: "Oldest first", desc: "Newest first" },
  added: { asc: "Oldest first", desc: "Newest first" },
  watched: { asc: "Least recent first", desc: "Most recent first" },
  runtime: { asc: "Shortest first", desc: "Longest first" },
};

export const FILTER_LABELS: Record<WatchFilter, string> = {
  all: "All",
  unwatched: "Unwatched",
  in_progress: "In progress",
  watched: "Watched",
};

export const RESOLUTION_LABELS: Record<Resolution, string> = { "4k": "4K", "1080p": "1080p", "720p": "720p", sd: "SD" };

export function hasGroups(sort: LibrarySort): boolean {
  return sort === "title" || sort === "year" || sort === "added";
}

/** Group headers show only when the sort has groups and the viewer left them on. */
export function showsGroups(view: LibraryView): boolean {
  return view.group && hasGroups(view.sort);
}

/** Any filter narrowing the grid, so an empty result can offer to clear them. */
export function isFiltered(view: LibraryView, movies: boolean): boolean {
  return view.filter !== "all" || (movies && (view.res !== null || view.hdr));
}

/** A new sort starts in its natural direction; everything else carries over. */
export function withSort(view: LibraryView, sort: LibrarySort): LibraryView {
  return { ...view, sort, order: DEFAULT_ORDER[sort] };
}

/** The search-param form: only what differs from the default, so a plain `/library/3` stays plain. */
export type LibrarySearch = Partial<Record<"sort" | "order" | "filter" | "res" | "hdr" | "group", string>>;

const pick = <T extends string>(options: readonly T[], value: unknown): T | undefined =>
  options.find((o) => o === value);

/**
 * Reads a view from search params or stored JSON. Unknown or malformed values
 * fall back to the default one by one, so a hand-edited URL never breaks the page.
 */
export function parseLibraryView(raw: Readonly<Record<string, unknown>>): LibraryView {
  const sort = pick(LIBRARY_SORTS, raw.sort) ?? DEFAULT_VIEW.sort;
  return {
    sort,
    order: pick(SORT_ORDERS, raw.order) ?? DEFAULT_ORDER[sort],
    filter: pick(WATCH_FILTERS, raw.filter) ?? DEFAULT_VIEW.filter,
    res: pick(RESOLUTIONS, raw.res) ?? null,
    hdr: raw.hdr === true || raw.hdr === "1" || raw.hdr === "true",
    group: !(raw.group === false || raw.group === "0" || raw.group === "false"),
  };
}

export function librarySearch(view: LibraryView): LibrarySearch {
  const search: LibrarySearch = {};
  if (view.sort !== DEFAULT_VIEW.sort) search.sort = view.sort;
  if (view.order !== DEFAULT_ORDER[view.sort]) search.order = view.order;
  if (view.filter !== DEFAULT_VIEW.filter) search.filter = view.filter;
  if (view.res) search.res = view.res;
  if (view.hdr) search.hdr = "1";
  if (!view.group) search.group = "0";
  return search;
}

/** True when the search params name any part of a view, so they win over the stored one. */
export function hasViewParams(raw: Readonly<Record<string, unknown>>): boolean {
  return ["sort", "order", "filter", "res", "hdr", "group"].some((k) => raw[k] !== undefined);
}

export function sameView(a: LibraryView, b: LibraryView): boolean {
  return a.sort === b.sort && a.order === b.order && a.filter === b.filter && a.res === b.res && a.hdr === b.hdr && a.group === b.group;
}

/** The server's query for a view. Grouping is the client's; resolution and HDR only exist for movies. */
export function libraryQuery(view: LibraryView, movies: boolean): Record<string, string> {
  const query: Record<string, string> = { sort: view.sort, order: view.order, filter: view.filter };
  if (movies && view.res) query.res = view.res;
  if (movies && view.hdr) query.hdr = "true";
  return query;
}

/** The key under which a device remembers a library's view. */
export function libraryViewStorageKey(libraryId: number): string {
  return `nuvora-library-view-${libraryId}`;
}
