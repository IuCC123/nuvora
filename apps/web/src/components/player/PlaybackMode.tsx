import type { Version } from "@nuvora/core";

/** How the file reaches the screen, in a word or two, and why when the server steps in. */
export interface Mode {
  label: string;
  detail: string;
}

export function playbackMode(playback: Version["playback"], rewrapped: boolean): Mode | undefined {
  switch (playback.kind) {
    case "direct_play":
      return {
        label: "Direct play",
        detail: rewrapped ? "The original file, rewrapped for this browser as it plays" : "The original file, as it is",
      };
    case "hls": {
      const why = playback.reasons.join(". ");
      if (playback.converts_video) return { label: "Converting video", detail: why };
      if (playback.converts_audio) return { label: "Converting audio", detail: why };
      return { label: "Direct stream", detail: `The server repackages the file without converting it. ${why}` };
    }
    default:
      return undefined;
  }
}

/** A quiet chip beside the title; hovering it says why. */
export function PlaybackMode({ mode }: { mode: Mode | undefined }) {
  if (!mode) return null;
  return (
    <span className="player-mode" title={mode.detail}>
      {mode.label}
    </span>
  );
}
