import type { NuvoraClient, HomeResponse, ItemDetail, PosterCard, ScanStatus } from "./client";
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

export const queryKeys = {
  home: ["home"] as const,
  item: (id: number) => ["item", id] as const,
  scan: ["scan"] as const,
};

export interface Queries {
  /** Fresh for 30 s; then revalidated in the background (ETag => 304 on the wire). */
  home(): QueryDef<HomeResponse>;
  /** Prefetch on card focus/hover so the detail screen opens with data. */
  item(id: number): QueryDef<ItemDetail>;
  /** Apps poll this while `isScanRunning`. */
  scanStatus(): QueryDef<ScanStatus>;
}

export function createQueries(client: NuvoraClient): Queries {
  return {
    home: () => ({ queryKey: queryKeys.home, queryFn: ({ signal }) => client.home(signal), staleTime: 30_000 }),
    item: (id) => ({ queryKey: queryKeys.item(id), queryFn: ({ signal }) => client.item(id, signal), staleTime: 30_000 }),
    scanStatus: () => ({ queryKey: queryKeys.scan, queryFn: ({ signal }) => client.scanStatus(signal), staleTime: 0 }),
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
