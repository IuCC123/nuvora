import type { Image } from "@nuvora/core";
import { Artwork } from "./Artwork";
import { TitleCard, titleCardVars } from "./TitleCard";

/**
 * What a 2:3 poster slot shows, best first: a real poster; else a frame from
 * the video (any landscape image), cropped to fill with the title lettered
 * over it; else the typographic title card.
 */
export function CardArt({
  image,
  title,
  year,
  large = false,
  eager = false,
}: {
  image: Image | null | undefined;
  title: string;
  year?: number | null;
  large?: boolean;
  eager?: boolean;
}) {
  if (!image) return <TitleCard title={title} year={year} large={large} />;
  if (image.aspect_ratio <= 1) return <Artwork image={image} size={large ? "large" : "small"} alt="" eager={eager} />;
  return (
    <span className={large ? "title-card title-card-large frame-card" : "title-card frame-card"} style={titleCardVars(title)} aria-hidden>
      <Artwork image={image} size={large ? "large" : "small"} alt="" fill eager={eager} />
      <span className="title-card-text">
        <span className="title-card-title">{title}</span>
        {year != null && <span className="title-card-year">{year}</span>}
      </span>
    </span>
  );
}
