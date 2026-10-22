import { useQuery } from "@tanstack/react-query";
import { Link, useRouterState } from "@tanstack/react-router";
import { queries } from "../nuvora";
import { FilmIcon, HouseIcon } from "./Icon";

/**
 * Home, then the libraries. Step 1 has one library and no library screen yet,
 * so Movies opens Home at its All Movies row and is current on movie pages.
 */
export function NavRail() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
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
