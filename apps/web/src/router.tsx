import { createRootRoute, createRoute, createRouter, Outlet, redirect, type ParsedLocation } from "@tanstack/react-router";
import { ApiError, safeRedirect, type User } from "@nuvora/core";
import { onUnauthorized, queries, queryClient } from "./nuvora";
import { forgetResponses, setSessionUser } from "./lib/session";
import { Shell } from "./components/Shell";
import { Home } from "./screens/Home";
import { MovieScreen } from "./screens/Movie";
import { Player } from "./screens/Player";
import { ErrorState } from "./screens/ErrorState";
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
 *   /movie/$id/play?file=<id>   Player (reads the cached detail; no new request)
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
  errorComponent: ({ error, reset }) =>
    signedOut(error) ? null : (
      <ErrorState title="Can't reach your Nuvora server" detail="Check that it's running, then try again." retry={reset} />
    ),
});

const movie = createRoute({
  getParentRoute: () => shell,
  path: "/movie/$id",
  params: { parse: (p) => ({ id: Number(p.id) }), stringify: (p) => ({ id: String(p.id) }) },
  // Not awaited: the screen paints from the cached home card at once and fills
  // in when the item arrives (see `findCard`).
  loader: ({ params }) => void queryClient.prefetchQuery(queries.item(params.id)),
  component: MovieScreen,
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
  params: { parse: (p) => ({ id: Number(p.id) }), stringify: (p) => ({ id: String(p.id) }) },
  validateSearch: (s: Record<string, unknown>) => ({ file: Number(s.file) }),
  beforeLoad: requireUser,
  loader: ({ params }) => queryClient.ensureQueryData(queries.item(params.id)),
  component: Player,
});

export const router = createRouter({
  routeTree: root.addChildren([
    welcome,
    welcomeLibrary,
    signin,
    shell.addChildren([home, movie, settings.addChildren([settingsIndex, settingsGeneral, settingsLibraries, settingsUsers, settingsAccount])]),
    play,
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
