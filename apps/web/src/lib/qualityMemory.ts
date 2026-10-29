/** For a heavy video with a streaming copy: play the copy or the original. Carries over between sessions on this browser. */
const KEY = "nuvora.player.quality";

export type Quality = "copy" | "original";

export function loadQuality(): Quality {
  try {
    return localStorage.getItem(KEY) === "original" ? "original" : "copy";
  } catch {
    return "copy";
  }
}

export function saveQuality(quality: Quality) {
  try {
    localStorage.setItem(KEY, quality);
  } catch {
    // Storage full or blocked: the choice lasts for this session only.
  }
}
