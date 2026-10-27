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
  VideoSupport,
  AudioSupport,
  ErrorBody,
  FsListing,
  LoginRequest,
  Role,
  CastMember,
  HomeBackground,
  HomeLayout,
  HomeRowConfig,
  HomeRowQuery,
  PosterSize,
  Details,
  MetadataSettings,
  ServerInfo,
  Tuning,
  TuningResponse,
  HardwareEncoding,
  X264Preset,
  X264Tune,
  FfmpegLogLevel,
  Activity,
  ActiveStream,
  Download,
  DownloadState,
  AddDownloadRequest,
  ReleaseSearch,
  ReleaseResult,
  ReleaseSource,
  DownloadSettings,
  AddIndexerRequest,
  StreamState,
  PreviewQueue,
  UpdateStatus,
  UpdatePhase,
  Health,
  UpdateMetadataRequest,
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
  Episode,
  EpisodeDetail,
  EpisodeRef,
  FsInspect,
  IndexGroup,
  ItemDetail,
  ItemKind,
  LibraryIndex,
  SearchEpisode,
  PartyClient,
  PartyStatus,
  PartyView,
  PartyMember,
  PartyPerson,
  PartyInvite,
  PartyPlayback,
  LiveEvent,
  LiveCommand,
  SearchPerson,
  SearchResponse,
  LibraryKind,
  LibraryPage,
  LibrarySummary,
  MovieDetail,
  SeasonDetail,
  SeasonSummary,
  ShowDetail,
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
  AppSession,
  PreviewInfo,
  SubtitleFormat,
  SubtitleMode,
  SubtitleTrack,
} from "./client";
export { createQueries, findCard, isScanRunning, LIBRARY_PAGE_SIZE, queryKeys, homeRowQueryParams, rowCards, watchStateKeys } from "./queries";
export type { InfiniteQueryDef, Queries, QueryDef } from "./queries";
export { placeholderDataUrl } from "./images";
export { previewFrame, type PreviewFrame } from "./previews";
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
export {
  canGroup,
  DEFAULT_ORDER,
  DEFAULT_VIEW,
  FILTER_LABELS,
  hasGroups,
  hasViewParams,
  isFiltered,
  LIBRARY_SORTS,
  libraryQuery,
  MIN_GROUPED_CARDS,
  librarySearch,
  libraryViewStorageKey,
  ORDER_LABELS,
  parseLibraryView,
  RESOLUTION_LABELS,
  RESOLUTIONS,
  sameView,
  showsGroups,
  SORT_LABELS,
  SORT_ORDERS,
  WATCH_FILTERS,
  withSort,
} from "./library";
export type { LibrarySearch, LibrarySort, LibraryView, Resolution, SortOrder, WatchFilter } from "./library";
export { anchorsOf, cardsByOffset, groupOffsets, JUMP_LETTERS, layoutGrid, nextAnchor, offsetsInView, offsetTop, stepOffset } from "./grid";
export type { Anchor, GridGroup, GridLayout, GridMetrics, GridPage, GridSection } from "./grid";
export { episodeHeading, hasOwnTitle, initialSeason, plural, showCounts, showPlay, UP_NEXT_SECONDS, upNextCountdown } from "./shows";
export type { ShowPlay } from "./shows";
export { partyCorrection, partyPosition, partyStartsIn, ServerClock, type PartyCorrection } from "./party";
