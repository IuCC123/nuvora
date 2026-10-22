/**
 * @nuvora/core: the Nuvora data layer. Typed API client generated from the
 * server's OpenAPI document, cache policy, and pure helpers. Imported unchanged
 * by the web app and the React Native TV apps; `package.json#exports` exposes
 * only this file, so internals cannot be deep-imported.
 */
export { createClient } from "./client";
export type {
  ClientOptions,
  DeviceProfile,
  HomeResponse,
  HomeRow,
  Image,
  ImageSize,
  ItemDetail,
  MovieDetail,
  NuvoraClient,
  Playback,
  PosterCard,
  RowKind,
  RowPage,
  ScanStatus,
  Version,
} from "./client";
export { createQueries, findCard, isScanRunning, queryKeys } from "./queries";
export type { Queries, QueryDef } from "./queries";
export { placeholderDataUrl } from "./images";
export { ApiError } from "./transport";
export type { AbortSignalLike, FetchLike, RequestInitLike, ResponseLike } from "./transport";
