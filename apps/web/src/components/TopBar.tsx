import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { isScanRunning, queryKeys, type ScanStatus } from "@nuvora/core";
import { queries } from "../nuvora";
import { useStartScan } from "../lib/useStartScan";
import { useUser } from "../lib/session";
import { AccountMenu } from "./AccountMenu";
import { HomeCustomizer } from "./HomeCustomizer";
import { MarkIcon, RefreshIcon, SearchIcon, SlidersIcon, SpinIcon } from "./Icon";
import { SectionPill } from "./NavRail";
import { WatchTogether } from "./party/WatchTogether";

const PHASE_LABEL: Record<Extract<ScanStatus, { state: "running" }>["phase"], string> = {
  discovering: "Finding files",
  reading: "Reading files",
  artwork: "Making artwork",
  cleaning_up: "Cleaning up",
};

/**
 * Floats over the screen: clear over a hero, glass once the page scrolls.
 * The wordmark, the section pill, then Search, scan status, Watch Together
 * and the account. `/` or Ctrl/⌘+K opens Search from anywhere.
 */
export function TopBar() {
  const { status, startScan, starting } = useScan();
  const admin = useUser()?.role === "admin";
  const scrolled = useScrolled();
  useSearchShortcut();
  return (
    <header className="topbar" data-scrolled={scrolled}>
      <Link to="/" className="wordmark" aria-label="Nuvora, Home">
        <MarkIcon />
        <span aria-hidden>nuvora</span>
      </Link>
      <SectionPill />
      <div className="topbar-end">
        <Link to="/search" className="icon-btn topbar-round glass-thin" aria-label="Search" title="Search (/)">
          <SearchIcon size={20} />
        </Link>
        <CustomizeHome />
        {status?.state === "running" && (
          <div className="scan-pill glass-thin">
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
          <button type="button" className="icon-btn topbar-round topbar-scan glass-thin" onClick={startScan} aria-busy={starting} aria-label="Scan library" title="Scan library">
            <RefreshIcon />
          </button>
        )}
        <WatchTogether />
        <AccountMenu />
      </div>
    </header>
  );
}

/** True once the page has scrolled a little, read once per frame at most. */
function useScrolled() {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    let frame = 0;
    const read = () => {
      frame = 0;
      setScrolled(window.scrollY > 8);
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(read);
    };
    read();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      window.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(frame);
    };
  }, []);
  return scrolled;
}

/** `/` (outside a text field) or Ctrl/⌘+K opens Search; the Search screen focuses its field. */
function useSearchShortcut() {
  const navigate = useNavigate();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName));
      const slash = e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey;
      const command = e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey);
      if (!slash && !command) return;
      e.preventDefault();
      const field = document.querySelector<HTMLInputElement>("#search-field");
      if (field) {
        field.focus();
        field.select();
      } else {
        void navigate({ to: "/search" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);
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
      <button ref={button} type="button" className="icon-btn topbar-round glass-thin" onClick={() => setOpen(true)} aria-label="Customize Home" title="Customize Home">
        <SlidersIcon size={20} />
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
