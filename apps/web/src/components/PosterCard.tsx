import { Link } from "@tanstack/react-router";
import type { PosterCard } from "@nuvora/core";
import { Artwork } from "./Artwork";
import { PlayIcon } from "./Icon";
import { TitleCard } from "./TitleCard";

/** A poster that links to its detail screen. Hover/focus preloads the item (router `defaultPreload: "intent"`). */
export function PosterCardView({ card }: { card: PosterCard }) {
  return (
    <Link to="/movie/$id" params={{ id: card.id }} className="poster-card">
      <span className="poster-frame">
        {card.poster ? <Artwork image={card.poster} size="small" alt="" /> : <TitleCard title={card.title} year={card.year} />}
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
