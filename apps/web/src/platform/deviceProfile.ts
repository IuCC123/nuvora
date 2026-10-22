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
 * Containers: mp4 always; webm via `video/webm`; mkv on Chromium (see
 * `isChromium`). The server's codec checks still apply to the streams inside.
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
  const containers = ["mp4", ...(supports('video/webm; codecs="vp9"') ? ["webm"] : []), ...(isChromium() ? ["mkv"] : [])];
  return { containers, video, audio };
}

let probeElement: HTMLVideoElement | undefined;

function supports(mime: string): boolean {
  if (typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(mime)) return true;
  probeElement ??= document.createElement("video");
  return probeElement.canPlayType(mime) === "probably";
}

/**
 * Chromium plays H.264/AAC Matroska but reports "" for `video/x-matroska`, so
 * the probe cannot see it. Most libraries are MKV, so declaring it keeps them
 * playable before Step 3. If a file still fails, the player's `error` event
 * shows the same "can't play" notice as a server-side refusal.
 */
function isChromium(): boolean {
  const brands = (navigator as NavigatorWithHints).userAgentData?.brands ?? [];
  return brands.some((b) => b.brand === "Chromium");
}
