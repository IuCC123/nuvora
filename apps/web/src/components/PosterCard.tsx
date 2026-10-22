import { Link } from "@tanstack/react-router";
import type { PosterCard } from "@nuvora/core";
import { Artwork } from "./Artwork";

/** A poster that links to its detail screen. Hover/focus preloads the item (router `defaultPreload: "intent"`). */
export function PosterCardView({ card }: { card: PosterCard }) {
  const label = card.year ? `${card.title} (${card.year})` : card.title;
  return (
    <Link to="/movie/$id" params={{ id: card.id }} aria-label={label}>
      {card.poster ? <Artwork image={card.poster} size="small" alt="" /> : <TitleCard title={card.title} />}
      <span aria-hidden>{card.title}</span>
    </Link>
  );
}

/** No artwork at all yet (first scan still in the Artwork phase): the title on a token-coloured card. */
function TitleCard({ title }: { title: string }) {
  return <span>{title}</span>;
}
