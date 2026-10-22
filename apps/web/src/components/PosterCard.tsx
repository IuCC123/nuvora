import { Link } from "@tanstack/react-router";
import type { PosterCard } from "@nuvora/core";
import { Artwork } from "./Artwork";
import { CheckIcon, PlayIcon } from "./Icon";
import { TitleCard } from "./TitleCard";

/** A poster that links to its detail screen. Hover/focus preloads the item (router `defaultPreload: "intent"`). */
export function PosterCardView({ card }: { card: PosterCard }) {
  return (
    <Link to="/movie/$id" params={{ id: card.id }} className="poster-card">
      <span className="poster-frame">
        {card.poster ? <Artwork image={card.poster} size="small" alt="" /> : <TitleCard title={card.title} year={card.year} />}
        <PosterMarks progress={card.progress ?? null} watched={card.watched} />
        <span className="poster-hover" aria-hidden>
          <span className="poster-hover-play">
            <PlayIcon size={22} />
          </span>
        </span>
      </span>
      <span className="poster-caption">
        <span className="poster-title">{card.title}</span>
        <span className="poster-year">{card.year ?? " "}</span>
      </span>
    </Link>
  );
}

/** An amber bar along the bottom edge while in progress; a check in the corner once watched. Unwatched carries nothing. */
export function PosterMarks({ progress, watched }: { progress: number | null; watched: boolean }) {
  return (
    <>
      {progress != null && (
        <span className="poster-progress" aria-hidden>
          <span style={{ transform: `scaleX(${Math.min(1, Math.max(0.02, progress))})` }} />
        </span>
      )}
      {watched && progress == null && (
        <span className="poster-watched" aria-hidden>
          <CheckIcon size={13} strokeWidth={3} />
        </span>
      )}
    </>
  );
}
