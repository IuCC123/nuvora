import type { DeviceProfile } from "@nuvora/core";

/** User-Agent Client Hints. Not in TypeScript's DOM lib yet; absent on Safari and Firefox. */
type NavigatorWithHints = Navigator & { userAgentData?: { brands: readonly { brand: string }[] } };

/** RFC 6381 probes per codec, lowest bit depth first. The server's canonical codec names. */
const VIDEO_PROBES: readonly { codec: string; probes: readonly (readonly [bitDepth: number, mime: string])[] }[] = [
  { codec: "h264", probes: [[8, 'video/mp4; codecs="avc1.640033"'], [10, 'video/mp4; codecs="avc1.6e0033"']] },
  { codec: "hevc", probes: [[8, 'video/mp4; codecs="hvc1.1.6.L153.B0"'], [10, 'video/mp4; codecs="hvc1.2.4.L153.B0"']] },
  { codec: "av1", probes: [[8, 'video/mp4; codecs="av01.0.08M.08"'], [10, 'video/mp4; codecs="av01.0.08M.10"']] },
  { codec: "vp9", probes: [[8, 'video/webm; codecs="vp09.00.40.08"'], [10, 'video/webm; codecs="vp09.02.40.10"']] },
];

const AUDIO_PROBES: readonly (readonly [codec: string, mime: string])[] = [
  ["aac", 'audio/mp4; codecs="mp4a.40.2"'],
  ["ac3", 'audio/mp4; codecs="ac-3"'],
  ["eac3", 'audio/mp4; codecs="ec-3"'],
  ["opus", 'audio/webm; codecs="opus"'],
  ["flac", 'audio/mp4; codecs="flac"'],
  ["mp3", 'audio/mp4; codecs="mp4a.6B"'],
];

/**
 * Measure what this browser decodes, once at startup, with
 * `MediaSource.isTypeSupported` (falling back to `canPlayType`). The facts go
 * to the server, which makes the playback decision for every client the same way.
 *
 * Containers: mp4 always; Matroska and WebM wherever the browser opens them
 * itself or can rewrap them here (see `opensNatively` and `platform/rewrap`),
 * so their files play straight from disk instead of through the server. The
 * server's codec checks still apply to the streams inside, and the codec probes
 * ask about MP4, which is what a rewrapped file becomes.
 * HDR: `(dynamic-range: high)` plus a 30-bit screen gates hdr10/hlg; Dolby
 * Vision only where `dvh1.08.06` probes true (Safari).
 */
export function probeDeviceProfile(): DeviceProfile {
  const hdrScreen = matchMedia("(dynamic-range: high)").matches && screen.colorDepth >= 30;
  const dolbyVision = supports('video/mp4; codecs="dvh1.08.06"');
  const video = VIDEO_PROBES.flatMap(({ codec, probes }) => {
    const depths = probes.filter(([, mime]) => supports(mime)).map(([depth]) => depth);
    if (depths.length === 0) return [];
    const maxBitDepth = Math.max(...depths);
    const hdr = hdrScreen && maxBitDepth >= 10 ? ["hdr10", "hlg", ...(dolbyVision && codec === "hevc" ? ["dolby_vision"] : [])] : [];
    return [{ codec, max_width: 3840, max_height: 2160, max_bit_depth: maxBitDepth, hdr }];
  });
  // Browsers downmix multichannel audio themselves, so channel count never blocks playback.
  const audio = AUDIO_PROBES.filter(([, mime]) => supports(mime)).map(([codec]) => ({ codec, max_channels: 8 }));
  const containers = ["mp4", ...(opensWebm() || canRewrap() ? ["webm"] : []), ...(isChromium() || canRewrap() ? ["mkv"] : [])];
  return { containers, video, audio };
}

/**
 * Whether `<video src>` opens a file of this type here. When it doesn't, the
 * player rewraps the file in the browser (`platform/rewrap`) instead.
 */
export function opensNatively(mime: string): boolean {
  if (mime === "video/x-matroska") return isChromium();
  if (mime === "video/webm") return opensWebm();
  return true;
}

/**
 * Whether every file should stream through the server (`hls`), its video copied
 * as it is wherever this browser decodes it, rather than play directly. True for
 * a server reached over the internet: `<video src>` keeps only a few seconds
 * ahead (2.5 s, measured on a 4 Mbit/s episode), so any dip in the connection
 * stalls the picture, while hls.js keeps a minute or two. On the local network
 * the browser opens files itself, which seeks instantly and costs the server
 * nothing. Needs Media Source Extensions, for hls.js.
 */
export function streamsEverything(host: string): boolean {
  if (!mediaSourceClass()) return false;
  return !onLocalNetwork(host.replace(/^\[|\]$/g, "").toLowerCase());
}

function onLocalNetwork(host: string): boolean {
  if (host === "localhost" || host.endsWith(".local") || host.endsWith(".lan") || host.endsWith(".home.arpa")) return true;
  if (/^(127|10)\./.test(host) || /^192\.168\./.test(host) || /^169\.254\./.test(host)) return true;
  if (/^172\.(1[6-9]|2\d|3[01])\./.test(host)) return true;
  return host === "::1" || /^f[cd][0-9a-f]{2}:/.test(host) || /^fe80:/.test(host);
}

/** Whether this browser can rewrap a Matroska or WebM file it can't open itself. */
export function canRewrap(): boolean {
  return mediaSourceClass() !== undefined;
}

function opensWebm(): boolean {
  return supports('video/webm; codecs="vp9"');
}

/** `ManagedMediaSource` is iPhone Safari's Media Source Extensions; it isn't in TypeScript's DOM lib yet. */
export function mediaSourceClass(): typeof MediaSource | undefined {
  if (typeof MediaSource !== "undefined") return MediaSource;
  return (globalThis as { ManagedMediaSource?: typeof MediaSource }).ManagedMediaSource;
}

let probeElement: HTMLVideoElement | undefined;

function supports(mime: string): boolean {
  if (typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(mime)) return true;
  probeElement ??= document.createElement("video");
  return probeElement.canPlayType(mime) === "probably";
}

/**
 * Chromium opens Matroska (H.264, HEVC, AV1, VP9 inside) but reports "" for
 * `video/x-matroska`, so the probe cannot see it. If a file still fails, the
 * player falls back to the server's stream.
 */
function isChromium(): boolean {
  const brands = (navigator as NavigatorWithHints).userAgentData?.brands ?? [];
  return brands.some((b) => b.brand === "Chromium");
}
