import { useMutation } from "@tanstack/react-query";
import { queryKeys, watchStateKeys, type Episode, type ItemDetail, type WatchState } from "@nuvora/core";
import { client, queryClient } from "../nuvora";

const unwatched: WatchState = { position_ms: null, file_id: null, watched: false, play_count: 0, last_watched_at: null };

function marked(before: WatchState, watched: boolean): WatchState {
  if (!watched) return unwatched;
  // A bulk mark leaves an already finished episode's count alone, as the server does.
  if (before.watched && before.position_ms == null) return before;
  return { ...before, position_ms: null, file_id: null, watched: true, play_count: before.play_count + 1, last_watched_at: Date.now() };
}

/**
 * Mark as watched or unwatched: a movie, an episode, or every episode of a
 * season or show (`episodes` lists them, for the instant update). The detail
 * and any loaded episode list flip at once; home, rows, the grid and the
 * show's counts refetch when the server confirms, and a failure puts them back.
 */
export function useWatchedToggle(id: number, { showId, episodes }: { showId?: number; episodes?: readonly number[] } = {}) {
  return useMutation({
    mutationFn: (watched: boolean) => (watched ? client.markWatched(id) : client.markUnwatched(id)),
    onMutate: async (watched) => {
      const targets = new Set(episodes ?? [id]);
      await Promise.all([queryClient.cancelQueries({ queryKey: queryKeys.item(id) }), queryClient.cancelQueries({ queryKey: queryKeys.episodes })]);
      const item = queryClient.getQueryData<ItemDetail>(queryKeys.item(id));
      const lists = queryClient.getQueriesData<Episode[]>({ queryKey: queryKeys.episodes });
      if (item?.kind === "movie" || item?.kind === "episode") {
        const next: ItemDetail = { ...item, watch: marked(item.watch, watched) };
        queryClient.setQueryData(queryKeys.item(id), next);
      }
      for (const [key, list] of lists) {
        if (list?.some((e) => targets.has(e.id))) {
          queryClient.setQueryData<Episode[]>(key, list.map((e) => (targets.has(e.id) ? { ...e, watch: marked(e.watch, watched) } : e)));
        }
      }
      return { item, lists };
    },
    onError: (_error, _watched, context) => {
      if (context?.item) queryClient.setQueryData(queryKeys.item(id), context.item);
      for (const [key, list] of context?.lists ?? []) queryClient.setQueryData(key, list);
    },
    onSettled: () => Promise.all(watchStateKeys(id, showId).map((queryKey) => queryClient.invalidateQueries({ queryKey }))),
  });
}
