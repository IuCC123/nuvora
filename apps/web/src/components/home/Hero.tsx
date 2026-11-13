import { useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { formatRuntime, type FeaturedItem } from "@nuvora/core";
import { Artwork } from "../Artwork";
import { InfoIcon, PlayIcon, StarIcon } from "../Icon";
import { ambientColor } from "../../lib/ambient";
import { claimSharedArt } from "../../lib/sharedArt";
import { reducedMotion } from "../../lib/spatialNav";

/** How long each title holds the hero before the next one fades in. */
const DWELL_MS = 9000;

/**
 * The full-bleed hero: the featured titles' backdrops drifting slowly (Ken
 * Burns), crossfading every few seconds, with the title, its facts and
 * Play / More Info on the lower left. It pauses while the pointer or focus
 * is on it, while the tab is hidden, and for reduced motion; the dots pick
 * a title. The active title's art colors the page's ambient glow.
 */
export function Hero({ items }: { items: FeaturedItem[] }) {
  const [index, setIndex] = useState(0);
  const [previous, setPrevious] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const root = useRef<HTMLElement>(null);
  const count = items.length;
  const go = (next: number) => {
    setPrevious(index);
    setIndex((next + count) % count);
  };

  useEffect(() => {
    if (count < 2 || paused || reducedMotion()) return;
    const timer = window.setTimeout(() => {
      if (document.visibilityState === "visible") go(index + 1);
    }, DWELL_MS);
    return () => window.clearTimeout(timer);
  });

  const active = items[index]!;
  useEffect(() => {
    const color = ambientColor(active.card.backdrop);
    const home = root.current?.closest<HTMLElement>(".home");
    if (home && color) home.style.setProperty("--ambient", color);
  }, [active]);

  return (
    <section
      ref={root}
      className="hero"
      aria-roledescription="carousel"
      aria-label="Featured"
      onPointerEnter={() => setPaused(true)}
      onPointerLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={(e) => !e.currentTarget.contains(e.relatedTarget) && setPaused(false)}
    >
      <div className="hero-art" aria-hidden>
        {items.map((item, i) =>
          i === index || i === previous || i === (index + 1) % count ? (
            <div key={item.card.id} className="hero-slide" data-state={i === index ? "active" : i === previous ? "leaving" : "next"}>
              <Artwork image={item.card.backdrop!} size="large" alt="" fill eager={i === 0} />
            </div>
          ) : null,
        )}
      </div>
      <div className="hero-scrim" aria-hidden />
      <HeroInfo key={active.card.id} item={active} />
      {count > 1 && (
        <div className="hero-dots glass-thin" role="group" aria-label="Featured titles">
          {items.map((item, i) => (
            <button
              key={item.card.id}
              type="button"
              className="hero-dot"
              aria-label={item.card.title}
              aria-current={i === index ? "true" : undefined}
              onClick={() => i !== index && go(i)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function HeroInfo({ item }: { item: FeaturedItem }) {
  const { card } = item;
  // Facts marked `wide` drop out on phones, so the line stays on one row.
  const facts = [
    { text: item.genres[0], wide: false },
    { text: item.genres[1], wide: true },
    { text: card.year != null ? String(card.year) : undefined, wide: false },
    { text: item.season_count != null ? (item.season_count === 1 ? "1 Season" : `${item.season_count} Seasons`) : undefined, wide: true },
    { text: item.runtime_ms != null ? formatRuntime(item.runtime_ms) : undefined, wide: true },
  ].filter((f): f is { text: string; wide: boolean } => f.text != null);
  const claim = () => claimSharedArt(document.querySelector<HTMLElement>('.hero-slide[data-state="active"]'));
  return (
    <div className="hero-info">
      <p className="hero-facts">
        <span className="hero-kind">{card.kind === "show" ? "TV Show" : "Movie"}</span>
        {facts.map((fact) => (
          <span key={fact.text} data-wide={fact.wide || undefined}>
            {fact.text}
          </span>
        ))}
        {item.rating != null && (
          <span className="hero-rating">
            <StarIcon size={13} />
            {item.rating.toFixed(1)}
          </span>
        )}
      </p>
      <h2 className="hero-title">{card.title}</h2>
      {(item.overview ?? item.tagline) && <p className="hero-overview">{item.overview ?? item.tagline}</p>}
      <div className="hero-actions" data-nav-group>
        <PlayButton item={item} />
        {card.kind === "show" ? (
          <Link to="/show/$id" params={{ id: card.id }} className="btn btn-glass" data-nav-item onClick={claim}>
            <InfoIcon size={20} />
            More Info
          </Link>
        ) : (
          <Link to="/movie/$id" params={{ id: card.id }} className="btn btn-glass" data-nav-item onClick={claim}>
            <InfoIcon size={20} />
            More Info
          </Link>
        )}
      </div>
    </div>
  );
}

/** Resume where the viewer stopped, or play from the start: a movie resolves its file on the way; a show plays its next episode. */
function PlayButton({ item }: { item: FeaturedItem }) {
  const { card, next } = item;
  if (card.kind === "show") {
    if (!next) {
      return (
        <Link to="/show/$id" params={{ id: card.id }} className="btn btn-primary" data-nav-item>
          <PlayIcon size={20} />
          Episodes
        </Link>
      );
    }
    return (
      <Link to="/episode/$id/play" params={{ id: next.id }} search={{}} className="btn btn-primary" data-nav-item aria-label={`${next.position_ms != null ? "Resume" : "Play"} ${card.title}, ${next.label}`}>
        <PlayIcon size={20} />
        {next.position_ms != null ? "Resume" : "Play"}
        <small>{next.label}</small>
      </Link>
    );
  }
  return (
    <Link to="/movie/$id/play" params={{ id: card.id }} search={{}} className="btn btn-primary" data-nav-item>
      <PlayIcon size={20} />
      {card.progress != null ? "Resume" : "Play"}
    </Link>
  );
}
