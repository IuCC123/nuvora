import type { CSSProperties, ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import type { HomeRow, PosterCard } from "@nuvora/core";
import { Artwork } from "../Artwork";
import { CardArt } from "../CardArt";
import { PlayIcon } from "../Icon";
import { PosterMarks } from "../PosterCard";
import { Row } from "../Row";
import { TitleCard, titleCardVars } from "../TitleCard";
import { ambientColor } from "../../lib/ambient";
import { claimSharedArt } from "../../lib/sharedArt";

const rowId = (row: HomeRow) => `row-${row.id ?? row.kind}`;

/**
 * Continue Watching as wide cards: the episode's frame or the title's
 * backdrop, a progress bar, and the episode under the title. The art plays
 * (resuming where the viewer stopped); the title opens the movie or show.
 * `lead` cards (an admin's torrent streams) come first.
 */
export function ContinueRow({ row, lead }: { row: HomeRow; lead: readonly { key: string; card: ReactNode }[] }) {
  return (
    <Row id={rowId(row)} variant="landscape" title={row.title} itemCount={lead.length + row.items.length}>
      {lead.map(({ key, card }) => (
        <li key={key} className="row-item">
          {card}
        </li>
      ))}
      {row.items.map((card) => (
        <li key={card.id} className="row-item">
          <WideCard card={card} />
        </li>
      ))}
    </Row>
  );
}

function WideCard({ card }: { card: PosterCard }) {
  const progress = card.progress != null ? `${Math.max(1, Math.round(card.progress * 100))}% watched` : null;
  const label = [`${card.progress != null ? "Resume" : "Play"} ${card.title}`, card.subtitle, progress].filter(Boolean).join(", ");
  const art = (
    <span className="wide-frame">
      <LandscapeArt card={card} />
      {card.progress != null && (
        <span className="wide-progress" aria-hidden>
          <span style={{ transform: `scaleX(${Math.min(1, Math.max(0.02, card.progress))})` }} />
        </span>
      )}
      <span className="poster-hover" aria-hidden>
        <span className="poster-hover-play glass">
          <PlayIcon size={22} />
        </span>
      </span>
    </span>
  );
  const second = card.subtitle ?? (card.year != null ? String(card.year) : null);
  return (
    <div className="wide-card">
      {card.kind === "episode" ? (
        <Link to="/episode/$id/play" params={{ id: card.id }} search={{}} className="wide-link" data-nav-item aria-label={label}>
          {art}
        </Link>
      ) : (
        <Link to="/movie/$id/play" params={{ id: card.id }} search={{}} className="wide-link" data-nav-item aria-label={label}>
          {art}
        </Link>
      )}
      <span className="wide-caption">
        {card.kind === "episode" && card.show_id != null ? (
          <Link to="/show/$id" params={{ id: card.show_id }} className="wide-title" tabIndex={-1}>
            {card.title}
          </Link>
        ) : (
          <Link to="/movie/$id" params={{ id: card.id }} className="wide-title" tabIndex={-1}>
            {card.title}
          </Link>
        )}
        {second && <span className="wide-sub">{second}</span>}
      </span>
    </div>
  );
}

/**
 * 16:9 art for a card: its backdrop; else its poster, sharp at the left on
 * a blur of itself; else the title card.
 */
export function LandscapeArt({ card }: { card: PosterCard }) {
  if (card.backdrop) return <Artwork image={card.backdrop} size="large" alt="" fill />;
  if (card.poster) {
    return (
      <span className="landscape-poster">
        <Artwork image={card.poster} size="small" alt="" fill />
        <span className="landscape-poster-sharp">
          <Artwork image={card.poster} size="small" alt="" aspect={2 / 3} />
        </span>
      </span>
    );
  }
  return <TitleCard title={card.title} year={card.year} />;
}

/** The household's Top 10: posters with their rank set large behind them. */
export function TopRow({ row }: { row: HomeRow }) {
  return (
    <Row id={rowId(row)} variant="ranked" title={row.title} itemCount={row.items.length}>
      {row.items.map((card, i) => (
        <li key={card.id} className="row-item">
          <RankedCard card={card} rank={i + 1} />
        </li>
      ))}
    </Row>
  );
}

function RankedCard({ card, rank }: { card: PosterCard; rank: number }) {
  const claim = (e: React.MouseEvent<HTMLElement>) => claimSharedArt(e.currentTarget.querySelector<HTMLElement>(".poster-frame"));
  const body = (
    <>
      <span className="rank-numeral" aria-hidden>
        {rank}
      </span>
      <span className="poster-frame">
        <CardArt image={card.poster} title={card.title} year={card.kind === "episode" ? null : card.year} />
        <PosterMarks progress={card.progress ?? null} watched={card.watched} />
      </span>
    </>
  );
  const label = `${rank}. ${card.title}`;
  return card.kind === "show" ? (
    <Link to="/show/$id" params={{ id: card.id }} className="poster-card ranked-card" data-nav-item aria-label={label} onClick={claim}>
      {body}
    </Link>
  ) : (
    <Link to="/movie/$id" params={{ id: card.id }} className="poster-card ranked-card" data-nav-item aria-label={label} onClick={claim}>
      {body}
    </Link>
  );
}

/** Genres as tiles: one title's backdrop washed in its own color, with the genre's name and size. */
export function GenreRow({ row }: { row: HomeRow }) {
  return (
    <Row id={rowId(row)} variant="tile" title={row.title} itemCount={row.genres.length}>
      {row.genres.map((genre) => {
        const tint = ambientColor(genre.art);
        const style = (tint ? { "--tile": tint } : titleCardVars(genre.name)) as CSSProperties;
        return (
          <li key={genre.key} className="row-item">
            <Link to="/genre/$key" params={{ key: genre.key }} className="genre-tile" data-nav-item style={style} data-art={genre.art != null}>
              {genre.art && <Artwork image={genre.art} size="small" alt="" fill />}
              <span className="genre-text">
                <span className="genre-name">{genre.name}</span>
                <span className="genre-count">{genre.count === 1 ? "1 title" : `${genre.count} titles`}</span>
              </span>
            </Link>
          </li>
        );
      })}
    </Row>
  );
}
