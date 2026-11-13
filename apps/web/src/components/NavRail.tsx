import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useRouterState } from "@tanstack/react-router";
import { queries } from "../nuvora";
import { useUser } from "../lib/session";
import { ActivityIcon, CaptionsIcon, DownloadIcon, FolderIcon, HouseIcon, SearchIcon, ServerIcon, SlidersIcon, UserIcon, UsersIcon } from "./Icon";
import { LibraryIcon } from "./LibraryIcon";

type Target = { to: "/" | "/search" | "/downloads" } | { to: "/library/$id"; id: number };
type Tab = { key: string; label: string; icon: ReactNode; target: Target; current: boolean };

/** Home, each library, and Downloads for admins: the app's sections, shared by the top bar's pill and the phone tab bar. */
function useTabs(withSearch: boolean): Tab[] {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const { data: libraries = [] } = useQuery(queries.libraries());
  const admin = useUser()?.role === "admin";
  const tabs: Tab[] = [
    { key: "home", label: "Home", icon: <HouseIcon />, target: { to: "/" }, current: pathname === "/" },
    ...libraries.map((library) => ({
      key: `library-${library.id}`,
      label: library.name,
      icon: <LibraryIcon icon={library.icon} kind={library.kind} />,
      target: { to: "/library/$id" as const, id: library.id },
      current: pathname === `/library/${library.id}`,
    })),
  ];
  if (withSearch) tabs.push({ key: "search", label: "Search", icon: <SearchIcon />, target: { to: "/search" }, current: pathname === "/search" });
  if (admin) tabs.push({ key: "downloads", label: "Downloads", icon: <DownloadIcon />, target: { to: "/downloads" }, current: pathname === "/downloads" });
  return tabs;
}

function TabLink({ tab, className, children, refFn }: { tab: Tab; className: string; children: ReactNode; refFn?: (el: HTMLAnchorElement | null) => void }) {
  const current = tab.current ? "page" : undefined;
  return tab.target.to === "/library/$id" ? (
    <Link ref={refFn} to="/library/$id" params={{ id: tab.target.id }} className={className} aria-current={current}>
      {children}
    </Link>
  ) : (
    <Link ref={refFn} to={tab.target.to} className={className} aria-current={current}>
      {children}
    </Link>
  );
}

/**
 * The glass pill in the top bar. A white indicator slides under the current
 * section on a spring; it hides when no section is current (a movie page).
 */
export function SectionPill() {
  const tabs = useTabs(false);
  const items = useRef(new Map<string, HTMLAnchorElement>());
  const [indicator, setIndicator] = useState<{ x: number; w: number } | null>(null);
  const current = tabs.find((t) => t.current)?.key;
  useLayoutEffect(() => {
    const el = current ? items.current.get(current) : undefined;
    setIndicator(el ? { x: el.offsetLeft, w: el.offsetWidth } : null);
  }, [current, tabs.length]);
  return (
    <nav className="section-pill glass" aria-label="Main">
      <span
        className="section-indicator"
        aria-hidden
        data-on={indicator != null}
        style={indicator ? { transform: `translateX(${indicator.x}px)`, width: indicator.w } : undefined}
      />
      {tabs.map((t) => (
        <TabLink key={t.key} tab={t} className="section-tab" refFn={(el) => void (el ? items.current.set(t.key, el) : items.current.delete(t.key))}>
          {t.label}
        </TabLink>
      ))}
    </nav>
  );
}

/** Phones: the sections and Search in a floating glass bar at the bottom, at most five. */
export function TabBar() {
  const tabs = useTabs(true);
  const shown = tabs.length <= 5 ? tabs : [...tabs.slice(0, 3), ...tabs.filter((t) => t.key === "search" || t.key === "downloads")].slice(0, 5);
  return (
    <nav className="tab-bar glass" aria-label="Sections">
      {shown.map((t) => (
        <TabLink key={t.key} tab={t} className="tab-bar-item">
          {t.icon}
          <span>{t.label}</span>
        </TabLink>
      ))}
    </nav>
  );
}

const SECTIONS = [
  { to: "/settings/general", label: "General", icon: ServerIcon, admin: true },
  { to: "/settings/libraries", label: "Libraries", icon: FolderIcon, admin: true },
  { to: "/settings/transcoding", label: "Transcoding", icon: SlidersIcon, admin: true },
  { to: "/settings/subtitles", label: "Subtitles", icon: CaptionsIcon, admin: true },
  { to: "/settings/downloads", label: "Downloads", icon: DownloadIcon, admin: true },
  { to: "/settings/activity", label: "Activity", icon: ActivityIcon, admin: true },
  { to: "/settings/users", label: "Users", icon: UsersIcon, admin: true },
  { to: "/settings/account", label: "Account", icon: UserIcon, admin: false },
] as const;

/** Settings' sections beside the page; a scrolling row of pills on phones. Members see only Account. */
export function SettingsRail() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const admin = useUser()?.role === "admin";
  return (
    <nav className="rail" aria-label="Settings">
      <p className="rail-group" id="rail-settings">
        Settings
      </p>
      <ul role="list" aria-labelledby="rail-settings">
        {SECTIONS.filter((s) => admin || !s.admin).map(({ to, label, icon: Icon }) => (
          <li key={to}>
            <Link to={to} className="rail-item" aria-current={pathname === to ? "page" : undefined}>
              <Icon />
              <span className="rail-label">{label}</span>
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}
