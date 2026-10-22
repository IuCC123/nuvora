import { expect, test } from "bun:test";
import { fileFacts, formatBitrate, formatBytes, formatDuration, formatRuntime, versionChips, type Version } from "../src";

test("formats runtimes and durations", () => {
  expect(formatRuntime(734_000)).toBe("12 min");
  expect(formatRuntime(6_720_000)).toBe("1 h 52 min");
  expect(formatRuntime(7_200_000)).toBe("2 h");
  expect(formatRuntime(20_000)).toBe("20 s");
  expect(formatRuntime(59_400)).toBe("59 s");
  expect(formatRuntime(59_600)).toBe("1 min");
  expect(formatRuntime(89_000)).toBe("1 min");
  expect(formatRuntime(91_000)).toBe("2 min");
  expect(formatDuration(734_000)).toBe("12 min 14 s");
  expect(formatDuration(6_723_000)).toBe("1 h 52 min 3 s");
  expect(formatDuration(42_000)).toBe("42 s");
});

test("formats sizes and bitrates in decimal units", () => {
  expect(formatBytes(726_000_000)).toBe("726 MB");
  expect(formatBytes(4_230_000_000)).toBe("4.2 GB");
  expect(formatBytes(512)).toBe("512 B");
  expect(formatBitrate(7_900_000)).toBe("7.9 Mb/s");
  expect(formatBitrate(640_000)).toBe("640 kb/s");
});

const tearsOfSteel: Version = {
  file_id: 13,
  file_name: "Tears of Steel (2012).webm",
  size_bytes: 726_000_000,
  summary: "1080p · VP9 · Opus 2.0",
  playback: { kind: "direct_play", url: "/api/files/13/stream", mime_type: "video/webm" },
  media: {
    container: "webm",
    duration_ms: 734_000,
    bitrate: 7_900_000,
    video: [{ codec: "VP9", profile: "Profile 0", width: 1920, height: 800, frame_rate: 24, hdr: null, bit_depth: 8 }],
    audio: [{ index: 1, codec: "opus", channels: 2, layout: "Stereo", language: "eng", is_default: true }],
    subtitles: [
      { index: 2, codec: "subrip", format: "srt", renderable: true, language: "eng", is_default: false, is_forced: false, is_hearing_impaired: false },
      { index: 3, codec: "subrip", format: "srt", renderable: true, language: "spa", is_default: false, is_forced: false, is_hearing_impaired: false },
    ],
  },
};

test("lists the mockup's File facts for a version", () => {
  const names: Record<string, string> = { eng: "English", spa: "Spanish" };
  expect(fileFacts(tearsOfSteel, (c) => names[c] ?? c)).toEqual([
    { label: "Video", value: "1920 × 800", detail: "VP9 Profile 0 · 24 fps · SDR" },
    { label: "Audio", value: "English", detail: "Opus · Stereo" },
    { label: "Subtitles", value: "English, Spanish", detail: "SubRip" },
    { label: "Length", value: "12 min 14 s", detail: "7.9 Mb/s" },
    { label: "Location", value: "Tears of Steel (2012).webm", detail: "726 MB" },
  ]);
});

test("untagged tracks leave the language out instead of saying unknown", () => {
  const names: Record<string, string> = { eng: "English" };
  const media = tearsOfSteel.media!;
  const untagged: Version = {
    ...tearsOfSteel,
    media: {
      ...media,
      audio: [{ ...media.audio[0]!, language: null }],
      subtitles: [
        { index: 2, codec: "subrip", format: "srt", renderable: true, language: "und", is_default: false, is_forced: false, is_hearing_impaired: false },
        { index: 3, codec: "subrip", format: "srt", renderable: true, language: "eng", is_default: false, is_forced: false, is_hearing_impaired: false },
      ],
    },
  };
  const facts = fileFacts(untagged, (c) => names[c] ?? c);
  expect(facts.find((f) => f.label === "Audio")).toEqual({ label: "Audio", value: "Opus", detail: "Stereo" });
  expect(facts.find((f) => f.label === "Subtitles")).toEqual({ label: "Subtitles", value: "English", detail: "SubRip" });
});

test("a file that could not be probed has only its location", () => {
  const unreadable: Version = { ...tearsOfSteel, media: null, playback: { kind: "unavailable", reason: "ffprobe failed" } };
  expect(fileFacts(unreadable)).toEqual([{ label: "Location", value: "Tears of Steel (2012).webm", detail: "726 MB" }]);
  expect(versionChips(unreadable)).toEqual(["1080p", "VP9", "Opus 2.0"]);
});

test("chips are the summary segments plus the container", () => {
  expect(versionChips(tearsOfSteel)).toEqual(["1080p", "VP9", "Opus 2.0", "WebM"]);
});
