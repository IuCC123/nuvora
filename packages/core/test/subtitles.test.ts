import { describe, expect, test } from "bun:test";
import {
  audioLanguage,
  chooseSubtitle,
  nextSubtitle,
  preferredLanguage,
  rememberSubtitle,
  subtitleDescription,
  subtitleLabel,
  type SubtitlePrefs,
  type SubtitleTrack,
} from "../src";

let nextIndex = 2;
function track(language: string | null, extra: Partial<SubtitleTrack> = {}): SubtitleTrack {
  return {
    index: nextIndex++,
    codec: "SRT",
    format: "srt",
    renderable: true,
    language,
    title: null,
    is_default: false,
    is_forced: false,
    is_hearing_impaired: false,
    external: false,
    ...extra,
  };
}

const auto: SubtitlePrefs = { mode: "auto", language: "eng" };
const always: SubtitlePrefs = { mode: "always", language: "eng" };
const off: SubtitlePrefs = { mode: "off", language: "eng" };

describe("chooseSubtitle", () => {
  test("the owner's anime: Japanese audio, a default Spanish ASS track, English preferred, auto picks the Spanish track", () => {
    const spanish = track("spa", { codec: "ASS", format: "ass", is_default: true });
    expect(chooseSubtitle([spanish], "jpn", auto, null)).toBe(spanish);
  });

  test("off gives nothing, whatever the file flags", () => {
    expect(chooseSubtitle([track("eng", { is_default: true, is_forced: true })], "jpn", off, null)).toBeNull();
  });

  test("auto: foreign audio picks the preferred-language track, non-forced over forced", () => {
    const forced = track("eng", { is_forced: true, title: "Signs" });
    const full = track("eng");
    const spanish = track("spa", { is_default: true });
    expect(chooseSubtitle([forced, spanish, full], "jpn", auto, null)).toBe(full);
  });

  test("auto: foreign audio with only a forced preferred track uses it", () => {
    const forced = track("eng", { is_forced: true });
    expect(chooseSubtitle([track("spa"), forced], "fre", auto, null)).toBe(forced);
  });

  test("auto: audio in the preferred language shows only a forced preferred track", () => {
    const full = track("eng");
    const forced = track("eng", { is_forced: true });
    expect(chooseSubtitle([full, forced], "eng", auto, null)).toBe(forced);
    expect(chooseSubtitle([full], "eng", auto, null)).toBeNull();
  });

  test("auto: with no preferred track, the default-flagged one, then none", () => {
    const flagged = track("ger", { is_default: true });
    expect(chooseSubtitle([track("fre"), flagged], "jpn", auto, null)).toBe(flagged);
    expect(chooseSubtitle([track("fre"), track("ger")], "jpn", auto, null)).toBeNull();
  });

  test("auto: audio matching the preferred language still honours a default flag, as the spec's rules read", () => {
    const flagged = track("spa", { is_default: true });
    expect(chooseSubtitle([track("eng"), flagged], "eng", auto, null)).toBe(flagged);
  });

  test("auto: untagged audio counts as the viewer's language", () => {
    expect(chooseSubtitle([track("eng")], null, auto, null)).toBeNull();
    expect(chooseSubtitle([track("eng")], "und", auto, null)).toBeNull();
  });

  test("codes compare across ISO 639-1, 639-2/B and 639-2/T", () => {
    const french = track("fra");
    expect(chooseSubtitle([track("ger"), french], "jpn", { mode: "auto", language: "fr" }, null)).toBe(french);
    expect(chooseSubtitle([french], "jpn", { mode: "auto", language: "fre" }, null)).toBe(french);
    const german = track("ger");
    expect(chooseSubtitle([german], "deu", { mode: "auto", language: "ger" }, null)).toBeNull();
  });

  test("always: the preferred-language track even when the audio matches", () => {
    const english = track("eng");
    expect(chooseSubtitle([track("spa", { is_default: true }), english], "eng", always, null)).toBe(english);
  });

  test("always: falls back to the default-flagged track, then none", () => {
    const flagged = track("spa", { is_default: true });
    expect(chooseSubtitle([track("fre"), flagged], "eng", always, null)).toBe(flagged);
    expect(chooseSubtitle([track("fre")], "eng", always, null)).toBeNull();
  });

  test("among several preferred tracks: the default flag, then not SDH, then file order", () => {
    const sdh = track("eng", { is_hearing_impaired: true, title: "SDH" });
    const plain = track("eng", { title: "Dialogue" });
    const flagged = track("eng", { is_default: true, title: "Full" });
    expect(chooseSubtitle([sdh, plain, flagged], "jpn", auto, null)).toBe(flagged);
    expect(chooseSubtitle([sdh, plain], "jpn", auto, null)).toBe(plain);
    const second = track("eng");
    expect(chooseSubtitle([plain, second], "jpn", auto, null)).toBe(plain);
  });

  test("image tracks are never chosen", () => {
    const pgs = track("eng", { codec: "PGS", format: "image", renderable: false, is_default: true });
    expect(chooseSubtitle([pgs], "jpn", auto, null)).toBeNull();
    expect(chooseSubtitle([pgs], "jpn", always, null)).toBeNull();
    const srt = track("eng");
    expect(chooseSubtitle([pgs, srt], "jpn", always, null)).toBe(srt);
  });

  test("no preferred language: auto falls through to the default flag", () => {
    const flagged = track("spa", { is_default: true });
    expect(chooseSubtitle([track("eng"), flagged], "jpn", { mode: "auto", language: null }, null)).toBe(flagged);
  });

  test("a remembered Off wins over the account's Always", () => {
    expect(chooseSubtitle([track("eng")], "jpn", always, { kind: "off" })).toBeNull();
  });

  test("a remembered track wins over the account's Off and the default flag", () => {
    const flagged = track("spa", { is_default: true });
    const english = track("eng");
    const remembered = rememberSubtitle(english);
    expect(chooseSubtitle([flagged, english], "jpn", off, remembered)).toBe(english);
  });

  test("a remembered track carries to the next episode by language, title and flags, not stream index", () => {
    const episode1 = [track("eng", { title: "Signs & Songs", is_forced: true }), track("eng", { title: "Full Subtitles" })];
    const remembered = rememberSubtitle(episode1[1]!);
    const full = track("eng", { title: "Full Subtitles" });
    const episode2 = [track("eng", { title: "Signs & Songs", is_forced: true }), track("spa", { is_default: true }), full];
    expect(chooseSubtitle(episode2, "jpn", auto, remembered)).toBe(full);
  });

  test("a remembered language the file lacks falls back to the account rules", () => {
    const remembered = rememberSubtitle(track("ita"));
    const flagged = track("spa", { is_default: true });
    expect(chooseSubtitle([flagged], "jpn", auto, remembered)).toBe(flagged);
  });
});

test("rememberSubtitle records what the track is", () => {
  expect(rememberSubtitle(null)).toEqual({ kind: "off" });
  expect(rememberSubtitle(track("fra", { format: "ass", title: "Complets", is_forced: false, is_hearing_impaired: true }))).toEqual({
    kind: "track",
    language: "fre",
    title: "Complets",
    forced: false,
    hearingImpaired: true,
    styled: true,
  });
});

test("preferredLanguage: the account's choice, else the device's, as ISO 639-2/B", () => {
  expect(preferredLanguage("spa", "en-US")).toBe("spa");
  expect(preferredLanguage(null, "en-US")).toBe("eng");
  expect(preferredLanguage(undefined, "pt-BR")).toBe("por");
  expect(preferredLanguage(null, "zh-Hant-TW")).toBe("chi");
  expect(preferredLanguage(null, "de")).toBe("ger");
  expect(preferredLanguage(null, "x-klingon")).toBeNull();
});

test("audioLanguage: the default audio track's language, else the first's", () => {
  const a = (language: string | null, is_default: boolean) => ({ index: 1, codec: "aac", channels: 2, language, is_default });
  expect(audioLanguage([a("eng", false), a("jpn", true)])).toBe("jpn");
  expect(audioLanguage([a("jpn", false), a("eng", false)])).toBe("jpn");
  expect(audioLanguage([])).toBeNull();
});

test("nextSubtitle cycles Off, then each renderable track, then Off", () => {
  const eng = track("eng");
  const pgs = track("fre", { format: "image", renderable: false });
  const spa = track("spa");
  const tracks = [eng, pgs, spa];
  expect(nextSubtitle(tracks, null)).toBe(eng);
  expect(nextSubtitle(tracks, eng)).toBe(spa);
  expect(nextSubtitle(tracks, spa)).toBeNull();
  expect(nextSubtitle([], null)).toBeNull();
});

describe("subtitleLabel", () => {
  test("names the language and adds the markers that apply", () => {
    const label = subtitleLabel(track("eng", { title: "Signs & Songs", is_forced: true, format: "ass" }));
    expect(label).toEqual({ name: "English", title: "Signs & Songs", tags: ["Forced", "Styled"], needsConversion: false });
    expect(subtitleDescription(label)).toBe("English, Signs & Songs, Forced, Styled");
  });

  test("drops a title that only repeats the language or a marker", () => {
    expect(subtitleLabel(track("spa", { title: "Spanish" })).title).toBeNull();
    expect(subtitleLabel(track("eng", { title: "SDH", is_hearing_impaired: true })).title).toBeNull();
    expect(subtitleLabel(track("eng", { title: "  " })).title).toBeNull();
  });

  test("image tracks need conversion; untagged tracks read Unknown", () => {
    const label = subtitleLabel(track(null, { format: "image", renderable: false, codec: "PGS" }));
    expect(label).toEqual({ name: "Unknown", title: null, tags: [], needsConversion: true });
    expect(subtitleDescription(label)).toBe("Unknown, needs conversion");
  });

  test("takes a naming function, so the web can use Intl", () => {
    expect(subtitleLabel(track("spa"), () => "Español").name).toBe("Español");
  });
});
