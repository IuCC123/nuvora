import { Fragment } from "react";
import { episodeHeading, type EpisodeDetail } from "@nuvora/core";

/** The player's heading: a movie's title and year, or "Show · S1 E4 · Episode title" with the show leading. */
export function PlayerTitle({ title, year, episode }: { title: string | undefined; year: number | null | undefined; episode: EpisodeDetail | undefined }) {
  if (episode) {
    const [show, ...rest] = episodeHeading(episode);
    return (
      <div className="player-heading">
        <h1>
          {show}
          {rest.map((part) => (
            <Fragment key={part}>
              <span className="player-heading-sep"> · </span>
              <span className="player-heading-part">{part}</span>
            </Fragment>
          ))}
        </h1>
      </div>
    );
  }
  if (!title) return null;
  return (
    <div className="player-heading">
      <h1>{title}</h1>
      {year != null && <span className="player-year">{year}</span>}
    </div>
  );
}
