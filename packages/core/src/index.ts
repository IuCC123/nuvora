/**
 * @nuvora/core: the Nuvora data layer. Typed API client generated from the
 * server's OpenAPI document, cache policy, and pure helpers. Imported unchanged
 * by the web app and the React Native TV apps; `package.json#exports` exposes
 * only this file, so internals cannot be deep-imported.
 */
export { createClient } from "./client";
export type {
  AdminClient,
  AdminLibrary,
  AdminUser,
  AuthState,
  ClientOptions,
  CreateLibraryRequest,
  CreateUserRequest,
  DeviceProfile,
  ErrorBody,
  FsListing,
  LoginRequest,
  Role,
  ServerInfo,
  SetupRequest,
  UpdateLibraryRequest,
  UpdateMeRequest,
  UpdateServerRequest,
  UpdateUserRequest,
  User,
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
export { createQueries, findCard, isScanRunning, queryKeys, rowCards } from "./queries";
export type { InfiniteQueryDef, Queries, QueryDef } from "./queries";
export { placeholderDataUrl } from "./images";
export { titleCardLook } from "./titleCard";
export type { TitleCardLook } from "./titleCard";
export {
  containerLabel,
  formatLastSeen,
  formatUptime,
  initials,
  fileFacts,
  formatBitrate,
  formatBytes,
  formatDuration,
  formatFrameRate,
  formatRuntime,
  versionChips,
} from "./format";
export type { FileFact, LanguageName } from "./format";
export { ApiError } from "./transport";
export type { AbortSignalLike, FetchLike, HttpMethod, RequestInitLike, ResponseLike } from "./transport";
export { describeError, errorCode } from "./errors";
export type { ErrorCode } from "./errors";
export { checkPassword, checkPasswordsMatch, checkRequired, checkUsername, PASSWORD_MIN_LENGTH } from "./validation";
export { safeRedirect } from "./redirect";
export { folderCrumbs } from "./folders";
export type { FolderCrumb } from "./folders";
