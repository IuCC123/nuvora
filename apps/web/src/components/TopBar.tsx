import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useRouterState } from "@tanstack/react-router";
import { isScanRunning, queryKeys, type ScanStatus } from "@nuvora/core";
import { queries } from "../nuvora";
import { useStartScan } from "../lib/useStartScan";
import { useUser } from "../lib/session";
import { AccountMenu } from "./AccountMenu";
import { HomeCustomizer } from "./HomeCustomizer";
import { MarkIcon, RefreshIcon, SlidersIcon, SpinIcon } from "./Icon";
import { SearchBox } from "./SearchBox";
import { WatchTogether } from "./party/WatchTogether";

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
      <CustomizeHome />
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
      <WatchTogether />
      <AccountMenu />
    </header>
  );
}

/** Opens the home screen editor; shown on Home only. */
function CustomizeHome() {
  const home = useRouterState({ select: (s) => s.location.pathname === "/" });
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  if (!home) return null;
  const close = () => {
    setOpen(false);
    button.current?.focus();
  };
  return (
    <>
      <button ref={button} type="button" className="topbar-btn" onClick={() => setOpen(true)}>
        <SlidersIcon size={18} />
        <span className="topbar-btn-label">Customize</span>
      </button>
      {/* The top bar is its own stacking context, so the panel goes to the body to cover the page. */}
      {open && createPortal(<HomeCustomizer onClose={close} />, document.body)}
    </>
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
