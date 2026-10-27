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
import { ApiError, librarySearch, parseLibraryView, safeRedirect, type User } from "@nuvora/core";
import { onUnauthorized, queries, queryClient } from "./nuvora";
import { forgetResponses, setSessionUser } from "./lib/session";
import { Shell } from "./components/Shell";
import { Home } from "./screens/Home";
import { LibraryScreen } from "./screens/Library";
import { MovieScreen } from "./screens/Movie";
import { Search } from "./screens/Search";
import { Downloads } from "./screens/Downloads";
import { PartyLayer } from "./components/party/PartyLayer";
import { EpisodePlayer, Player } from "./screens/Player";
import { ShowScreen } from "./screens/Show";
import { ErrorState, MissingEpisode, MissingLibrary, MissingMovie, MissingShow } from "./screens/ErrorState";
import { playable, playState } from "./lib/playState";
import { SignIn } from "./screens/SignIn";
import { WelcomeAccount, WelcomeLibrary } from "./screens/Welcome";
import { SettingsAccount } from "./screens/settings/Account";
import { SettingsDownloads } from "./screens/settings/Downloads";
import { SettingsActivity } from "./screens/settings/Activity";
import { SettingsGeneral } from "./screens/settings/General";
import { SettingsLibraries } from "./screens/settings/Libraries";
import { SettingsTranscoding } from "./screens/settings/Transcoding";
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
 *   /downloads?q=               Find a movie on the indexers and download it (admins)
 *   /settings/<section>         general, libraries, transcoding, downloads, activity, users (admins), account
 */
/** Every screen, with Watch Together's invitations and follow-along under them. */
const root = createRootRoute({
  component: () => (
    <>
      <Outlet />
      <PartyLayer />
    </>
  ),
});

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

// The nav rail lists the libraries; its request starts with the screen's, so the rail paints with the page.
const shell = createRoute({
  getParentRoute: () => root,
  id: "shell",
  beforeLoad: requireUser,
  loader: () => void queryClient.prefetchQuery(queries.libraries()),
  component: Shell,
});

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
  // Parsed and written back, so a hand-edited URL settles on valid values and defaults drop out.
  validateSearch: (s: Record<string, unknown>) => librarySearch(parseLibraryView(s)),
  notFoundComponent: MissingLibrary,
  // The library's type decides which filters exist, so the list (small, shared with the rail) comes first.
  loader: async ({ params }) => {
    const libraries = await queryClient.ensureQueryData(queries.libraries());
    if (!libraries.some((l) => l.id === params.id)) throw notFound();
  },
  component: LibraryScreen,
  errorComponent: Unreachable,
});

const search = createRoute({
  getParentRoute: () => shell,
  path: "/search",
  validateSearch: (s: Record<string, unknown>): { q?: string } => (typeof s.q === "string" && s.q !== "" ? { q: s.q.slice(0, 200) } : {}),
  component: Search,
});

const downloads = createRoute({
  getParentRoute: () => shell,
  path: "/downloads",
  beforeLoad: requireAdmin,
  validateSearch: (s: Record<string, unknown>): { q?: string } => (typeof s.q === "string" && s.q !== "" ? { q: s.q.slice(0, 200) } : {}),
  loader: () => {
    void queryClient.prefetchQuery(queries.downloadSettings());
    void queryClient.prefetchQuery(queries.downloads());
  },
  component: Downloads,
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

const settingsTranscoding = createRoute({
  getParentRoute: () => settings,
  path: "/transcoding",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.adminTuning()),
  component: SettingsTranscoding,
});

const settingsDownloads = createRoute({
  getParentRoute: () => settings,
  path: "/downloads",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.downloadSettings()),
  component: SettingsDownloads,
});

const settingsActivity = createRoute({
  getParentRoute: () => settings,
  path: "/activity",
  beforeLoad: requireAdmin,
  loader: () => void queryClient.prefetchQuery(queries.adminActivity()),
  component: SettingsActivity,
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
    const resumable = item.versions.some((v) => v.file_id === file_id && playable(v));
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
    shell.addChildren([home, movie, show, library, search, downloads, settings.addChildren([
      settingsIndex,
      settingsGeneral,
      settingsLibraries,
      settingsTranscoding,
      settingsDownloads,
      settingsActivity,
      settingsUsers,
      settingsAccount,
    ])]),
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
