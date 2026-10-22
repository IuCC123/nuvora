import type { Version } from "./client";

/**
 * Display strings every client shows the same way. Pure: the one platform
 * fact, how to name a language, is passed in (the web uses `Intl.DisplayNames`).
 */

/** `"12 min"`, `"1 h 52 min"`. Rounds to the nearest minute; under a minute reads in seconds, `"20 s"`. */
export function formatRuntime(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.round(ms / 60_000);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** `"12 min 14 s"`, `"1 h 52 min 3 s"`. */
export function formatDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return [h && `${h} h`, (h || m) && `${m} min`, `${s} s`].filter(Boolean).join(" ");
}

/** Decimal units, as file managers show them: `"726 MB"`, `"4.2 GB"`. */
export function formatBytes(bytes: number): string {
  return scaled(bytes, ["B", "kB", "MB", "GB", "TB"], " ");
}

/** `"7.9 Mb/s"`, `"640 kb/s"`. */
export function formatBitrate(bitsPerSecond: number): string {
  return scaled(bitsPerSecond, ["b/s", "kb/s", "Mb/s", "Gb/s"], " ");
}

/** `"24 fps"`, `"23.976 fps"`. */
export function formatFrameRate(fps: number): string {
  return `${Number(fps.toFixed(3))} fps`;
}

function scaled(n: number, units: readonly string[], sep: string): string {
  let i = 0;
  while (n >= 1000 && i < units.length - 1) {
    n /= 1000;
    i++;
  }
  const digits = i === 0 || n >= 10 ? 0 : 1;
  return `${n.toFixed(digits)}${sep}${units[i]}`;
}

/** Wire container name -> label: `mkv` -> `"MKV"`, `webm` -> `"WebM"`. */
export function containerLabel(container: string): string {
  return CONTAINERS[container] ?? container.toUpperCase();
}

const CONTAINERS: Record<string, string> = { mkv: "MKV", mp4: "MP4", webm: "WebM", mov: "MOV", avi: "AVI", ts: "TS" };

/**
 * Technical badges for a version: the server's `summary` segments
 * (`"4K · HEVC Main 10 · Dolby Vision · TrueHD 7.1"`), then the container.
 */
export function versionChips(version: Version): string[] {
  const chips = version.summary.split(" · ").map((s) => s.trim()).filter(Boolean);
  if (version.media) chips.push(containerLabel(version.media.container));
  return chips;
}

export interface FileFact {
  label: "Video" | "Audio" | "Subtitles" | "Length" | "Location";
  value: string;
  /** Secondary text after the value, already joined with `" · "`. */
  detail?: string;
}

export type LanguageName = (code: string) => string;

/** The File list on a detail screen: one row per kind of fact the version has. */
export function fileFacts(version: Version, languageName: LanguageName = (c) => c): FileFact[] {
  const facts: FileFact[] = [];
  const media = version.media;
  const video = media?.video[0];
  if (video) {
    facts.push(
      fact("Video", [
        `${video.width} × ${video.height}`,
        [video.codec, video.profile].filter(Boolean).join(" "),
        video.frame_rate ? formatFrameRate(video.frame_rate) : null,
        video.hdr ?? "SDR",
      ]),
    );
  }
  if (media?.audio.length) {
    const main = media.audio.find((a) => a.is_default) ?? media.audio[0]!;
    facts.push(
      fact("Audio", [languages(media.audio, languageName), codecLabel(main.codec, AUDIO_CODECS), main.layout ?? channelLayout(main.channels)]),
    );
  }
  if (media?.subtitles.length) {
    const codecs = unique(media.subtitles.map((s) => codecLabel(s.codec, SUBTITLE_CODECS)));
    facts.push(fact("Subtitles", [languages(media.subtitles, languageName), codecs.join(", ")]));
  }
  if (media?.duration_ms) {
    facts.push(fact("Length", [formatDuration(media.duration_ms), media.bitrate ? formatBitrate(media.bitrate) : null]));
  }
  facts.push(fact("Location", [version.file_name, formatBytes(version.size_bytes)]));
  return facts;
}

/** The first non-empty part is the value, the rest the detail. Every caller's parts include at least one string. */
function fact(label: FileFact["label"], parts: readonly (string | null | undefined)[]): FileFact {
  const [value = "", ...detail] = parts.filter((p): p is string => !!p);
  const text = detail.join(" · ");
  return text ? { label, value, detail: text } : { label, value };
}

/** The tracks' languages, named. Untagged tracks are left out rather than called "unknown". */
function languages(tracks: readonly { language?: string | null }[], name: LanguageName): string {
  const tagged = tracks.flatMap((t) => (t.language && t.language !== "und" ? [name(t.language)] : []));
  return unique(tagged).join(", ");
}

function unique(values: readonly string[]): string[] {
  return [...new Set(values)];
}

function channelLayout(channels: number): string {
  if (channels === 1) return "Mono";
  if (channels === 2) return "Stereo";
  return `${channels} ch`;
}

/** Codec names as ffprobe reports them -> display names. Unknown names pass through. */
function codecLabel(codec: string, table: Record<string, string>): string {
  return table[codec.toLowerCase()] ?? codec;
}

const AUDIO_CODECS: Record<string, string> = {
  aac: "AAC",
  ac3: "AC3",
  eac3: "E-AC3",
  truehd: "TrueHD",
  dts: "DTS",
  opus: "Opus",
  vorbis: "Vorbis",
  flac: "FLAC",
  mp3: "MP3",
  alac: "ALAC",
};

const SUBTITLE_CODECS: Record<string, string> = {
  subrip: "SubRip",
  srt: "SubRip",
  ass: "ASS",
  ssa: "SSA",
  webvtt: "WebVTT",
  mov_text: "MP4 Timed Text",
  hdmv_pgs_subtitle: "PGS",
  pgs: "PGS",
  dvd_subtitle: "VobSub",
  dvb_subtitle: "DVB",
};

/** Up to two letters for an avatar: `"Jamie Rivera"` -> `"JR"`, `"jamie"` -> `"J"`. */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  const first = (w: string | undefined) => (w ? [...w][0]!.toLocaleUpperCase() : "");
  if (words.length === 0) return "?";
  return words.length === 1 ? first(words[0]) : first(words[0]) + first(words[words.length - 1]);
}

/** Server uptime: `"3 days, 4 hours"`, `"2 hours, 5 minutes"`, `"40 seconds"`. The two largest units at most. */
export function formatUptime(seconds: number): string {
  let rest = Math.max(0, Math.floor(seconds));
  const counts = UPTIME_UNITS.map(([unit, size]) => {
    const n = Math.floor(rest / size);
    rest -= n * size;
    return [unit, n] as const;
  });
  const lead = counts.findIndex(([, n]) => n > 0);
  if (lead < 0) return "0 seconds";
  return counts
    .slice(lead, lead + 2)
    .filter(([, n]) => n > 0)
    .map(([unit, n]) => `${n} ${unit}${n === 1 ? "" : "s"}`)
    .join(", ");
}

const UPTIME_UNITS: readonly (readonly [string, number])[] = [
  ["day", 86_400],
  ["hour", 3_600],
  ["minute", 60],
  ["second", 1],
];

/**
 * `"Just now"`, `"12 minutes ago"`, `"Yesterday"`, `"4 days ago"`, then a date
 * (`"3 Mar 2026"`). Both arguments are Unix milliseconds; `null` reads `"Never"`.
 */
export function formatLastSeen(at: number | null, now: number): string {
  if (at == null) return "Never";
  const minutes = Math.floor((now - at) / 60_000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return plural(minutes, "minute");
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return plural(hours, "hour");
  const days = Math.floor(hours / 24);
  if (days === 1) return "Yesterday";
  if (days < 7) return plural(days, "day");
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short", year: "numeric" }).format(at);
}

function plural(n: number, unit: string): string {
  return `${n} ${unit}${n === 1 ? "" : "s"} ago`;
}

/** Player clock: `"0:07"`, `"42:10"`, `"1:02:03"`. Floors, so the clock never runs ahead of the picture. */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
