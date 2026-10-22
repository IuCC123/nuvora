import {
  createRootRoute,
  createRoute,
  createRouter,
  notFound,
  Outlet,
  redirect,
  useRouter,
  type ErrorComponentProps,
  type ParsedLocation,
} from "@tanstack/react-router";
import { ApiError, safeRedirect, type LibrarySearch, type User } from "@nuvora/core";
import { onUnauthorized, queries, queryClient } from "./nuvora";
import { forgetResponses, setSessionUser } from "./lib/session";
import { Shell } from "./components/Shell";
import { Home } from "./screens/Home";
import { LibraryScreen } from "./screens/Library";
import { MovieScreen } from "./screens/Movie";
import { EpisodePlayer, Player } from "./screens/Player";
import { ShowScreen } from "./screens/Show";
import { ErrorState, MissingEpisode, MissingLibrary, MissingMovie, MissingShow } from "./screens/ErrorState";
import { playState } from "./lib/playState";
import { SignIn } from "./screens/SignIn";
import { WelcomeAccount, WelcomeLibrary } from "./screens/Welcome";
import { SettingsAccount } from "./screens/settings/Account";
import { SettingsGeneral } from "./screens/settings/General";
import { SettingsLibraries } from "./screens/settings/Libraries";
import { SettingsUsers } from "./screens/settings/Users";

/**
 * Each loader starts its screen's single request on navigation (and on link
 * hover/focus via `defaultPreload: "intent"`), so data is usually cached before
 * the screen renders. Every app route first reads the auth state: with no users
 * it goes to the welcome, without a session to Sign in (and back afterwards).
 *
 *   /welcome, /welcome/library  First run: create the admin, add a library
 *   /signin?redirect=<path>     Sign in
 *   /                           Home
 *   /movie/$id                  Movie detail
 *   /movie/$id/play?file=<id>&t=<s>  Player (reads the cached detail; no new request), from second `t`
 *   /show/$id?season=<n>        Show: seasons and episodes, season `n` open
 *   /library/$id?sort=&order=&filter=&res=&hdr=&group=  Library grid (view also kept per device)
 *   /episode/$id/play?file=&t=  Player for an episode; without `file` it resumes where the viewer stopped
 *   /settings/<section>         general, libraries, users (admins), account
 */
const root = createRootRoute({ component: Outlet });

const authState = () => queryClient.ensureQueryData(queries.authState());

async function requireUser({ location }: { location: ParsedLocation }): Promise<{ user: User }> {
  const auth = await authState();
  if (auth.setup_required) throw redirect({ to: "/welcome" });
  if (!auth.user) throw redirect({ to: "/signin", search: { redirect: location.href } });
  return { user: auth.user };
}

function requireAdmin({ context }: { context: { user: User } }) {
  if (context.user.role !== "admin") throw redirect({ to: "/settings/account" });
}

/** A 401 renders nothing: the client has already sent the person to Sign in. */
const signedOut = (error: unknown) => error instanceof ApiError && error.status === 401;

/** A route's loader failed. Try again re-runs the loaders; the boundary's own `reset` would only re-render the error. */
function Unreachable({ error }: ErrorComponentProps) {
  const router = useRouter();
  if (signedOut(error)) return null;
  return <ErrorState title="Can't reach your Nuvora server" detail="Check that it's running, then try again." retry={() => void router.invalidate()} />;
}

/** `/movie/abc` names no movie, so it gets the missing-movie screen instead of a request for `NaN`. */
const movieParams = {
  parse: (p: { id: string }) => {
    const id = Number(p.id);
    if (!Number.isSafeInteger(id) || id <= 0) throw notFound();
    return { id };
  },
  stringify: (p: { id: number }) => ({ id: String(p.id) }),
};

/** Search values arrive parsed (`?hdr=1` is a number); the view model reads strings. */
const VIEW_KEYS = ["sort", "order", "filter", "res", "hdr", "group"] as const;
function viewSearch(s: Record<string, unknown>): LibrarySearch {
  const search: LibrarySearch = {};
  for (const key of VIEW_KEYS) if (s[key] !== undefined && s[key] !== null) search[key] = String(s[key]);
  return search;
}

function playSearch(s: Record<string, unknown>): { t?: number | undefined } {
  const t = Math.floor(Number(s.t));
  return { t: Number.isFinite(t) && t > 0 ? t : undefined };
}

const welcome = createRoute({
  getParentRoute: () => root,
  path: "/welcome",
  beforeLoad: async () => {
    const auth = await authState();
    if (!auth.setup_required) throw redirect({ to: auth.user ? "/" : "/signin" });
  },
  component: WelcomeAccount,
});

const welcomeLibrary = createRoute({
  getParentRoute: () => root,
  path: "/welcome/library",
  beforeLoad: async (ctx) => requireAdmin({ context: await requireUser(ctx) }),
  component: WelcomeLibrary,
});

const signin = createRoute({
  getParentRoute: () => root,
  path: "/signin",
  validateSearch: (s: Record<string, unknown>): { redirect?: string } => (typeof s.redirect === "string" ? { redirect: s.redirect } : {}),
  beforeLoad: async ({ search }) => {
    const auth = await authState();
    if (auth.setup_required) throw redirect({ to: "/welcome" });
    if (auth.user) throw redirect({ href: safeRedirect(search.redirect) });
  },
  component: SignIn,
});

const shell = createRoute({ getParentRoute: () => root, id: "shell", beforeLoad: requireUser, component: Shell });

const home = createRoute({
  getParentRoute: () => shell,
  path: "/",
  loader: () => queryClient.ensureQueryData(queries.home()),
  component: Home,
  errorComponent: Unreachable,
});

const movie = createRoute({
  getParentRoute: () => shell,
  path: "/movie/$id",
  params: movieParams,
  notFoundComponent: MissingMovie,
  // Not awaited: the screen paints from the cached home card at once and fills
  // in when the item arrives (see `findCard`).
  loader: ({ params }) => void queryClient.prefetchQuery(queries.item(params.id)),
  component: MovieScreen,
});

const show = createRoute({
  getParentRoute: () => shell,
  path: "/show/$id",
  params: movieParams,
  validateSearch: (s: Record<string, unknown>): { season?: number | undefined } => {
    const season = Number(s.season);
    return { season: Number.isSafeInteger(season) && season >= 0 ? season : undefined };
  },
  notFoundComponent: MissingShow,
  loader: ({ params }) => void queryClient.prefetchQuery(queries.item(params.id)),
  component: ShowScreen,
});

const library = createRoute({
  getParentRoute: () => shell,
  path: "/library/$id",
  params: movieParams,
  validateSearch: viewSearch,
  notFoundComponent: MissingLibrary,
  // The library's type decides which filters exist, so the list (small, shared with the rail) comes first.
  loader: async ({ params }) => {
    const libraries = await queryClient.ensureQueryData(queries.libraries());
    if (!libraries.some((l) => l.id === params.id)) throw notFound();
  },
  component: LibraryScreen,
  errorComponent: Unreachable,
});

const settings = createRoute({ getParentRoute: () => shell, path: "/settings", component: Outlet });

const settingsIndex = createRoute({
  getParentRoute: () => settings,
  path: "/",
  beforeLoad: ({ context }) => {
    throw redirect({ to: context.user.role === "admin" ? "/settings/general" : "/settings/account", replace: true });
  },
});

const settingsGeneral = createRoute({
  getParentRoute: () => settings,
  path: "/general",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.adminServer()),
  component: SettingsGeneral,
});

const settingsLibraries = createRoute({
  getParentRoute: () => settings,
  path: "/libraries",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.adminLibraries()),
  component: SettingsLibraries,
});

const settingsUsers = createRoute({
  getParentRoute: () => settings,
  path: "/users",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.adminUsers()),
  component: SettingsUsers,
});

const settingsAccount = createRoute({ getParentRoute: () => settings, path: "/account", component: SettingsAccount });

const play = createRoute({
  getParentRoute: () => root,
  path: "/movie/$id/play",
  params: movieParams,
  // The router lays this over the raw search, so a rejected `t` has to be overwritten, not just left out.
  validateSearch: (s: Record<string, unknown>): { file: number; t?: number | undefined } => ({ file: Number(s.file), ...playSearch(s) }),
  beforeLoad: requireUser,
  loader: ({ params }) =>
    queryClient.ensureQueryData(queries.item(params.id)).catch((error: unknown) => {
      throw error instanceof ApiError && error.status === 404 ? notFound() : error;
    }),
  component: Player,
  notFoundComponent: MissingMovie,
  errorComponent: Unreachable,
});

/**
 * An episode plays from a card or button that knows only its id. Without a
 * `file`, the loader picks one the way the show page would (the version the
 * viewer stopped in, else the first playable) and redirects with it and the
 * resume point, so a reload resumes like any movie link.
 */
const playEpisode = createRoute({
  getParentRoute: () => root,
  path: "/episode/$id/play",
  params: movieParams,
  validateSearch: (s: Record<string, unknown>): { file?: number | undefined; t?: number | undefined } => {
    const file = Number(s.file);
    return { file: Number.isSafeInteger(file) && file > 0 ? file : undefined, ...playSearch(s) };
  },
  beforeLoad: requireUser,
  loaderDeps: ({ search }) => ({ file: search.file }),
  loader: async ({ params, deps }) => {
    const item = await queryClient.ensureQueryData(queries.item(params.id)).catch((error: unknown) => {
      throw error instanceof ApiError && error.status === 404 ? notFound() : error;
    });
    if (item.kind !== "episode") throw notFound();
    if (deps.file !== undefined) return;
    const { position_ms, file_id } = item.watch;
    const resumable = item.versions.some((v) => v.file_id === file_id && v.playback.kind === "direct_play");
    const state = playState(item.versions);
    const file = resumable ? file_id! : state.kind === "play" ? state.fileId : (item.versions[0]?.file_id ?? 0);
    const t = resumable && position_ms != null ? Math.floor(position_ms / 1000) : undefined;
    throw redirect({ to: "/episode/$id/play", params: { id: params.id }, search: { file, t }, replace: true });
  },
  component: EpisodePlayer,
  notFoundComponent: MissingEpisode,
  errorComponent: Unreachable,
});

export const router = createRouter({
  routeTree: root.addChildren([
    welcome,
    welcomeLibrary,
    signin,
    shell.addChildren([home, movie, show, library, settings.addChildren([settingsIndex, settingsGeneral, settingsLibraries, settingsUsers, settingsAccount])]),
    play,
    playEpisode,
  ]),
  defaultPreload: "intent",
  defaultPreloadStaleTime: 0, // TanStack Query owns freshness
  scrollRestoration: true,
});

onUnauthorized(() => {
  const { pathname, href } = router.state.location;
  if (pathname === "/signin" || pathname.startsWith("/welcome")) return;
  setSessionUser(null);
  void router.navigate({ to: "/signin", search: { redirect: href } }).then(forgetResponses);
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
