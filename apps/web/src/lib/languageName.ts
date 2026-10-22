const names = new Intl.DisplayNames(undefined, { type: "language", fallback: "code" });

/** `"eng"` or `"en"` -> `"English"` in the browser's language; unknown codes come back unchanged. */
export function languageName(code: string): string {
  try {
    return names.of(code) ?? code;
  } catch {
    return code; // not a well-formed BCP 47 tag
  }
}
