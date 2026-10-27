import type { components, paths } from "./generated/schema";
import { ApiError, type AbortSignalLike, type FetchLike, type HttpMethod, type ResponseLike } from "./transport";

type Schemas = components["schemas"];
export type HomeResponse = Schemas["HomeResponse"];
export type HomeLayout = Schemas["HomeLayout"];
export type HomeRowConfig = Schemas["HomeRowConfig"];
export type HomeBackground = Schemas["HomeBackground"];
export type PosterSize = Schemas["PosterSize"];
export type HomeRowQuery = Schemas["HomeRowQuery"];
export type HomeRow = Schemas["HomeRow"];
export type RowKind = Schemas["RowKind"];
export type RowPage = Schemas["RowPage"];
export type PosterCard = Schemas["PosterCard"];
export type ItemDetail = Schemas["ItemDetail"];
export type ItemKind = Schemas["ItemKind"];
export type MovieDetail = Schemas["MovieDetail"];
export type ShowDetail = Schemas["ShowDetail"];
export type SeasonSummary = Schemas["SeasonSummary"];
export type SeasonDetail = Schemas["SeasonDetail"];
export type EpisodeDetail = Schemas["EpisodeDetail"];
export type EpisodeRef = Schemas["EpisodeRef"];
export type Episode = Schemas["Episode"];
export type LibrarySummary = Schemas["LibrarySummary"];
export type LibraryKind = Schemas["LibraryKind"];
export type LibraryPage = Schemas["LibraryPage"];
export type LibraryIndex = Schemas["LibraryIndex"];
export type SearchResponse = Schemas["SearchResponse"];
export type PartyStatus = Schemas["PartyStatus"];
export type PartyView = Schemas["PartyView"];
export type PartyMember = Schemas["PartyMember"];
export type PartyPerson = Schemas["PartyPerson"];
export type PartyInvite = Schemas["PartyInvite"];
export type PartyPlayback = Schemas["PartyPlayback"];
export type LiveEvent = Schemas["LiveEvent"];
export type LiveCommand = Schemas["LiveCommand"];
export type SearchEpisode = Schemas["SearchEpisode"];
export type SearchPerson = Schemas["SearchPerson"];
export type IndexGroup = Schemas["IndexGroup"];
export type FsInspect = Schemas["FsInspect"];
export type Version = Schemas["Version"];
export type Playback = Schemas["Playback"];
export type Image = Schemas["Image"];
export type ImageSize = Schemas["ImageSize"];
export type ScanStatus = Schemas["ScanStatus"];
export type DeviceProfile = Schemas["DeviceProfile"];
export type VideoSupport = Schemas["VideoSupport"];
export type AudioSupport = Schemas["AudioSupport"];
export type AuthState = Schemas["AuthState"];
export type User = Schemas["User"];
export type Role = Schemas["Role"];
export type SetupRequest = Schemas["SetupRequest"];
export type LoginRequest = Schemas["LoginRequest"];
export type AppSession = Schemas["AppSession"];
export type UpdateMeRequest = Schemas["UpdateMeRequest"];
export type AdminLibrary = Schemas["AdminLibrary"];
export type CreateLibraryRequest = Schemas["CreateLibraryRequest"];
export type UpdateLibraryRequest = Schemas["UpdateLibraryRequest"];
export type FsListing = Schemas["FsListing"];
export type AdminUser = Schemas["AdminUser"];
export type CreateUserRequest = Schemas["CreateUserRequest"];
export type UpdateUserRequest = Schemas["UpdateUserRequest"];
export type ServerInfo = Schemas["ServerInfo"];
export type Details = Schemas["Details"];
export type CastMember = Schemas["CastMember"];
export type MetadataSettings = Schemas["MetadataSettings"];
export type UpdateStatus = Schemas["UpdateStatus"];
export type UpdatePhase = Schemas["UpdatePhase"];
export type Health = Schemas["Health"];
export type UpdateMetadataRequest = Schemas["UpdateMetadataRequest"];
export type UpdateServerRequest = Schemas["UpdateServerRequest"];
export type Tuning = Schemas["Tuning"];
export type TuningResponse = Schemas["TuningResponse"];
export type HardwareEncoding = Schemas["HardwareEncoding"];
export type X264Preset = Schemas["X264Preset"];
export type X264Tune = Schemas["X264Tune"];
export type FfmpegLogLevel = Schemas["FfmpegLogLevel"];
export type Activity = Schemas["Activity"];
export type ActiveStream = Schemas["ActiveStream"];
export type StreamState = Schemas["StreamState"];
export type PreviewQueue = Schemas["PreviewQueue"];
export type ErrorBody = Schemas["ErrorBody"];
export type ProgressRequest = Schemas["ProgressRequest"];
export type WatchState = Schemas["WatchState"];
export type AudioTrack = Schemas["AudioTrack"];
export type SubtitleTrack = Schemas["SubtitleTrack"];
export type SubtitleFormat = Schemas["SubtitleFormat"];
export type SubtitleMode = Schemas["SubtitleMode"];
export type FontAttachment = Schemas["FontAttachment"];
export type PreviewInfo = Schemas["PreviewInfo"];
export type Download = Schemas["Download"];
export type DownloadState = Schemas["DownloadState"];
export type AddDownloadRequest = Schemas["AddDownloadRequest"];
export type ReleaseSearch = Schemas["ReleaseSearch"];
export type ReleaseResult = Schemas["ReleaseResult"];
export type ReleaseContent = Schemas["ReleaseContent"];
export type UpdateDownloadSettings = Schemas["UpdateDownloadSettings"];
export type ReleaseSource = Schemas["ReleaseSource"];
export type DownloadSettings = Schemas["DownloadSettings"];
export type AddIndexerRequest = Schemas["AddIndexerRequest"];
export type TorrentStream = Schemas["TorrentStream"];
export type TorrentStreamState = Schemas["TorrentStreamState"];
export type OpenStreamRequest = Schemas["OpenStreamRequest"];
export type WatchedStream = Schemas["WatchedStream"];
export type StreamProgressRequest = Schemas["StreamProgressRequest"];

export interface ClientOptions {
  /** Origin of the Nuvora server, e.g. `""` (same origin, web) or `"http://192.168.1.10:8640"` (TV). */
  baseUrl: string;
  fetch: FetchLike;
  /** Measured once at startup by the app (it needs platform APIs this package must not touch). */
  device: DeviceProfile;
  /**
   * An app's session token (`appSignIn`), sent as `Authorization: Bearer`.
   * The web leaves it out: its session is a cookie.
   */
  token?: string;
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
  homeLayout(signal?: AbortSignalLike): Promise<HomeLayout>;
  saveHomeLayout(layout: HomeLayout): Promise<HomeLayout>;
  resetHomeLayout(): Promise<void>;
  home(signal?: AbortSignalLike): Promise<HomeResponse>;
  /** The page of a home row after `after` (a `next` from `home()` or a previous page). A `library` row also names its library. */
  row(kind: RowKind, after: string, signal?: AbortSignalLike, library?: number): Promise<RowPage>;
  /** Rejects with `ApiError(404)` when the item does not exist. */
  item(id: number, signal?: AbortSignalLike): Promise<ItemDetail>;
  /** A season's episodes in order. 404 unless `seasonId` is a season. */
  episodes(seasonId: number, signal?: AbortSignalLike): Promise<Episode[]>;
  /** Every library, for the nav rail and the library page header. */
  libraries(signal?: AbortSignalLike): Promise<LibrarySummary[]>;
  /** One page of a library grid. `query` comes from `libraryQuery`; `after` from a page's `next` or a group's `after`. */
  libraryItems(id: number, query: Record<string, string>, after: string | null, limit: number, signal?: AbortSignalLike): Promise<LibraryPage>;
  /** The grid's group headers for the same query: letters, decades or months. */
  libraryIndex(id: number, query: Record<string, string>, signal?: AbortSignalLike): Promise<LibraryIndex>;
  /** Titles, episodes and people matching `q`, across every library. */
  search(q: string, signal?: AbortSignalLike): Promise<SearchResponse>;
  scanStatus(signal?: AbortSignalLike): Promise<ScanStatus>;
  /** Idempotent; returns the status after queuing. */
  startScan(): Promise<ScanStatus>;
  /** Never 401: without a session `user` is null. `setup_required` until the first admin exists. */
  authState(signal?: AbortSignalLike): Promise<AuthState>;
  /** The server's version, without signing in. */
  health(signal?: AbortSignalLike): Promise<Health>;
  /** Creates the first admin and signs them in. 409 `setup_done` once any user exists. */
  setup(body: SetupRequest): Promise<User>;
  /** 401 `bad_credentials` for any wrong detail; 429 `rate_limited` after five failures. */
  login(body: LoginRequest): Promise<User>;
  /** Sign-in for apps: the session comes back as a token to pass as `token`, not a cookie. Same errors as `login`. */
  appSignIn(body: LoginRequest): Promise<AppSession>;
  logout(): Promise<void>;
  /** The signed-in user's own name and password. A new password needs the current one. */
  updateMe(body: UpdateMeRequest): Promise<User>;
  /**
   * Where playback is; the server decides what it means (see `createProgressReporter`).
   * `keepalive` lets the request finish after the page closes.
   */
  reportProgress(id: number, body: ProgressRequest, options?: { keepalive?: boolean }): Promise<void>;
  markWatched(id: number): Promise<void>;
  markUnwatched(id: number): Promise<void>;
  /** A file's embedded fonts, which its styled subtitle tracks may name. Empty for most files. */
  fonts(fileId: number, signal?: AbortSignalLike): Promise<FontAttachment[]>;
  /** A file's seek previews; `null` while the server is still making them (asking hurries them along). */
  previews(fileId: number, signal?: AbortSignalLike): Promise<PreviewInfo | null>;
  /** Admin role only; anyone else gets 403. */
  admin: AdminClient;
  /** Watch Together. Live updates arrive on the `/api/live` WebSocket. */
  party: PartyClient;
  /** Absolute URL of an artwork size. Immutable: safe to cache forever. */
  imageUrl(image: Image, size: ImageSize): string;
  /**
   * Absolute URL of a renderable subtitle track (`SubtitleTrack.index`) as WebVTT or,
   * for styled tracks, ASS. Immutable: safe to cache forever.
   */
  subtitleUrl(fileId: number, streamIndex: number, format: "vtt" | "ass"): string;
  /** Absolute URL for a server-relative URL the API returned (e.g. `playback.url`). */
  resolve(url: string): string;
}

/** Watch Together: invitations and membership. Playback goes over `/api/live`. */
export interface PartyClient {
  /** Everyone else who can sign in: who you can invite. */
  people(signal?: AbortSignalLike): Promise<PartyPerson[]>;
  status(signal?: AbortSignalLike): Promise<PartyStatus>;
  /** Starts a party for you first if you aren't in one. */
  invite(userId: number): Promise<PartyView>;
  cancelInvite(userId: number): Promise<PartyView>;
  /** Accepts an invitation, leaving any party you're in. */
  join(partyId: string): Promise<PartyView>;
  decline(partyId: string): Promise<void>;
  leave(): Promise<void>;
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
  /** Counts a folder's video files and how many look like episodes, to suggest a library type. */
  inspect(path: string, signal?: AbortSignalLike): Promise<FsInspect>;
  users(signal?: AbortSignalLike): Promise<AdminUser[]>;
  createUser(body: CreateUserRequest): Promise<AdminUser>;
  /** 409 `last_admin` when the change would leave no enabled admin. */
  updateUser(id: number, body: UpdateUserRequest): Promise<AdminUser>;
  deleteUser(id: number): Promise<void>;
  server(signal?: AbortSignalLike): Promise<ServerInfo>;
  updateServer(body: UpdateServerRequest): Promise<ServerInfo>;
  metadata(signal?: AbortSignalLike): Promise<MetadataSettings>;
  updateMetadata(body: UpdateMetadataRequest): Promise<MetadataSettings>;
  refreshMetadata(): Promise<void>;
  /** Transcoding settings, their defaults, and the encoder in use. */
  tuning(signal?: AbortSignalLike): Promise<TuningResponse>;
  /** Saves every setting; can take a few seconds while a new encoder is tested. 400 `validation` names the field. */
  updateTuning(body: Tuning): Promise<TuningResponse>;
  /** What the server is converting, scanning and making previews for, right now. */
  activity(signal?: AbortSignalLike): Promise<Activity>;
  /** This version and the newest; `refresh` asks GitHub now (at most once a minute). */
  update(refresh?: boolean, signal?: AbortSignalLike): Promise<UpdateStatus>;
  /** Downloads and installs the newest version; the server then restarts. */
  installUpdate(): Promise<UpdateStatus>;
  /** Every download, newest first. Poll it while the page is open. */
  downloads(signal?: AbortSignalLike): Promise<Download[]>;
  /** 400 when the link isn't a magnet or `.torrent` link, there is no Movies library, or the server can't download. */
  addDownload(body: AddDownloadRequest): Promise<Download>;
  pauseDownload(id: number): Promise<void>;
  /** Also starts a failed download again. */
  resumeDownload(id: number): Promise<void>;
  /** Cancels and deletes what was fetched; a finished one only leaves the list. */
  removeDownload(id: number): Promise<void>;
  /** Movie releases from every indexer, most seeders first. */
  searchReleases(q: string, signal?: AbortSignalLike): Promise<ReleaseSearch>;
  downloadSettings(signal?: AbortSignalLike): Promise<DownloadSettings>;
  /** Where movies, and episodes and seasons, go; `null` for the first library of the kind. Send both. */
  setDownloadLibraries(body: UpdateDownloadSettings): Promise<DownloadSettings>;
  /** The server asks the indexer first; 400 `validation` names `url` or `api_key` when it doesn't answer. */
  addIndexer(body: AddIndexerRequest): Promise<DownloadSettings>;
  removeIndexer(id: number): Promise<DownloadSettings>;
  /** Starts playing a release while it downloads, or returns the stream already playing it. */
  openStream(body: OpenStreamRequest): Promise<TorrentStream>;
  /** Progress, and once `ready` how this device plays it. Poll it: a stream nobody asks about for two minutes stops. */
  stream(id: string, signal?: AbortSignalLike): Promise<TorrentStream>;
  /** Stops it and deletes what it fetched. */
  stopStream(id: string): Promise<void>;
  /** Where the player is in a stream, kept per person for Continue Watching (not under a minute, not past 90%). */
  streamProgress(id: string, body: StreamProgressRequest, opts?: { keepalive?: boolean }): Promise<void>;
  /** Streams the signed-in person stopped partway through, newest first. */
  watchedStreams(signal?: AbortSignalLike): Promise<WatchedStream[]>;
  /** Takes a stream off Continue Watching. */
  forgetStream(link: string): Promise<void>;
}

type Params = Record<string, string | number>;

/** The routes that answer `method`, so a call to a route without it fails the typecheck. */
type Path<M extends HttpMethod = HttpMethod> = {
  [P in keyof paths]: paths[P] extends Record<Lowercase<M>, object> ? P : never;
}[keyof paths];

export function createClient(options: ClientOptions): NuvoraClient {
  const headers: Record<string, string> = {
    accept: "application/json",
    "nuvora-device": JSON.stringify(options.device),
    ...(options.token && { authorization: `Bearer ${options.token}` }),
  };

  async function call<T>(
    method: HttpMethod,
    path: keyof paths,
    params: Params,
    signal?: AbortSignalLike,
    query?: Record<string, string>,
    body?: unknown,
    keepalive?: boolean,
  ): Promise<T> {
    const url = options.baseUrl + fill(path, params) + queryString(query);
    const init = {
      method,
      headers: method === "GET" ? headers : { ...headers, "content-type": "application/json" },
      ...(body !== undefined && { body: JSON.stringify(body) }),
      ...(signal && { signal }),
      ...(keepalive && { keepalive }),
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
  const put = (path: Path<"PUT">, params: Params, body?: unknown, keepalive?: boolean) =>
    call<void>("PUT", path, params, undefined, undefined, body, keepalive);
  const remove = (path: Path<"DELETE">, params: Params) => call<void>("DELETE", path, params);

  return {
    home: (signal) => get<HomeResponse>("/api/home", {}, signal),
    homeLayout: (signal) => get<HomeLayout>("/api/me/home", {}, signal),
    saveHomeLayout: async (layout) => {
      await put("/api/me/home", {}, layout);
      return get<HomeLayout>("/api/me/home");
    },
    resetHomeLayout: () => remove("/api/me/home", {}),
    row: (kind, after, signal, library) =>
      get<RowPage>("/api/rows/{row}", { row: kind }, signal, library === undefined ? { after } : { after, library: String(library) }),
    item: (id, signal) => get<ItemDetail>("/api/items/{id}", { id }, signal),
    episodes: (id, signal) => get<Episode[]>("/api/items/{id}/episodes", { id }, signal),
    libraries: (signal) => get<LibrarySummary[]>("/api/libraries", {}, signal),
    libraryItems: (id, query, after, limit, signal) =>
      get<LibraryPage>("/api/libraries/{id}/items", { id }, signal, { ...query, ...(after !== null && { after }), limit: String(limit) }),
    libraryIndex: (id, query, signal) => get<LibraryIndex>("/api/libraries/{id}/index", { id }, signal, query),
    search: (q, signal) => get<SearchResponse>("/api/search", {}, signal, { q }),
    scanStatus: (signal) => get<ScanStatus>("/api/scan", {}, signal),
    startScan: () => post<ScanStatus>("/api/scan"),
    authState: (signal) => get<AuthState>("/api/auth/state", {}, signal),
    health: (signal) => get<Health>("/api/health", {}, signal),
    setup: (body) => post<User>("/api/setup", {}, body),
    login: (body) => post<User>("/api/auth/login", {}, body),
    appSignIn: (body) => post<AppSession>("/api/auth/token", {}, body),
    logout: () => post<void>("/api/auth/logout"),
    updateMe: (body) => patch<User>("/api/auth/me", {}, body),
    reportProgress: (id, body, opts) => put("/api/items/{id}/progress", { id }, body, opts?.keepalive),
    markWatched: (id) => put("/api/items/{id}/watched", { id }),
    markUnwatched: (id) => remove("/api/items/{id}/watched", { id }),
    fonts: (fileId, signal) => get<FontAttachment[]>("/api/files/{id}/fonts", { id: fileId }, signal),
    previews: (fileId, signal) =>
      get<PreviewInfo>("/api/files/{id}/previews", { id: fileId }, signal).catch((e: unknown) => {
        if (e instanceof ApiError && e.status === 404) return null;
        throw e;
      }),
    party: {
      people: (signal) => get<PartyPerson[]>("/api/people", {}, signal),
      status: (signal) => get<PartyStatus>("/api/party", {}, signal),
      invite: (userId) => post<PartyView>("/api/party/invites", {}, { user_id: userId }),
      cancelInvite: (userId) => call<PartyView>("DELETE", "/api/party/invites/{user_id}", { user_id: userId }),
      join: (id) => post<PartyView>("/api/party/{id}/join", { id }),
      decline: (id) => post<void>("/api/party/{id}/decline", { id }),
      leave: () => post<void>("/api/party/leave"),
    },
    admin: {
      libraries: (signal) => get<AdminLibrary[]>("/api/admin/libraries", {}, signal),
      createLibrary: (body) => post<AdminLibrary>("/api/admin/libraries", {}, body),
      updateLibrary: (id, body) => patch<AdminLibrary>("/api/admin/libraries/{id}", { id }, body),
      deleteLibrary: (id) => remove("/api/admin/libraries/{id}", { id }),
      scanLibrary: (id) => post<ScanStatus>("/api/admin/libraries/{id}/scan", { id }),
      fs: (path, signal) => get<FsListing>("/api/admin/fs", {}, signal, path === null ? undefined : { path }),
      inspect: (path, signal) => get<FsInspect>("/api/admin/fs/inspect", {}, signal, { path }),
      users: (signal) => get<AdminUser[]>("/api/admin/users", {}, signal),
      createUser: (body) => post<AdminUser>("/api/admin/users", {}, body),
      updateUser: (id, body) => patch<AdminUser>("/api/admin/users/{id}", { id }, body),
      deleteUser: (id) => remove("/api/admin/users/{id}", { id }),
      server: (signal) => get<ServerInfo>("/api/admin/server", {}, signal),
      updateServer: (body) => patch<ServerInfo>("/api/admin/server", {}, body),
      metadata: (signal) => get<MetadataSettings>("/api/admin/metadata", {}, signal),
      updateMetadata: async (body) => {
        await put("/api/admin/metadata", {}, body);
        return get<MetadataSettings>("/api/admin/metadata");
      },
      refreshMetadata: () => post<void>("/api/admin/metadata/refresh"),
      tuning: (signal) => get<TuningResponse>("/api/admin/tuning", {}, signal),
      updateTuning: (body) => call<TuningResponse>("PUT", "/api/admin/tuning", {}, undefined, undefined, body),
      activity: (signal) => get<Activity>("/api/admin/activity", {}, signal),
      update: (refresh, signal) => get<UpdateStatus>("/api/admin/update", {}, signal, refresh ? { refresh: "true" } : undefined),
      installUpdate: () => post<UpdateStatus>("/api/admin/update"),
      downloads: (signal) => get<Download[]>("/api/admin/downloads", {}, signal),
      addDownload: (body) => post<Download>("/api/admin/downloads", {}, body),
      pauseDownload: (id) => post<void>("/api/admin/downloads/{id}/pause", { id }),
      resumeDownload: (id) => post<void>("/api/admin/downloads/{id}/resume", { id }),
      removeDownload: (id) => remove("/api/admin/downloads/{id}", { id }),
      searchReleases: (q, signal) => get<ReleaseSearch>("/api/admin/downloads/search", {}, signal, { q }),
      downloadSettings: (signal) => get<DownloadSettings>("/api/admin/downloads/settings", {}, signal),
      setDownloadLibraries: (body) => call<DownloadSettings>("PUT", "/api/admin/downloads/settings", {}, undefined, undefined, body),
      addIndexer: (body) => post<DownloadSettings>("/api/admin/downloads/indexers", {}, body),
      removeIndexer: (id) => call<DownloadSettings>("DELETE", "/api/admin/downloads/indexers/{id}", { id }),
      openStream: (body) => post<TorrentStream>("/api/admin/streams", {}, body),
      stream: (id, signal) => get<TorrentStream>("/api/admin/streams/{id}", { id }, signal),
      stopStream: (id) => remove("/api/admin/streams/{id}", { id }),
      streamProgress: (id, body, opts) => put("/api/admin/streams/{id}/progress", { id }, body, opts?.keepalive),
      watchedStreams: (signal) => get<WatchedStream[]>("/api/admin/streams/watched", {}, signal),
      forgetStream: (link) => call<void>("DELETE", "/api/admin/streams/watched", {}, undefined, { link }),
    },
    imageUrl: (image, size) => options.baseUrl + fill("/api/images/{id}/{size}", { id: image.id, size }),
    subtitleUrl: (fileId, streamIndex, format) =>
      options.baseUrl + fill("/api/files/{id}/subtitles/{stream_index}", { id: fileId, stream_index: streamIndex }) + queryString({ format }),
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
