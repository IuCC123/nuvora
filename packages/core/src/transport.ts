/**
 * The only I/O boundary of the data layer. Each app injects its platform's
 * `fetch` through these structural types, so this package never names a DOM
 * or React Native type. The web app passes `globalThis.fetch`; RN passes its own.
 */

export interface AbortSignalLike {
  readonly aborted: boolean;
}

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export interface RequestInitLike {
  method: HttpMethod;
  headers: Record<string, string>;
  /** JSON text. State-changing requests always send `content-type: application/json` (the server's CSRF rule). */
  body?: string;
  signal?: AbortSignalLike;
  /** Let the request outlive the page (the web's last progress report on close). Platforms without it ignore it. */
  keepalive?: boolean;
}

export interface ResponseLike {
  readonly ok: boolean;
  readonly status: number;
  json(): Promise<unknown>;
}

export type FetchLike = (url: string, init: RequestInitLike) => Promise<ResponseLike>;

/**
 * A non-2xx answer. `status === 404` on an item means "deleted", not "broken".
 * `code` is the server's stable error code (`bad_credentials`, `last_admin`,
 * ...) when the body carried one; `describeError` turns it into a sentence.
 */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
    readonly code: string | null = null,
    /** The server's own explanation, when it sent one. */
    readonly detail: string | null = null,
    /** The request field a `validation` error is about (`"username"`, `"paths"`). */
    readonly field: string | null = null,
  ) {
    super(message);
  }
}
