import type { ErrorBody } from "./client";
import { ApiError } from "./transport";

/** The stable codes the server puts on an error body. */
export type ErrorCode = ErrorBody["code"];

const SERVER_TROUBLE = "Something went wrong on the server. Try again.";

/** Unknown codes read like `bad_request`, as the server asks. */
const SENTENCES: Record<ErrorCode, string> = {
  bad_request: "Nuvora couldn't use that request. Reload the page and try again.",
  validation: "Check the highlighted fields and try again.",
  unauthorized: "Your session ended. Sign in again.",
  bad_credentials: "Wrong username or password.",
  forbidden: "Only an admin can do that.",
  not_found: "That no longer exists. It may have been removed in another window.",
  setup_done: "This server is already set up. Sign in instead.",
  username_taken: "That username is taken. Choose another.",
  last_admin: "Nuvora needs at least one admin who can sign in. Make someone else an admin first.",
  rate_limited: "Too many sign-in attempts. Try again in 15 minutes.",
  busy: "The server is as busy as it's set to be. Try again in a minute.",
  internal: SERVER_TROUBLE,
};

/**
 * What to tell a person when a request failed: one plain sentence. `overrides`
 * replaces a code's sentence where the generic one would mislead (a wrong
 * current password on the account screen is `bad_credentials` too).
 */
export function describeError(error: unknown, overrides: Partial<Record<ErrorCode, string>> = {}): string {
  if (!(error instanceof ApiError)) return "Can't reach your Nuvora server. Check that it's running, then try again.";
  const code = errorCode(error);
  if (!code) return error.status >= 500 ? SERVER_TROUBLE : SENTENCES.bad_request;
  if (overrides[code]) return overrides[code];
  // The server writes validation messages for people ("Usernames are 1 to 32 letters..."); others are generic.
  if (code === "validation" && error.detail) return sentence(error.detail);
  return SENTENCES[code];
}

/** The error's code when it is one this build knows; a status alone maps to the code it implies. */
export function errorCode(error: ApiError): ErrorCode | null {
  if (error.code && error.code in SENTENCES) return error.code as ErrorCode;
  if (error.code) return "bad_request";
  const byStatus: Record<number, ErrorCode> = { 401: "unauthorized", 403: "forbidden", 404: "not_found", 429: "rate_limited" };
  return byStatus[error.status] ?? null;
}

function sentence(text: string): string {
  const t = text.trim();
  const capped = t.charAt(0).toUpperCase() + t.slice(1);
  return /[.!?]$/.test(capped) ? capped : `${capped}.`;
}
