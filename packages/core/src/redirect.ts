/**
 * Where to go after signing in: the in-app path the person was on, never an
 * absolute URL (an open redirect) and never the sign-in or welcome screens.
 */
export function safeRedirect(target: unknown): string {
  if (typeof target !== "string" || !target.startsWith("/") || target.startsWith("//") || target.startsWith("/\\")) return "/";
  if (/^\/(signin|welcome)(?=$|[/?#])/.test(target)) return "/";
  return target;
}
