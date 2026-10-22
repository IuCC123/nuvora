import { ApiError } from "./transport";

/** The stable codes the server puts on an error body. */
export type ErrorCode = "setup_done" | "bad_credentials" | "rate_limited" | "last_admin" | "validation";

const SENTENCES: Record<ErrorCode, string> = {
  setup_done: "This server is already set up. Sign in instead.",
  bad_credentials: "Wrong username or password.",
  rate_limited: "Too many sign-in attempts. Try again in 15 minutes.",
  last_admin: "Nuvora needs at least one admin who can sign in. Make someone else an admin first.",
  validation: "Check the highlighted fields and try again.",
};

/**
 * What to tell a person when a request failed: one plain sentence. `overrides`
 * replaces a code's sentence where the generic one would mislead (a wrong
 * current password on the account screen is `bad_credentials` too).
 */
export function describeError(error: unknown, overrides: Partial<Record<ErrorCode, string>> = {}): string {
  if (!(error instanceof ApiError)) return "Can't reach your Nuvora server. Check that it's running, then try again.";
  const code = error.code as ErrorCode | null;
  if (code && code in SENTENCES) {
    if (overrides[code]) return overrides[code];
    if (code === "validation" && error.detail) return sentence(error.detail);
    return SENTENCES[code];
  }
  if (error.status === 401) return "Your session ended. Sign in again.";
  if (error.status === 403) return "Only an admin can do that.";
  if (error.status === 404) return "That no longer exists. It may have been removed in another window.";
  return "Something went wrong on the server. Try again.";
}

function sentence(text: string): string {
  const t = text.trim();
  const capped = t.charAt(0).toUpperCase() + t.slice(1);
  return /[.!?]$/.test(capped) ? capped : `${capped}.`;
}
