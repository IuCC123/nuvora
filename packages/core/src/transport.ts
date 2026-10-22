/**
 * The only I/O boundary of the data layer. Each app injects its platform's
 * `fetch` through these structural types, so this package never names a DOM
 * or React Native type. The web app passes `globalThis.fetch`; RN passes its own.
 */

export interface AbortSignalLike {
  readonly aborted: boolean;
}

export interface RequestInitLike {
  method: "GET" | "POST";
  headers: Record<string, string>;
  signal?: AbortSignalLike;
}

export interface ResponseLike {
  readonly ok: boolean;
  readonly status: number;
  json(): Promise<unknown>;
}

export type FetchLike = (url: string, init: RequestInitLike) => Promise<ResponseLike>;

/** A non-2xx answer. `status === 404` on an item means "deleted", not "broken". */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}
