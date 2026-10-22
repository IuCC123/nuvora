import { createRootRoute, createRoute, createRouter, Outlet } from "@tanstack/react-router";
import { queries, queryClient } from "./nuvora";
import { Home } from "./screens/Home";
import { MovieScreen } from "./screens/Movie";
import { Player } from "./screens/Player";

/**
 * Three routes. Each loader starts its screen's single request on navigation
 * (and on link hover/focus via `defaultPreload: "intent"`), so data is usually
 * cached before the screen renders.
 *
 *   /                          Home
 *   /movie/$id                 Movie detail
 *   /movie/$id/play?file=<id>  Player (reads the cached detail; no new request)
 */
const root = createRootRoute({ component: Outlet });

const home = createRoute({
  getParentRoute: () => root,
  path: "/",
  loader: () => queryClient.ensureQueryData(queries.home()),
  component: Home,
});

const movie = createRoute({
  getParentRoute: () => root,
  path: "/movie/$id",
  params: { parse: (p) => ({ id: Number(p.id) }), stringify: (p) => ({ id: String(p.id) }) },
  // Not awaited: the screen paints from the cached home card at once and fills
  // in when the item arrives (see `findCard`).
  loader: ({ params }) => void queryClient.prefetchQuery(queries.item(params.id)),
  component: MovieScreen,
});

const play = createRoute({
  getParentRoute: () => root,
  path: "/movie/$id/play",
  params: { parse: (p) => ({ id: Number(p.id) }), stringify: (p) => ({ id: String(p.id) }) },
  validateSearch: (s: Record<string, unknown>) => ({ file: Number(s.file) }),
  loader: ({ params }) => queryClient.ensureQueryData(queries.item(params.id)),
  component: Player,
});

export const router = createRouter({
  routeTree: root.addChildren([home, movie, play]),
  defaultPreload: "intent",
  defaultPreloadStaleTime: 0, // TanStack Query owns freshness
  scrollRestoration: true,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export { movie as movieRoute, play as playRoute };
