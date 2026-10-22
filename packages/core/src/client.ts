import type { components, paths } from "./generated/schema";
import { ApiError, type AbortSignalLike, type FetchLike } from "./transport";

type Schemas = components["schemas"];
export type HomeResponse = Schemas["HomeResponse"];
export type HomeRow = Schemas["HomeRow"];
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
}

/**
 * One method per screen or action. Callers never build paths, set headers, or
 * parse JSON; the generated `paths` type checks every path literal below, so a
 * renamed route in Rust fails this file's typecheck after `bun run gen:api`.
 */
export interface NuvoraClient {
  home(signal?: AbortSignalLike): Promise<HomeResponse>;
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

  async function call<T>(method: "GET" | "POST", path: Path, params: Record<string, string | number>, signal?: AbortSignalLike): Promise<T> {
    const url = options.baseUrl + fill(path, params);
    const init = signal ? { method, headers, signal } : { method, headers };
    const res = await options.fetch(url, init);
    if (!res.ok) throw new ApiError(res.status, `${method} ${url} -> ${res.status}`);
    // Trusted: the server's types generated this client. Parsing again here would
    // be a second schema to keep in sync.
    return (await res.json()) as T;
  }

  return {
    home: (signal) => call<HomeResponse>("GET", "/api/home", {}, signal),
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
