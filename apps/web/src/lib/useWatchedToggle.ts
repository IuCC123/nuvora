import { useMutation } from "@tanstack/react-query";
import { queryKeys, watchStateKeys, type ItemDetail } from "@nuvora/core";
import { client, queryClient } from "../nuvora";

/**
 * Mark as watched or unwatched. The detail page flips at once; home and the
 * rows refetch when the server confirms, and a failure puts the detail back.
 */
export function useWatchedToggle(id: number) {
  return useMutation({
    mutationFn: (watched: boolean) => (watched ? client.markWatched(id) : client.markUnwatched(id)),
    onMutate: async (watched) => {
      const key = queryKeys.item(id);
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<ItemDetail>(key);
      if (before) {
        const watch = watched
          ? { ...before.watch, position_ms: null, file_id: null, watched: true, play_count: before.watch.play_count + 1, last_watched_at: Date.now() }
          : { position_ms: null, file_id: null, watched: false, play_count: 0, last_watched_at: null };
        queryClient.setQueryData<ItemDetail>(key, { ...before, watch });
      }
      return { before };
    },
    onError: (_error, _watched, context) => {
      if (context?.before) queryClient.setQueryData(queryKeys.item(id), context.before);
    },
    onSettled: () => Promise.all(watchStateKeys(id).map((queryKey) => queryClient.invalidateQueries({ queryKey }))),
  });
}
