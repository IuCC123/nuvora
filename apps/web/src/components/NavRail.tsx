import { useQuery } from "@tanstack/react-query";
import { Link, useRouterState } from "@tanstack/react-router";
import { queries } from "../nuvora";
import { useUser } from "../lib/session";
import { FilmIcon, FolderIcon, HouseIcon, ServerIcon, UserIcon, UsersIcon } from "./Icon";

/**
 * Home, then the libraries. Step 1 has one library and no library screen yet,
 * so Movies opens Home at its All Movies row and is current on movie pages.
 */
export function NavRail() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  return pathname.startsWith("/settings") ? <SettingsRail pathname={pathname} /> : <LibraryRail pathname={pathname} />;
}

function LibraryRail({ pathname }: { pathname: string }) {
  const { data: home } = useQuery(queries.home());
  const movies = home?.rows.find((r) => r.kind === "all_movies")?.total;
  return (
    <nav className="rail" aria-label="Main">
      <Link to="/" className="rail-item" aria-current={pathname === "/" ? "page" : undefined}>
        <HouseIcon />
        <span className="rail-label">Home</span>
      </Link>
      <p className="rail-group" id="rail-libraries">
        Libraries
      </p>
      <ul role="list" aria-labelledby="rail-libraries">
        <li>
          <Link
            to="/"
            hash="row-all_movies"
            className="rail-item"
            aria-current={pathname.startsWith("/movie/") ? "page" : undefined}
          >
            <FilmIcon />
            <span className="rail-label">Movies</span>
            {movies != null && <span className="rail-count">{movies}</span>}
          </Link>
        </li>
      </ul>
    </nav>
  );
}

const SECTIONS = [
  { to: "/settings/general", label: "General", icon: ServerIcon, admin: true },
  { to: "/settings/libraries", label: "Libraries", icon: FolderIcon, admin: true },
  { to: "/settings/users", label: "Users", icon: UsersIcon, admin: true },
  { to: "/settings/account", label: "Account", icon: UserIcon, admin: false },
] as const;

/** In Settings the rail lists its sections instead of the libraries; members see only Account. */
function SettingsRail({ pathname }: { pathname: string }) {
  const admin = useUser()?.role === "admin";
  return (
    <nav className="rail" aria-label="Settings">
      <Link to="/" className="rail-item">
        <HouseIcon />
        <span className="rail-label">Home</span>
      </Link>
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
