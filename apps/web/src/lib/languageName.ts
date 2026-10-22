import { findLanguage, languageName as englishName } from "@nuvora/core";

const names = new Intl.DisplayNames(undefined, { type: "language", fallback: "none" });

/**
 * `"eng"`, `"fre"` or `"en"` -> `"English"`, `"French"` in the browser's language,
 * via the ISO 639-1 code `Intl` knows best. Falls back to core's English names,
 * then to the code itself.
 */
export function languageName(code: string | null | undefined): string {
  const short = findLanguage(code)?.short ?? code;
  try {
    const name = short ? names.of(short) : undefined;
    if (name) return name.charAt(0).toLocaleUpperCase() + name.slice(1);
  } catch {
    // not a well-formed BCP 47 tag
  }
  return englishName(code);
}
