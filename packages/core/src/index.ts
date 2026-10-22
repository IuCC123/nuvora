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
  ProgressRequest,
  RowKind,
  RowPage,
  ScanStatus,
  Version,
  WatchState,
  AudioTrack,
  FontAttachment,
  SubtitleFormat,
  SubtitleMode,
  SubtitleTrack,
} from "./client";
export { createQueries, findCard, isScanRunning, queryKeys, rowCards, watchStateKeys } from "./queries";
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
  formatClock,
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
export { checkDisplayName, checkPassword, checkPasswordsMatch, checkRequired, checkUsername, DISPLAY_NAME_MAX_LENGTH, PASSWORD_MIN_LENGTH } from "./validation";
export { safeRedirect } from "./redirect";
export { folderCrumbs } from "./folders";
export type { FolderCrumb } from "./folders";
export { createProgressReporter, PROGRESS_INTERVAL_MS, PROGRESS_SETTLE_MS } from "./progress";
export type { PlaybackPosition, ProgressReporter, ProgressReporterOptions, Timers } from "./progress";
export { audioLanguage, chooseSubtitle, nextSubtitle, preferredLanguage, rememberSubtitle, subtitleDescription, subtitleLabel } from "./subtitles";
export type { RememberedSubtitle, SubtitleLabel, SubtitlePrefs, SubtitleTag } from "./subtitles";
export { findLanguage, LANGUAGES, languageCode, languageName } from "./languages";
export type { Language } from "./languages";
export { activeCues, parseVtt } from "./vtt";
export type { Cue, CueRun } from "./vtt";
