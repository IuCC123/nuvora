import type { EpisodeRef } from "@nuvora/core";
import { SkipNextIcon } from "../Icon";

/** Skips to the next episode. Self-contained so any cluster of the control bar can host it. */
export function NextEpisodeButton({ next, onPlay }: { next: EpisodeRef; onPlay: (next: EpisodeRef) => void }) {
  return (
    <button
      type="button"
      className="player-btn"
      aria-label={`Next episode, ${next.label}`}
      data-tip={`Next · ${next.label}`}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => onPlay(next)}
    >
      <SkipNextIcon />
    </button>
  );
}
