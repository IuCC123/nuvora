import type { DeviceProfile } from "@nuvora/core";

/**
 * Measure what this browser decodes, once at startup, with
 * `MediaSource.isTypeSupported` (falling back to `canPlayType`). The facts go
 * to the server, which makes the playback decision for every client the same way.
 *
 * Probe table (RFC 6381 strings; first match wins per codec / bit depth):
 *   h264  8-bit: avc1.640033            10-bit: avc1.6e0033
 *   hevc  8-bit: hvc1.1.6.L153.B0       10-bit: hvc1.2.4.L153.B0
 *   av1   8-bit: av01.0.08M.08          10-bit: av01.0.08M.10
 *   vp9   8-bit: vp09.00.40.08          10-bit: vp09.02.40.10
 *   aac mp4a.40.2, ac3, ec-3, opus, flac, mp3 (mp4a.6B)
 * Containers: mp4 always; webm via `video/webm`; mkv via `video/x-matroska`
 * (Chromium plays common MKVs but reports "", so mkv is listed only when
 * `canPlayType` says "maybe"/"probably").
 * HDR: `matchMedia("(dynamic-range: high)")` and `screen.colorDepth >= 30`
 * gate hdr10/hlg; Dolby Vision only on Safari via `dvh1.08.06`.
 * Max resolution: 3840x2160 unless `navigator.hardwareConcurrency` and a
 * probe at level 5.1 say otherwise.
 */
export function probeDeviceProfile(): DeviceProfile {
  throw new Error("not implemented: see probe table above");
}
