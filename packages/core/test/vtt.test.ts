import { expect, test } from "bun:test";
import { activeCues, findLanguage, LANGUAGES, languageCode, languageName, parseVtt } from "../src";

const plain = (text: string) => ({ text, italic: false, bold: false, underline: false });

test("parses ffmpeg's WebVTT output: hours optional, ids, settings, styling", () => {
  const cues = parseVtt(
    "\uFEFFWEBVTT\r\n\r\nNOTE made by ffmpeg\r\n\r\n1\r\n00:00:01.500 --> 00:00:04.000\r\nHello <i>there</i>,\r\n<b>friend</b> &amp; co.\r\n\r\n" +
      "00:05.000 --> 00:07.250 line:0 align:center\n<c.yellow>Top</c> sign\n\n" +
      "01:02:03.004 --> 01:02:04.000\n<v Celia>Late</v>\n",
  );
  expect(cues).toEqual([
    {
      start: 1.5,
      end: 4,
      placement: "bottom",
      lines: [[plain("Hello "), { text: "there", italic: true, bold: false, underline: false }, plain(",")], [{ text: "friend", italic: false, bold: true, underline: false }, plain(" & co.")]],
    },
    { start: 5, end: 7.25, placement: "top", lines: [[plain("Top sign")]] },
    { start: 3723.004, end: 3724, placement: "bottom", lines: [[plain("Late")]] },
  ]);
});

test("drops ruby text, empty cues, backwards timings and junk blocks", () => {
  const cues = parseVtt(
    "WEBVTT\n\nSTYLE\n::cue { color: red }\n\n00:00:01.000 --> 00:00:02.000\n<ruby>漢<rt>かん</rt></ruby>字\n\n" +
      "00:00:03.000 --> 00:00:04.000\n   \n\n00:00:06.000 --> 00:00:05.000\nBackwards\n\nnot a cue\n\n00:00:08,000 --> 00:00:09,000\nComma\n",
  );
  expect(cues.map((c) => [c.start, c.lines.map((l) => l.map((r) => r.text).join(""))])).toEqual([
    [1, ["漢字"]],
    [8, ["Comma"]],
  ]);
});

test("line positions under half the picture go on top", () => {
  const at = (settings: string) => parseVtt(`WEBVTT\n\n00:00:01.000 --> 00:00:02.000 ${settings}\nx\n`)[0]!.placement;
  expect(at("line:10%")).toBe("top");
  expect(at("line:85%")).toBe("bottom");
  expect(at("line:2")).toBe("top");
  expect(at("line:-1")).toBe("bottom");
  expect(at("position:50%")).toBe("bottom");
});

test("activeCues: start is inclusive, end exclusive, overlaps all show", () => {
  const cues = parseVtt("WEBVTT\n\n00:01.000 --> 00:03.000\nA\n\n00:02.000 --> 00:04.000\nB\n\n00:10.000 --> 00:11.000\nC\n");
  const texts = (t: number) => activeCues(cues, t).map((c) => c.lines[0]![0]!.text);
  expect(texts(0.999)).toEqual([]);
  expect(texts(1)).toEqual(["A"]);
  expect(texts(2.5)).toEqual(["A", "B"]);
  expect(texts(3)).toEqual(["B"]);
  expect(texts(4)).toEqual([]);
  expect(texts(10.5)).toEqual(["C"]);
});

test("languages: every ISO 639-1 code, both 639-2 forms, BCP 47 tags", () => {
  expect(LANGUAGES).toHaveLength(183);
  expect(findLanguage("es")).toEqual({ code: "spa", short: "es", name: "Spanish" });
  expect(languageCode("fra")).toBe("fre");
  expect(languageCode("FR-ca")).toBe("fre");
  expect(languageCode("zh_Hans")).toBe("chi");
  expect(languageCode("und")).toBeNull();
  expect(languageCode("")).toBeNull();
  expect(languageName("jpn")).toBe("Japanese");
  expect(languageName("nob")).toBe("Norwegian Bokmål");
  expect(languageName("tlh")).toBe("tlh");
  expect(languageName(null)).toBe("Unknown");
  expect(languageName("und")).toBe("Unknown");
});

test("styling that spans a line break carries over, as ffmpeg writes ASS conversions", () => {
  const [cue] = parseVtt("WEBVTT\n\n00:01.500 --> 00:12.000\n<b>ESTACIÓN\nCENTRAL</b>\n");
  const bold = (text: string) => ({ text, italic: false, bold: true, underline: false });
  expect(cue!.lines).toEqual([[bold("ESTACIÓN")], [bold("CENTRAL")]]);
});
