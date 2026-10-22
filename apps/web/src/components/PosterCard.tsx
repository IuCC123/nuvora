import { Link } from "@tanstack/react-router";
import type { PosterCard } from "@nuvora/core";
import { CheckIcon, PlayIcon } from "./Icon";
import { CardArt } from "./CardArt";

/**
 * A poster that links to what it shows: a movie's or a show's page. An episode
 * card (Continue Watching) plays the episode from its poster, and its title
 * opens the show. Hover/focus preloads the target (router `defaultPreload: "intent"`).
 */
export function PosterCardView({ card }: { card: PosterCard }) {
  const marks = card.progress != null ? `${Math.max(1, Math.round(card.progress * 100))}% watched` : card.watched ? "Watched" : null;
  const second = card.subtitle ?? (card.year != null ? String(card.year) : " ");
  const frame = (
    <span className="poster-frame">
      <CardArt image={card.poster} title={card.title} year={card.kind === "episode" ? null : card.year} />
      <PosterMarks progress={card.progress ?? null} watched={card.watched} />
      {card.kind === "show" && !card.watched && card.unwatched_count != null && card.unwatched_count > 0 && (
        <span className="poster-count" aria-hidden>
          {card.unwatched_count}
        </span>
      )}
      <span className="poster-hover" aria-hidden>
        <span className="poster-hover-play">
          <PlayIcon size={22} />
        </span>
      </span>
    </span>
  );

  if (card.kind === "episode" && card.show_id != null) {
    return (
      <div className="poster-card-split">
        <Link to="/episode/$id/play" params={{ id: card.id }} search={{}} className="poster-card" aria-label={[`Play ${card.title}`, card.subtitle, marks].filter(Boolean).join(", ")}>
          {frame}
        </Link>
        <span className="poster-caption">
          <Link to="/show/$id" params={{ id: card.show_id }} className="poster-title poster-title-link">
            {card.title}
          </Link>
          <span className="poster-year">{second}</span>
        </span>
      </div>
    );
  }

  const unwatched = card.kind === "show" && card.unwatched_count ? `${card.unwatched_count} unwatched` : null;
  const caption = (
    <span className="poster-caption">
      <span className="poster-title">{card.title}</span>
      <span className="poster-year">{second}</span>
      {(marks ?? unwatched) && <span className="sr-only">{marks ?? unwatched}</span>}
    </span>
  );
  return card.kind === "show" ? (
    <Link to="/show/$id" params={{ id: card.id }} className="poster-card">
      {frame}
      {caption}
    </Link>
  ) : (
    <Link to="/movie/$id" params={{ id: card.id }} className="poster-card">
      {frame}
      {caption}
    </Link>
  );
}

/** An amber bar along the bottom edge while in progress; a check in the corner once watched. Unwatched carries nothing. Hidden from screen readers, so callers say it in words. */
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
