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
  /** Absolute URL of an artwork size. Immutable: safe to cache forever. */
  imageUrl(image: Image, size: ImageSize): string;
  /** Absolute URL for a server-relative URL the API returned (e.g. `playback.url`). */
  resolve(url: string): string;
}

/** `/api/images/{id}/{size}` etc.: substitutes `{name}` segments. */
type Path = keyof paths;

export function createClient(options: ClientOptions): NuvoraClient {
  const headers = { accept: "application/json", "nuvora-device": JSON.stringify(options.device) };

  async function call<T>(
    method: HttpMethod,
    path: Path,
    params: Record<string, string | number>,
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

  return {
    home: (signal) => call<HomeResponse>("GET", "/api/home", {}, signal),
    row: (kind, after, signal) => call<RowPage>("GET", "/api/rows/{row}", { row: kind }, signal, { after }),
    item: (id, signal) => call<ItemDetail>("GET", "/api/items/{id}", { id }, signal),
    scanStatus: (signal) => call<ScanStatus>("GET", "/api/scan", {}, signal),
    startScan: () => call<ScanStatus>("POST", "/api/scan", {}),
    imageUrl: (image, size) => options.baseUrl + fill("/api/images/{id}/{size}", { id: image.id, size }),
    resolve: (url) => (url.startsWith("/") ? options.baseUrl + url : url),
  };
}

function fill(path: Path, params: Record<string, string | number>): string {
  return path.replace(/\{(\w+)\}/g, (_, name: string) => encodeURIComponent(String(params[name])));
}

function queryString(query: Record<string, string> | undefined): string {
  if (!query) return "";
  const pairs = Object.entries(query).map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`);
  return pairs.length ? `?${pairs.join("&")}` : "";
}

/** The error body is `{ code, message }` JSON when the server has a stable code to give; anything else carries none. */
async function apiError(res: ResponseLike, message: string): Promise<ApiError> {
  const body = await res.json().catch(() => null);
  if (typeof body !== "object" || body === null) return new ApiError(res.status, message);
  const field = (name: string) => {
    const value = (body as Record<string, unknown>)[name];
    return typeof value === "string" ? value : null;
  };
  return new ApiError(res.status, message, field("code"), field("message") ?? field("error"));
}
