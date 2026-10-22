import type {
  AdminLibrary,
  AdminUser,
  AuthState,
  Episode,
  FsInspect,
  FsListing,
  HomeResponse,
  HomeRow,
  ItemDetail,
  LibraryIndex,
  LibraryPage,
  LibrarySummary,
  NuvoraClient,
  PosterCard,
  RowKind,
  RowPage,
  ScanStatus,
  MetadataSettings,
  ServerInfo,
} from "./client";
import type { AbortSignalLike } from "./transport";

/**
 * Cache policy, shared by every app. These are plain objects in TanStack
 * Query's options shape (`queryKey`, `queryFn`, `staleTime`), so apps pass them
 * straight to `useQuery` / `prefetchQuery` and this package needs no React or
 * TanStack import.
 */
export interface QueryDef<T> {
  readonly queryKey: readonly unknown[];
  readonly queryFn: (ctx: { signal: AbortSignalLike }) => Promise<T>;
  readonly staleTime: number;
}

/** TanStack's infinite-query shape, for rows that load more pages as they scroll. */
export interface InfiniteQueryDef<T, P> {
  readonly queryKey: readonly unknown[];
  readonly queryFn: (ctx: { signal: AbortSignalLike; pageParam: P }) => Promise<T>;
  readonly initialPageParam: P;
  readonly getNextPageParam: (lastPage: T) => P | undefined;
  readonly staleTime: number;
}

export const queryKeys = {
  home: ["home"] as const,
  /** Prefix of every row page query; invalidate it together with `home`. */
  rows: ["row"] as const,
  item: (id: number) => ["item", id] as const,
  /** Prefix of every season's episode list. */
  episodes: ["episodes"] as const,
  libraries: ["libraries"] as const,
  /** Prefix of every library grid page and index; invalidate it when watch state or the catalog changes. */
  library: ["library"] as const,
  scan: ["scan"] as const,
  /** Who is signed in. Set it from the login, setup and logout responses instead of refetching. */
  auth: ["auth"] as const,
  /** Prefix of every admin query; a sign-out drops them all. */
  admin: ["admin"] as const,
  adminLibraries: ["admin", "libraries"] as const,
  adminFs: (path: string | null) => ["admin", "fs", path] as const,
  adminInspect: (path: string) => ["admin", "inspect", path] as const,
  adminUsers: ["admin", "users"] as const,
  adminServer: ["admin", "server"] as const,
  adminMetadata: ["admin", "metadata"] as const,
};

/**
 * Everything that shows one user's watch state for an item: refresh these
 * after a report or a mark. An episode's state also shows on its show (pass
 * `showId`), its season's list and the library grid's show card.
 */
export function watchStateKeys(id: number, showId?: number): (readonly unknown[])[] {
  return [
    queryKeys.home,
    queryKeys.rows,
    queryKeys.item(id),
    queryKeys.episodes,
    queryKeys.library,
    ...(showId === undefined ? [] : [queryKeys.item(showId)]),
  ];
}

/** Library grid cards per request. Enough for two screens on a wide window, small enough to arrive at once. */
export const LIBRARY_PAGE_SIZE = 60;

export interface Queries {
  /** Fresh for 30 s; then revalidated in the background (ETag => 304 on the wire). */
  home(): QueryDef<HomeResponse>;
  /** Prefetch on card focus/hover so the detail screen opens with data. */
  item(id: number): QueryDef<ItemDetail>;
  /**
   * The pages of a home row after its first, starting at `after` (the row's
   * `next`). Keyed by that cursor, so a refreshed home with a new first page
   * starts paging again instead of appending stale pages.
   */
  rowPages(kind: RowKind, after: string, library?: number): InfiniteQueryDef<RowPage, string>;
  /** A season's episodes, with watch state and a playback decision each. */
  episodes(seasonId: number): QueryDef<Episode[]>;
  libraries(): QueryDef<LibrarySummary[]>;
  /** `query` is `libraryQuery(view, movies)`, so equal views share one cache entry. */
  libraryIndex(id: number, query: Record<string, string>): QueryDef<LibraryIndex>;
  /** One grid page from a cursor (`null`: the start). */
  libraryPage(id: number, query: Record<string, string>, after: string | null): QueryDef<LibraryPage>;
  /** Apps poll this while `isScanRunning`. */
  scanStatus(): QueryDef<ScanStatus>;
  /** Read before any screen decides where to go: welcome, sign in, or the app. */
  authState(): QueryDef<AuthState>;
  adminLibraries(): QueryDef<AdminLibrary[]>;
  /** One folder in the folder picker; `null` is the filesystem roots. */
  adminFs(path: string | null): QueryDef<FsListing>;
  /** The library type a folder looks like, for the add dialog's suggestion. */
  adminInspect(path: string): QueryDef<FsInspect>;
  adminUsers(): QueryDef<AdminUser[]>;
  adminServer(): QueryDef<ServerInfo>;
  adminMetadata(): QueryDef<MetadataSettings>;
}

/** Long enough that the screen's mount reuses its loader's request, short enough that a revisit shows other admins' changes. */
const ADMIN_FRESH = 5_000;

export function createQueries(client: NuvoraClient): Queries {
  return {
    home: () => ({ queryKey: queryKeys.home, queryFn: ({ signal }) => client.home(signal), staleTime: 30_000 }),
    item: (id) => ({ queryKey: queryKeys.item(id), queryFn: ({ signal }) => client.item(id, signal), staleTime: 30_000 }),
    rowPages: (kind, after, library) => ({
      queryKey: [...queryKeys.rows, kind, library ?? null, after],
      queryFn: ({ signal, pageParam }) => client.row(kind, pageParam, signal, library),
      initialPageParam: after,
      getNextPageParam: (last) => last.next ?? undefined,
      staleTime: 30_000,
    }),
    episodes: (id) => ({ queryKey: [...queryKeys.episodes, id], queryFn: ({ signal }) => client.episodes(id, signal), staleTime: 30_000 }),
    libraries: () => ({ queryKey: queryKeys.libraries, queryFn: ({ signal }) => client.libraries(signal), staleTime: 60_000 }),
    libraryIndex: (id, query) => ({
      queryKey: [...queryKeys.library, id, "index", query],
      queryFn: ({ signal }) => client.libraryIndex(id, query, signal),
      staleTime: 30_000,
    }),
    libraryPage: (id, query, after) => ({
      queryKey: [...queryKeys.library, id, "page", query, after],
      queryFn: ({ signal }) => client.libraryItems(id, query, after, LIBRARY_PAGE_SIZE, signal),
      staleTime: 30_000,
    }),
    scanStatus: () => ({ queryKey: queryKeys.scan, queryFn: ({ signal }) => client.scanStatus(signal), staleTime: 0 }),
    authState: () => ({ queryKey: queryKeys.auth, queryFn: ({ signal }) => client.authState(signal), staleTime: 60_000 }),
    adminLibraries: () => ({ queryKey: queryKeys.adminLibraries, queryFn: ({ signal }) => client.admin.libraries(signal), staleTime: ADMIN_FRESH }),
    adminFs: (path) => ({ queryKey: queryKeys.adminFs(path), queryFn: ({ signal }) => client.admin.fs(path, signal), staleTime: 30_000 }),
    adminInspect: (path) => ({ queryKey: queryKeys.adminInspect(path), queryFn: ({ signal }) => client.admin.inspect(path, signal), staleTime: 30_000 }),
    adminUsers: () => ({ queryKey: queryKeys.adminUsers, queryFn: ({ signal }) => client.admin.users(signal), staleTime: ADMIN_FRESH }),
    adminServer: () => ({ queryKey: queryKeys.adminServer, queryFn: ({ signal }) => client.admin.server(signal), staleTime: ADMIN_FRESH }),
    adminMetadata: () => ({ queryKey: queryKeys.adminMetadata, queryFn: ({ signal }) => client.admin.metadata(signal), staleTime: ADMIN_FRESH }),
  };
}

export function isScanRunning(status: ScanStatus | undefined): boolean {
  return status?.state === "running";
}

/**
 * The card the user just selected, from the cached home response, so the
 * detail screen paints title + poster placeholder in the first frame while the
 * item request is in flight.
 */
export function findCard(home: HomeResponse | undefined, id: number): PosterCard | undefined {
  for (const row of home?.rows ?? []) {
    const card = row.items.find((c) => c.id === id);
    if (card) return card;
  }
  return undefined;
}

/**
 * A home row's cards so far: its first page plus every page loaded after it.
 * `hasMore` is false once the last loaded page (or the first, if none) has no cursor.
 */
export function rowCards(row: HomeRow, pages: readonly RowPage[] = []): { cards: PosterCard[]; hasMore: boolean } {
  const last = pages.length ? pages[pages.length - 1]!.next : row.next;
  return { cards: [...row.items, ...pages.flatMap((p) => p.items)], hasMore: last != null };
}
