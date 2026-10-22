import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { isScanRunning, queryKeys, type ScanStatus } from "@nuvora/core";
import { queries } from "../nuvora";
import { useStartScan } from "../lib/useStartScan";
import { useUser } from "../lib/session";
import { AccountMenu } from "./AccountMenu";
import { MarkIcon, RefreshIcon, SpinIcon } from "./Icon";
import { SearchBox } from "./SearchBox";

const PHASE_LABEL: Record<Extract<ScanStatus, { state: "running" }>["phase"], string> = {
  discovering: "Finding files",
  reading: "Reading files",
  artwork: "Making artwork",
  cleaning_up: "Cleaning up",
};

export function TopBar() {
  const { status, startScan, starting } = useScan();
  const admin = useUser()?.role === "admin";
  return (
    <header className="topbar">
      <Link to="/" className="wordmark" aria-label="Nuvora, Home">
        <MarkIcon />
        <span aria-hidden>nuvora</span>
      </Link>
      <SearchBox />
      <div className="topbar-spacer" />
      {status?.state === "running" && (
        <div className="scan-pill">
          <SpinIcon className="scan-spin" />
          <span role="status">{PHASE_LABEL[status.phase]}</span>
          {status.total > 0 && (
            <>
              <span className="scan-count">
                {status.done} of {status.total}
              </span>
              <span
                className="scan-bar"
                role="progressbar"
                aria-label="Scan progress"
                aria-valuemin={0}
                aria-valuemax={status.total}
                aria-valuenow={status.done}
              >
                <span style={{ width: `${(100 * status.done) / status.total}%` }} />
              </span>
            </>
          )}
        </div>
      )}
      {admin && (
        <button type="button" className="icon-btn" onClick={startScan} aria-busy={starting} aria-label="Scan library" title="Scan library">
          <RefreshIcon />
        </button>
      )}
      <AccountMenu />
    </header>
  );
}

/**
 * Scan status, polled once a second only while a scan runs. Home and its row
 * pages refresh when the phase changes and when the scan ends, so new movies
 * fill in without a reload.
 */
function useScan() {
  const qc = useQueryClient();
  const { data: status } = useQuery({
    ...queries.scanStatus(),
    refetchInterval: (q) => (isScanRunning(q.state.data) ? 1000 : false),
  });
  const phase = status?.state === "running" ? status.phase : "idle";
  const previous = useRef(phase);
  useEffect(() => {
    if (previous.current === phase) return;
    const wasRunning = previous.current !== "idle";
    previous.current = phase;
    if (!wasRunning) return;
    void qc.invalidateQueries({ queryKey: queryKeys.home });
    void qc.invalidateQueries({ queryKey: queryKeys.rows });
  }, [phase, qc]);
  const start = useStartScan();
  return { status, startScan: () => start.mutate(), starting: start.isPending };
}
