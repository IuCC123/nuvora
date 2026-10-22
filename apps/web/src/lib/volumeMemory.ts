/** Volume and mute carry over between sessions on this browser. */
const KEY = "nuvora.player.volume";

export interface VolumeSetting {
  volume: number;
  muted: boolean;
}

export function loadVolume(): VolumeSetting {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (saved && typeof saved === "object") {
      const { volume, muted } = saved as Record<string, unknown>;
      if (typeof volume === "number" && volume >= 0 && volume <= 1 && typeof muted === "boolean") return { volume, muted };
    }
  } catch {
    // Private mode or a corrupt value: fall through to the default.
  }
  return { volume: 1, muted: false };
}

export function saveVolume(setting: VolumeSetting) {
  try {
    localStorage.setItem(KEY, JSON.stringify(setting));
  } catch {
    // Storage full or blocked: the setting lasts for this session only.
  }
}
