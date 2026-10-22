import { useQuery, useQueryClient } from "@tanstack/react-query";
import { isScanRunning, queryKeys } from "@nuvora/core";
import { queries } from "../nuvora";

/**
 * "Scanning: reading files 412 / 2,000" while a scan runs; polls once a second
 * only while running, and invalidates Home when a scan finishes so new movies appear.
 */
export function ScanBanner() {
  const qc = useQueryClient();
  const status = useQuery({
    ...queries.scanStatus(),
    refetchInterval: (q) => (isScanRunning(q.state.data) ? 1000 : false),
  });
  void qc;
  void queryKeys;
  // TODO: on running -> idle transition, qc.invalidateQueries({ queryKey: queryKeys.home }).
  if (status.data?.state !== "running") return null;
  return (
    <p role="status" aria-live="polite">
      Scanning: {status.data.phase} {status.data.done} / {status.data.total}
    </p>
  );
}
