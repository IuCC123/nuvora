import type { Version } from "@nuvora/core";
import { opensNatively } from "../../platform/deviceProfile";

/** Fetches the player code these versions will need, so pressing Play doesn't wait for it. Loads once; later calls are free. */
export function preloadPlayer(versions: readonly Version[]): void {
  for (const { playback } of versions) {
    if (playback.kind === "hls") void import("hls.js/light").catch(() => {});
    else if (playback.kind === "direct_play" && !opensNatively(playback.mime_type)) void import("../../platform/rewrap").catch(() => {});
  }
}
