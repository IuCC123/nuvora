import { useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@nuvora/core";
import { client } from "../nuvora";

/** POST /api/scan. The returned status goes straight into the cache, so the top bar starts polling at once. */
export function useStartScan() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => client.startScan(),
    onSuccess: (next) => qc.setQueryData(queryKeys.scan, next),
  });
}
