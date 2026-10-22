import type { CSSProperties } from "react";
import { titleCardLook } from "@nuvora/core";
import "../styles/title-card.css";

/** The card palette as CSS variables; the detail page's ambient wash reads the same ones. */
export function titleCardVars(title: string): CSSProperties {
  const { palette, bloomX, bloomY } = titleCardLook(title);
  return {
    "--tc-bloom": palette.bloom,
    "--tc-top": palette.top,
    "--tc-bottom": palette.bottom,
    "--tc-x": `${bloomX}%`,
    "--tc-y": `${bloomY}%`,
  } as CSSProperties;
}

/**
 * The poster of a movie without artwork: palette gradient, bloom, grain and
 * vignette, with the title as poster lettering. Pure CSS, so it paints in the
 * first frame. Hidden from assistive tech; the caption or heading names the movie.
 */
export function TitleCard({ title, year, large = false }: { title: string; year?: number | null; large?: boolean }) {
  return (
    <span className={large ? "title-card title-card-large" : "title-card"} style={titleCardVars(title)} aria-hidden>
      <span className="title-card-text">
        <span className="title-card-title">{title}</span>
        {year != null && <span className="title-card-year">{year}</span>}
      </span>
    </span>
  );
}
