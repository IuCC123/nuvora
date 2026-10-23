import type {
  AdminLibrary,
  AdminUser,
  AuthState,
  CreateLibraryRequest,
  CreateUserRequest,
  ErrorBody,
  FsInspect,
  FsListing,
  HttpMethod,
  LoginRequest,
  ServerInfo,
  SetupRequest,
  UpdateLibraryRequest,
  UpdateMeRequest,
  UpdateServerRequest,
  UpdateUserRequest,
  User,
} from "@nuvora/core";
import { checkDisplayName, checkPassword, checkUsername } from "@nuvora/core";

/** The server's display name rule, which also refuses control characters the form doesn't check for. */
function invalidDisplayName(name: string): Reply | null {
  const invalid = checkDisplayName(name, "a name") ?? (/\p{Cc}/u.test(name) ? "Use 1 to 64 characters" : null);
  return invalid ? fail(400, "validation", invalid, "display_name") : null;
}

/**
 * The accounts half of the fixture server: users, the session, libraries, the
 * folder tree and server settings, with the real server's rules (last admin,
 * no self-disable, rate limit, setup once). State lives in sessionStorage, so
 * a reload keeps you signed in. `?fixtures=<scenario>` on any URL starts over:
 *
 *   welcome     no users yet: the two-step welcome
 *   signed-out  users exist, nobody signed in: Sign in (jamie / password1)
 *   admin       signed in as the admin (the default)
 *   member      signed in as a member
 *   empty       signed in as the admin, with no libraries (an empty Home)
 *   member-empty  signed in as a member, with no libraries
 *
 * Sample content: names and folders are invented.
 */

export interface Reply {
  status: number;
  body?: unknown;
}

type StoredUser = AdminUser & { password: string } & Partial<Pick<User, "subtitle_mode" | "subtitle_language">>;

interface State {
  users: StoredUser[];
  session: number | null;
  serverName: string;
  libraries: AdminLibrary[];
  nextId: number;
  failedLogins: number;
}

const STORAGE_KEY = "nuvora-fixtures";
const DAY = 86_400_000;
const STARTED = Date.now() - 3 * DAY - 4 * 3_600_000;

function scenario(name: string): State {
  const now = Date.now();
  const users: StoredUser[] = [
    { id: 1, username: "jamie", display_name: "Jamie Rivera", role: "admin", disabled: false, created_at: now - 40 * DAY, last_seen_at: now - 60_000, password: "password1" },
    { id: 2, username: "sam", display_name: "Sam Okafor", role: "member", disabled: false, created_at: now - 31 * DAY, last_seen_at: now - 3 * 3_600_000, password: "password1" },
    { id: 3, username: "noor", display_name: "Noor Haddad", role: "member", disabled: false, created_at: now - 12 * DAY, last_seen_at: now - 30 * 3_600_000, password: "password1" },
    { id: 4, username: "grandpa.lu", display_name: "Grandpa Lu", role: "member", disabled: true, created_at: now - 90 * DAY, last_seen_at: null, password: "password1" },
  ];
  const libraries: AdminLibrary[] = [
    { id: 1, name: "Movies", kind: "movies", paths: ["/media/Movies"], item_count: 14, missing_paths: [] },
    { id: 2, name: "4K Movies", kind: "movies", paths: ["/mnt/nas/Films 4K"], item_count: 3, missing_paths: ["/mnt/nas/Films 4K"] },
    { id: 3, name: "TV Shows", kind: "shows", paths: ["/media/TV Shows"], item_count: 3, missing_paths: [] },
    { id: 4, name: "Film Archive", kind: "movies", paths: ["/mnt/nas/Films"], item_count: 592, missing_paths: [] },
  ];
  const base = { serverName: "Rivera Home", nextId: 10, failedLogins: 0 };
  switch (name) {
    case "welcome":
      return { ...base, users: [], session: null, libraries: [], serverName: "Nuvora" };
    case "signed-out":
      return { ...base, users, session: null, libraries };
    case "member":
      return { ...base, users, session: 2, libraries };
    case "empty":
      return { ...base, users, session: 1, libraries: [] };
    case "member-empty":
      return { ...base, users, session: 2, libraries: [] };
    default:
      return { ...base, users, session: 1, libraries };
  }
}

const state: State = load();

function load(): State {
  const requested = new URLSearchParams(location.search).get("fixtures");
  const stored = sessionStorage.getItem(STORAGE_KEY);
  const next = requested || !stored ? scenario(requested ?? "admin") : (JSON.parse(stored) as State);
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  return next;
}

function save() {
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

/** The folders the picker can browse. Every other path is unreadable. */
const FOLDERS: Record<string, string[]> = {
  "/": ["bin", "etc", "home", "media", "mnt", "opt", "srv", "usr", "var"],
  "/home": ["jamie"],
  "/home/jamie": ["Downloads", "Videos"],
  "/home/jamie/Downloads": [],
  "/home/jamie/Videos": ["Holidays", "Phone"],
  "/media": ["Anime", "Concerts", "Documentaries", "Home Videos", "Kids", "Movies", "Music", "TV Shows"],
  "/media/Anime": ["Harbor Lights", "The Long Field"],
  "/media/Movies": ["Big Buck Bunny (2008)", "Sintel (2010)", "Spring (2019)", "Tears of Steel (2012)"],
  "/media/Kids": [],
  "/media/Documentaries": ["Nature", "Space"],
  "/media/Concerts": [],
  "/media/Home Videos": ["2019", "2020", "2021", "2022", "2023", "2024", "2025"],
  "/media/Music": [],
  "/media/TV Shows": ["Charge", "Sprite Fright"],
  "/mnt": ["nas"],
  "/mnt/nas": ["Backups", "Films", "Photos"],
  "/mnt/nas/Films": [],
  "/srv": [],
};

function listFolder(path: string | null): Reply {
  const at = path ?? "/";
  const dirs = FOLDERS[at.length > 1 ? at.replace(/\/+$/, "") : at];
  if (!dirs) return fail(400, "validation", "Nuvora can't read that folder.", "path");
  const parent = at === "/" ? null : at.slice(0, at.lastIndexOf("/")) || "/";
  const join = (name: string) => (at === "/" ? `/${name}` : `${at}/${name}`);
  return ok({ path: at, parent, dirs: dirs.map((name) => ({ name, path: join(name) })) } satisfies FsListing);
}

/** What `/api/admin/fs/inspect` finds under a folder: video files, and how many are named like episodes. */
const VIDEOS: Record<string, [videos: number, episodeLike: number]> = {
  "/media/Anime": [74, 74],
  "/media/TV Shows": [164, 158],
  "/media/Movies": [14, 0],
  "/media/Documentaries": [22, 3],
  "/media/Home Videos": [312, 0],
  "/mnt/nas/Films": [592, 0],
};

function inspect(path: string | null): Reply {
  if (!path || !FOLDERS[path.replace(/\/+$/, "") || "/"]) return fail(400, "validation", "Nuvora can't read that folder.", "path");
  const [video_files, episode_like] = Object.entries(VIDEOS)
    .filter(([p]) => p === path || p.startsWith(`${path}/`))
    .reduce(([v, e], [, [dv, de]]) => [v + dv, e + de], [0, 0]);
  return ok({ video_files, episode_like, suggested_kind: episode_like * 2 > video_files ? "shows" : "movies", truncated: false } satisfies FsInspect);
}

const ok = (body?: unknown): Reply => (body === undefined ? { status: 204 } : { status: 200, body });

function fail(status: number, code: ErrorBody["code"], message: string, field?: string): Reply {
  return { status, body: { code, message, field: field ?? null } satisfies ErrorBody };
}

const publicUser = ({ id, username, display_name, role, subtitle_mode, subtitle_language }: StoredUser): User => ({
  id,
  username,
  display_name,
  role,
  subtitle_mode: subtitle_mode ?? "auto",
  subtitle_language: subtitle_language ?? null,
});
const adminUser = ({ password: _, subtitle_mode: _m, subtitle_language: _l, ...user }: StoredUser): AdminUser => user;
const me = () => state.users.find((u) => u.id === state.session && !u.disabled);
const enabledAdmins = () => state.users.filter((u) => u.role === "admin" && !u.disabled);

/** For the browser console and scripted checks: end the session as if it expired on the server. */
export function expireSession() {
  state.session = null;
  save();
}

export function libraries(): readonly AdminLibrary[] {
  return state.libraries;
}

export function hasLibraries(): boolean {
  return state.libraries.length > 0;
}

export function signedInAsAdmin(): boolean {
  return me()?.role === "admin";
}

export function signedIn(): boolean {
  return me() !== undefined;
}

/** Answers the accounts and admin routes; `undefined` for any other path. */
export function accountRoute(method: HttpMethod, path: string, query: URLSearchParams, body: unknown): Reply | undefined {
  const reply = route(method, path, query, body);
  if (method !== "GET") save();
  return reply;
}

function route(method: HttpMethod, path: string, query: URLSearchParams, body: unknown): Reply | undefined {
  if (path === "/api/health") return ok({ status: "ok", version: "0.1.0" });
  if (path === "/api/auth/state") {
    const user = me();
    return ok({ setup_required: state.users.length === 0, user: user ? publicUser(user) : null, server_name: state.serverName, server_id: "fixture-server" } satisfies AuthState);
  }
  if (path === "/api/setup" && method === "POST") return setup(body as SetupRequest);
  if (path === "/api/auth/login" && method === "POST") return login(body as LoginRequest);
  if (!path.startsWith("/api/auth/") && !path.startsWith("/api/admin/")) return undefined;

  const user = me();
  if (!user) return fail(401, "unauthorized", "Sign in to continue.");
  if (path === "/api/auth/logout" && method === "POST") {
    state.session = null;
    return ok();
  }
  if (path === "/api/auth/me" && method === "PATCH") return updateMe(user, body as UpdateMeRequest);
  if (user.role !== "admin") return fail(403, "forbidden", "Only an admin can do that.");
  return adminRoute(user, method, path, query, body);
}

function setup(req: SetupRequest): Reply {
  if (state.users.length > 0) return fail(409, "setup_done", "This server is already set up.");
  const badName = invalidDisplayName(req.display_name);
  if (badName) return badName;
  const invalid = checkUsername(req.username) ?? checkPassword(req.password);
  if (invalid) return fail(400, "validation", invalid, checkUsername(req.username) ? "username" : "password");
  const user: StoredUser = { id: state.nextId++, ...req, role: "admin", disabled: false, created_at: Date.now(), last_seen_at: Date.now() };
  state.users.push(user);
  state.session = user.id;
  return ok(publicUser(user));
}

function login(req: LoginRequest): Reply {
  if (state.failedLogins >= 5) return fail(429, "rate_limited", "Too many attempts.");
  const user = state.users.find((u) => u.username.toLowerCase() === req.username.toLowerCase());
  if (!user || user.password !== req.password || user.disabled) {
    state.failedLogins++;
    return fail(401, "bad_credentials", "Wrong username or password.");
  }
  state.failedLogins = 0;
  state.session = user.id;
  user.last_seen_at = Date.now();
  return ok(publicUser(user));
}

function updateMe(user: StoredUser, req: UpdateMeRequest): Reply {
  if (req.new_password != null) {
    if (req.current_password !== user.password) return fail(400, "bad_credentials", "The current password is wrong.", "current_password");
    const weak = checkPassword(req.new_password);
    if (weak) return fail(400, "validation", weak, "new_password");
    user.password = req.new_password;
  }
  if (req.display_name != null) {
    const badName = invalidDisplayName(req.display_name);
    if (badName) return badName;
    user.display_name = req.display_name.trim();
  }
  if (req.subtitle_mode != null) user.subtitle_mode = req.subtitle_mode;
  if (req.subtitle_language !== undefined) {
    if (req.subtitle_language !== null && !/^[a-z]{3}$/.test(req.subtitle_language)) {
      return fail(400, "validation", "Use a three-letter ISO 639-2 code.", "subtitle_language");
    }
    user.subtitle_language = req.subtitle_language;
  }
  return ok(publicUser(user));
}

function adminRoute(self: StoredUser, method: HttpMethod, path: string, query: URLSearchParams, body: unknown): Reply | undefined {
  if (path === "/api/admin/fs") return listFolder(query.get("path"));
  if (path === "/api/admin/fs/inspect") return inspect(query.get("path"));
  if (path === "/api/admin/server") {
    if (method === "PATCH") state.serverName = (body as UpdateServerRequest).server_name.trim();
    return ok({
      server_name: state.serverName,
      version: "0.1.0",
      data_dir: "/var/lib/nuvora",
      bind: "0.0.0.0:8640",
      ffmpeg: { path: "/usr/bin/ffmpeg", version: "7.1.1" },
      ffprobe: { path: "/usr/bin/ffprobe", version: "7.1.1" },
      uptime_s: Math.floor((Date.now() - STARTED) / 1000),
    } satisfies ServerInfo);
  }
  if (path === "/api/admin/libraries") {
    if (method === "GET") return ok(state.libraries);
    const req = body as CreateLibraryRequest;
    const clash = checkPaths(req.paths, null);
    if (clash) return clash;
    const library: AdminLibrary = { id: state.nextId++, name: req.name.trim(), kind: req.kind, paths: req.paths, item_count: 0, missing_paths: [] };
    state.libraries.push(library);
    return ok(library);
  }
  const lib = /^\/api\/admin\/libraries\/(\d+)(\/scan)?$/.exec(path);
  if (lib) {
    const library = state.libraries.find((l) => l.id === Number(lib[1]));
    if (!library) return fail(404, "not_found", "No such library.");
    if (lib[2]) return ok({ state: "running", phase: "discovering", done: 0, total: 0, queued: false, last: null });
    if (method === "DELETE") {
      state.libraries = state.libraries.filter((l) => l !== library);
      return ok();
    }
    const req = body as UpdateLibraryRequest;
    if (req.paths) {
      const clash = checkPaths(req.paths, library.id);
      if (clash) return clash;
      library.paths = req.paths;
      library.missing_paths = library.missing_paths.filter((p) => req.paths!.includes(p));
    }
    if (req.name != null) library.name = req.name.trim();
    if (req.kind != null && req.kind !== library.kind) Object.assign(library, { kind: req.kind, item_count: 0 });
    return ok(library);
  }
  if (path === "/api/admin/users") {
    if (method === "GET") return ok(state.users.map(adminUser));
    const req = body as CreateUserRequest;
    if (state.users.some((u) => u.username.toLowerCase() === req.username.toLowerCase())) {
      return fail(409, "username_taken", "That username is taken.", "username");
    }
    const user: StoredUser = { id: state.nextId++, ...req, disabled: false, created_at: Date.now(), last_seen_at: null };
    state.users.push(user);
    return ok(adminUser(user));
  }
  const userMatch = /^\/api\/admin\/users\/(\d+)$/.exec(path);
  if (userMatch) {
    const user = state.users.find((u) => u.id === Number(userMatch[1]));
    if (!user) return fail(404, "not_found", "No such user.");
    const lastAdmin = user.role === "admin" && !user.disabled && enabledAdmins().length === 1;
    if (method === "DELETE") {
      if (user.id === self.id) return fail(400, "bad_request", "You can't delete yourself.");
      if (lastAdmin) return fail(409, "last_admin", "This is the last admin.");
      state.users = state.users.filter((u) => u !== user);
      return ok();
    }
    const req = body as UpdateUserRequest;
    if (req.disabled && user.id === self.id) return fail(400, "bad_request", "You can't disable yourself.");
    if (lastAdmin && (req.disabled || req.role === "member")) return fail(409, "last_admin", "This is the last admin.");
    if (req.display_name != null) user.display_name = req.display_name.trim();
    if (req.role != null) user.role = req.role;
    if (req.disabled != null) user.disabled = req.disabled;
    if (req.password != null) user.password = req.password;
    return ok(adminUser(user));
  }
  return undefined;
}

function checkPaths(paths: string[], libraryId: number | null): Reply | null {
  if (paths.length === 0) return fail(400, "validation", "Choose at least one folder.", "paths");
  const taken = state.libraries.find((l) => l.id !== libraryId && l.paths.some((p) => paths.includes(p)));
  return taken ? fail(400, "validation", `That folder is already in ${taken.name}.`, "paths") : null;
}
