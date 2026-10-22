import type { components, paths } from "./generated/schema";
import { ApiError, type AbortSignalLike, type FetchLike, type HttpMethod, type ResponseLike } from "./transport";

type Schemas = components["schemas"];
export type HomeResponse = Schemas["HomeResponse"];
export type HomeRow = Schemas["HomeRow"];
export type RowKind = Schemas["RowKind"];
export type RowPage = Schemas["RowPage"];
export type PosterCard = Schemas["PosterCard"];
export type ItemDetail = Schemas["ItemDetail"];
export type MovieDetail = Schemas["MovieDetail"];
export type Version = Schemas["Version"];
export type Playback = Schemas["Playback"];
export type Image = Schemas["Image"];
export type ImageSize = Schemas["ImageSize"];
export type ScanStatus = Schemas["ScanStatus"];
export type DeviceProfile = Schemas["DeviceProfile"];
export type AuthState = Schemas["AuthState"];
export type User = Schemas["User"];
export type Role = Schemas["Role"];
export type SetupRequest = Schemas["SetupRequest"];
export type LoginRequest = Schemas["LoginRequest"];
export type UpdateMeRequest = Schemas["UpdateMeRequest"];
export type AdminLibrary = Schemas["AdminLibrary"];
export type CreateLibraryRequest = Schemas["CreateLibraryRequest"];
export type UpdateLibraryRequest = Schemas["UpdateLibraryRequest"];
export type FsListing = Schemas["FsListing"];
export type AdminUser = Schemas["AdminUser"];
export type CreateUserRequest = Schemas["CreateUserRequest"];
export type UpdateUserRequest = Schemas["UpdateUserRequest"];
export type ServerInfo = Schemas["ServerInfo"];
export type UpdateServerRequest = Schemas["UpdateServerRequest"];
export type ErrorBody = Schemas["ErrorBody"];

export interface ClientOptions {
  /** Origin of the Nuvora server, e.g. `""` (same origin, web) or `"http://192.168.1.10:8640"` (TV). */
  baseUrl: string;
  fetch: FetchLike;
  /** Measured once at startup by the app (it needs platform APIs this package must not touch). */
  device: DeviceProfile;
  /**
   * Called when a request comes back 401 because the session ended or never
   * existed (not for a wrong password). The app sends the person to Sign in.
   * The request still rejects with its `ApiError`.
   */
  onUnauthorized?: () => void;
}

/**
 * One method per screen or action. Callers never build paths, set headers, or
 * parse JSON; the generated `paths` type checks every path literal below, so a
 * renamed route in Rust fails this file's typecheck after `bun run gen:api`.
 */
export interface NuvoraClient {
  home(signal?: AbortSignalLike): Promise<HomeResponse>;
  /** The page of a home row after `after` (a `next` from `home()` or a previous page). */
  row(kind: RowKind, after: string, signal?: AbortSignalLike): Promise<RowPage>;
  /** Rejects with `ApiError(404)` when the item does not exist. */
  item(id: number, signal?: AbortSignalLike): Promise<ItemDetail>;
  scanStatus(signal?: AbortSignalLike): Promise<ScanStatus>;
  /** Idempotent; returns the status after queuing. */
  startScan(): Promise<ScanStatus>;
  /** Never 401: without a session `user` is null. `setup_required` until the first admin exists. */
  authState(signal?: AbortSignalLike): Promise<AuthState>;
  /** Creates the first admin and signs them in. 409 `setup_done` once any user exists. */
  setup(body: SetupRequest): Promise<User>;
  /** 401 `bad_credentials` for any wrong detail; 429 `rate_limited` after five failures. */
  login(body: LoginRequest): Promise<User>;
  logout(): Promise<void>;
  /** The signed-in user's own name and password. A new password needs the current one. */
  updateMe(body: UpdateMeRequest): Promise<User>;
  /** Admin role only; anyone else gets 403. */
  admin: AdminClient;
  /** Absolute URL of an artwork size. Immutable: safe to cache forever. */
  imageUrl(image: Image, size: ImageSize): string;
  /** Absolute URL for a server-relative URL the API returned (e.g. `playback.url`). */
  resolve(url: string): string;
}

export interface AdminClient {
  libraries(signal?: AbortSignalLike): Promise<AdminLibrary[]>;
  /** Starts a scan. */
  createLibrary(body: CreateLibraryRequest): Promise<AdminLibrary>;
  /** Scans when `paths` changed. */
  updateLibrary(id: number, body: UpdateLibraryRequest): Promise<AdminLibrary>;
  /** Removes it and its items from the catalog; files on disk stay. */
  deleteLibrary(id: number): Promise<void>;
  scanLibrary(id: number): Promise<ScanStatus>;
  /** A folder's subfolders; `null` lists the filesystem roots. */
  fs(path: string | null, signal?: AbortSignalLike): Promise<FsListing>;
  users(signal?: AbortSignalLike): Promise<AdminUser[]>;
  createUser(body: CreateUserRequest): Promise<AdminUser>;
  /** 409 `last_admin` when the change would leave no enabled admin. */
  updateUser(id: number, body: UpdateUserRequest): Promise<AdminUser>;
  deleteUser(id: number): Promise<void>;
  server(signal?: AbortSignalLike): Promise<ServerInfo>;
  updateServer(body: UpdateServerRequest): Promise<ServerInfo>;
}

type Params = Record<string, string | number>;

/** The routes that answer `method`, so a call to a route without it fails the typecheck. */
type Path<M extends HttpMethod = HttpMethod> = {
  [P in keyof paths]: paths[P] extends Record<Lowercase<M>, object> ? P : never;
}[keyof paths];

export function createClient(options: ClientOptions): NuvoraClient {
  const headers = { accept: "application/json", "nuvora-device": JSON.stringify(options.device) };

  async function call<T>(
    method: HttpMethod,
    path: keyof paths,
    params: Params,
    signal?: AbortSignalLike,
    query?: Record<string, string>,
    body?: unknown,
  ): Promise<T> {
    const url = options.baseUrl + fill(path, params) + queryString(query);
    const init = {
      method,
      headers: method === "GET" ? headers : { ...headers, "content-type": "application/json" },
      ...(body !== undefined && { body: JSON.stringify(body) }),
      ...(signal && { signal }),
    };
    const res = await options.fetch(url, init);
    if (!res.ok) {
      const error = await apiError(res, `${method} ${url} -> ${res.status}`);
      if (res.status === 401 && error.code !== "bad_credentials") options.onUnauthorized?.();
      throw error;
    }
    if (res.status === 204) return undefined as T;
    // Trusted: the server's types generated this client. Parsing again here would
    // be a second schema to keep in sync.
    return (await res.json()) as T;
  }

  const get = <T>(path: Path<"GET">, params: Params = {}, signal?: AbortSignalLike, query?: Record<string, string>) =>
    call<T>("GET", path, params, signal, query);
  const post = <T>(path: Path<"POST">, params: Params = {}, body?: unknown) => call<T>("POST", path, params, undefined, undefined, body);
  const patch = <T>(path: Path<"PATCH">, params: Params, body: unknown) => call<T>("PATCH", path, params, undefined, undefined, body);
  const remove = (path: Path<"DELETE">, params: Params) => call<void>("DELETE", path, params);

  return {
    home: (signal) => get<HomeResponse>("/api/home", {}, signal),
    row: (kind, after, signal) => get<RowPage>("/api/rows/{row}", { row: kind }, signal, { after }),
    item: (id, signal) => get<ItemDetail>("/api/items/{id}", { id }, signal),
    scanStatus: (signal) => get<ScanStatus>("/api/scan", {}, signal),
    startScan: () => post<ScanStatus>("/api/scan"),
    authState: (signal) => get<AuthState>("/api/auth/state", {}, signal),
    setup: (body) => post<User>("/api/setup", {}, body),
    login: (body) => post<User>("/api/auth/login", {}, body),
    logout: () => post<void>("/api/auth/logout"),
    updateMe: (body) => patch<User>("/api/auth/me", {}, body),
    admin: {
      libraries: (signal) => get<AdminLibrary[]>("/api/admin/libraries", {}, signal),
      createLibrary: (body) => post<AdminLibrary>("/api/admin/libraries", {}, body),
      updateLibrary: (id, body) => patch<AdminLibrary>("/api/admin/libraries/{id}", { id }, body),
      deleteLibrary: (id) => remove("/api/admin/libraries/{id}", { id }),
      scanLibrary: (id) => post<ScanStatus>("/api/admin/libraries/{id}/scan", { id }),
      fs: (path, signal) => get<FsListing>("/api/admin/fs", {}, signal, path === null ? undefined : { path }),
      users: (signal) => get<AdminUser[]>("/api/admin/users", {}, signal),
      createUser: (body) => post<AdminUser>("/api/admin/users", {}, body),
      updateUser: (id, body) => patch<AdminUser>("/api/admin/users/{id}", { id }, body),
      deleteUser: (id) => remove("/api/admin/users/{id}", { id }),
      server: (signal) => get<ServerInfo>("/api/admin/server", {}, signal),
      updateServer: (body) => patch<ServerInfo>("/api/admin/server", {}, body),
    },
    imageUrl: (image, size) => options.baseUrl + fill("/api/images/{id}/{size}", { id: image.id, size }),
    resolve: (url) => (url.startsWith("/") ? options.baseUrl + url : url),
  };
}

function fill(path: keyof paths, params: Params): string {
  return path.replace(/\{(\w+)\}/g, (_, name: string) => encodeURIComponent(String(params[name])));
}

function queryString(query: Record<string, string> | undefined): string {
  if (!query) return "";
  const pairs = Object.entries(query).map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`);
  return pairs.length ? `?${pairs.join("&")}` : "";
}

/** Errors carry an `ErrorBody` (`{ code, message, field }`); a proxy or crash in front of the server may send anything. */
async function apiError(res: ResponseLike, message: string): Promise<ApiError> {
  const body = await res.json().catch(() => null);
  if (typeof body !== "object" || body === null) return new ApiError(res.status, message);
  const text = (name: keyof ErrorBody) => {
    const value = (body as Record<string, unknown>)[name];
    return typeof value === "string" ? value : null;
  };
  return new ApiError(res.status, message, text("code"), text("message"), text("field"));
}
