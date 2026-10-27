import { describe, expect, test } from "bun:test";
import { contentLabel, languageLabels, releaseLanguage } from "../src/releases";

const sound = { audio_languages: [] as string[], subtitle_languages: [] as string[], dual_audio: false, dubbed: false, hardcoded_subs: false };

describe("contentLabel", () => {
  test("says what a release holds", () => {
    expect(contentLabel({ kind: "movie" })).toBe("Movie");
    expect(contentLabel({ kind: "episodes", season: 1, first: 5, last: 5 })).toBe("S1 · E5");
    expect(contentLabel({ kind: "episodes", season: 3, first: 1, last: 10 })).toBe("S3 · E1–10");
    expect(contentLabel({ kind: "episodes", season: null, first: 5, last: 5 })).toBe("Episode 5");
    expect(contentLabel({ kind: "episodes", season: null, first: 1, last: 12 })).toBe("Episodes 1–12");
    expect(contentLabel({ kind: "seasons", first: 2, last: 2 })).toBe("Season 2");
    expect(contentLabel({ kind: "seasons", first: 1, last: 10 })).toBe("Seasons 1–10");
    expect(contentLabel({ kind: "complete" })).toBe("Complete series");
  });
});

describe("releaseLanguage", () => {
  test("English, other languages only, or not said", () => {
    expect(releaseLanguage({ ...sound, audio_languages: ["hi", "en"] })).toBe("english");
    expect(releaseLanguage({ ...sound, audio_languages: ["ru"] })).toBe("other");
    expect(releaseLanguage({ ...sound, audio_languages: ["ja"], dual_audio: true })).toBe("unknown");
    expect(releaseLanguage(sound)).toBe("unknown");
  });
});

describe("languageLabels", () => {
  test("names the sound, then the subtitles", () => {
    expect(languageLabels({ ...sound, audio_languages: ["ru"], dubbed: true })).toEqual(["Russian dub"]);
    expect(languageLabels({ ...sound, audio_languages: ["ko", "en"], dual_audio: true, dubbed: true })).toEqual(["Korean", "English", "Dubbed"]);
    expect(languageLabels({ ...sound, dual_audio: true, subtitle_languages: ["mul"] })).toEqual(["Dual audio", "Many subtitles"]);
    expect(languageLabels({ ...sound, subtitle_languages: ["ko"], hardcoded_subs: true })).toEqual(["Korean burned-in subtitles"]);
    expect(languageLabels({ ...sound, subtitle_languages: ["fr"] })).toEqual(["French subtitles"]);
    expect(languageLabels(sound)).toEqual([]);
  });
});
