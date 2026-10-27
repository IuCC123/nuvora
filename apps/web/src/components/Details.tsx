import { Fragment, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import type { CastMember, Details, Image } from "@nuvora/core";
import { Artwork } from "./Artwork";
import { ChevronLeftIcon, ChevronRightIcon } from "./Icon";
import { pageStrip, useScrollEdges } from "./PosterRow";
import "../styles/home.css";
import "../styles/details.css";

/**
 * The top of a detail page, Apple TV style: the backdrop fills the screen and
 * the title, facts, actions and synopsis sit on its lower left. Its bottom
 * edge melts into a blur of the same art and then the canvas. Without a
 * backdrop, the title card's palette washes the hero and `poster` stands on
 * its right.
 */
export function DetailHero({
  title,
  backdrop,
  poster,
  children,
}: {
  title: string | undefined;
  backdrop: Image | null | undefined;
  poster?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header className="hero">
      <div className="hero-art" aria-hidden>
        {backdrop ? (
          <>
            <Artwork image={backdrop} size="large" alt="" fill eager />
            <span className="hero-blur">
              <Artwork image={backdrop} size="small" alt="" fill eager />
            </span>
          </>
        ) : (
          <div className="detail-wash" />
        )}
        <div className="hero-scrim" />
      </div>
      <div className="hero-body">
        {title && <h1 className="hero-title">{title}</h1>}
        {children}
      </div>
      {poster && <div className="hero-poster">{poster}</div>}
    </header>
  );
}

/** Genre, year and length on one line, then the rating and the technical badges. */
export function HeroMeta({ parts, rating, chips = [] }: { parts: ReactNode[]; rating?: number | null; chips?: string[] }) {
  return (
    <div className="hero-meta">
      {parts.map((part, i) => (
        <Fragment key={i}>
          {i > 0 && (
            <span className="hero-sep" aria-hidden>
              ·
            </span>
          )}
          <span>{part}</span>
        </Fragment>
      ))}
      {rating != null && (
        <span className="hero-rating" aria-label={`Rated ${rating.toFixed(1)} out of 10`}>
          <svg width="13" height="13" viewBox="0 0 24 24" aria-hidden>
            <path fill="currentColor" d="M12 2.8l2.85 5.78 6.38.93-4.62 4.5 1.09 6.35L12 17.36l-5.7 3 1.09-6.35-4.62-4.5 6.38-.93z" />
          </svg>
          {rating.toFixed(1)}
        </span>
      )}
      {chips.length > 0 && (
        <ul className="chips" role="list" aria-label="Technical details">
          {chips.map((chip) => (
            <li key={chip} className="chip">
              {chip}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The synopsis, three lines until the viewer asks for more. */
export function Overview({ text, tagline }: { text: string | null | undefined; tagline?: string | null }) {
  const [open, setOpen] = useState(false);
  const [clamped, setClamped] = useState(false);
  const ref = useRef<HTMLParagraphElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && !open) setClamped(el.scrollHeight > el.clientHeight + 1);
  }, [text, open]);
  if (!text) return null;
  return (
    <div className="overview">
      {tagline && <p className="overview-tagline">{tagline}</p>}
      <p ref={ref} className="detail-overview" data-open={open}>
        {text}
      </p>
      {(clamped || open) && (
        <button type="button" className="overview-more" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
          {open ? "Less" : "More"}
        </button>
      )}
    </div>
  );
}

/** "Starring" and the first three billed actors, under the synopsis. Anime, which lists voices, leaves it to the cast row. */
export function Starring({ details }: { details: Details | undefined }) {
  const names = [...new Set(details?.cast.filter((c) => c.kind === "cast").map((c) => c.name))].slice(0, 3);
  if (names.length === 0) return null;
  return (
    <p className="hero-starring">
      <span>Starring</span> {names.join(", ")}
    </p>
  );
}

/** Back and forward buttons for a horizontal strip, each paging by the cards that fit. */
export function ShelfArrows({ strip, label, itemCount }: { strip: RefObject<HTMLElement | null>; label: string; itemCount: number }) {
  const { atStart, atEnd } = useScrollEdges(strip, itemCount);
  if (atStart && atEnd) return null;
  return (
    <div className="shelf-arrows">
      <button type="button" className="round-btn" disabled={atStart} onClick={() => pageStrip(strip.current, -1)} aria-label={`Scroll ${label} back`}>
        <ChevronLeftIcon />
      </button>
      <button type="button" className="round-btn" disabled={atEnd} onClick={() => pageStrip(strip.current, 1)} aria-label={`Scroll ${label} forward`}>
        <ChevronRightIcon />
      </button>
    </div>
  );
}

/** Left and Right walk the strip's links, the way they walk a poster row. */
export function walkStrip(e: KeyboardEvent<HTMLElement>, selector: string) {
  if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>(selector)];
  const at = items.indexOf(document.activeElement as HTMLElement);
  const next = items[at + (e.key === "ArrowRight" ? 1 : -1)];
  if (at < 0 || !next) return;
  e.preventDefault();
  next.focus();
  next.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** Cast, or voices for anime with a Japanese/English switch, in round portraits on one row. */
export function CastSection({ details }: { details: Details | undefined }) {
  const voices = details?.cast.filter((c) => c.kind === "voice") ?? [];
  const languages = [...new Set(voices.map((v) => v.language).filter((l): l is string => !!l))];
  const [language, setLanguage] = useState<string | null>(null);
  const strip = useRef<HTMLUListElement>(null);
  if (!details || details.cast.length === 0) return null;
  const dub = language && languages.includes(language) ? language : (languages[0] ?? null);
  const people = voices.length > 0 ? voices.filter((v) => v.language === dub) : details.cast;
  const title = voices.length > 0 ? "Characters & voices" : "Cast";
  return (
    <section className="shelf cast" aria-labelledby="cast-heading">
      <div className="shelf-head">
        <h2 id="cast-heading">{title}</h2>
        {languages.length > 1 && (
          <div className="segmented" role="group" aria-label="Voice language">
            {languages.map((l) => (
              <button key={l} type="button" aria-pressed={l === dub} onClick={() => setLanguage(l)}>
                {l}
              </button>
            ))}
          </div>
        )}
        <ShelfArrows strip={strip} label={title} itemCount={people.length} />
      </div>
      <ul ref={strip} className="shelf-strip cast-list" role="list">
        {people.map((person, i) => (
          <CastCard key={`${person.name}-${person.character}-${i}`} person={person} />
        ))}
      </ul>
    </section>
  );
}

function CastCard({ person }: { person: CastMember }) {
  const voice = person.kind === "voice";
  const main = voice ? (person.character_image ?? person.image) : (person.image ?? person.character_image);
  const inset = voice && person.character_image ? person.image : null;
  return (
    <li className="cast-card">
      <span className="cast-photo">
        <span className="cast-face">
          {main ? <Artwork image={main} size="small" alt="" fill /> : <span className="cast-initials">{initials(voice ? (person.character ?? person.name) : person.name)}</span>}
        </span>
        {inset && (
          <span className="cast-inset">
            <Artwork image={inset} size="small" alt="" fill />
          </span>
        )}
      </span>
      <span className="cast-name">{voice ? (person.character ?? person.name) : person.name}</span>
      <span className="cast-role">{voice ? person.name : person.character}</span>
    </li>
  );
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("");
}

export interface InfoRow {
  label: string;
  value: ReactNode;
  /** Quieter text after the value. */
  detail?: string | undefined;
}

/** The closing "Information" block: a few titled columns of label-over-value facts, and where the details came from. */
export function InfoSection({ columns, details }: { columns: { title: string; rows: InfoRow[] }[]; details: Details | undefined }) {
  const shown = columns.filter((c) => c.rows.length > 0);
  if (shown.length === 0) return null;
  return (
    <section className="info" aria-labelledby="info-heading">
      <h2 id="info-heading">Information</h2>
      <div className="info-grid">
        {shown.map((column) => (
          <div key={column.title} className="info-column">
            <h3>{column.title}</h3>
            <dl>
              {column.rows.map((row) => (
                <div key={row.label} className="info-row">
                  <dt>{row.label}</dt>
                  <dd>
                    {row.value}
                    {row.detail && <span className="info-detail"> · {row.detail}</span>}
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        ))}
      </div>
      {details?.source && details.external_url && (
        <a className="info-source" href={details.external_url} target="_blank" rel="noreferrer">
          Details from {details.source}
        </a>
      )}
    </section>
  );
}

/** The Information column every title has: genres, studio, rating and the original title. */
export function aboutRows(details: Details | undefined, title: string): InfoRow[] {
  if (!details) return [];
  const rows: InfoRow[] = [];
  if (details.genres.length > 0) rows.push({ label: details.genres.length > 1 ? "Genres" : "Genre", value: details.genres.join(", ") });
  if (details.studio) rows.push({ label: "Studio", value: details.studio });
  if (details.rating != null) rows.push({ label: "Rating", value: `${details.rating.toFixed(1)} / 10`, detail: details.source ?? undefined });
  if (details.original_title && details.original_title !== title) rows.push({ label: "Original title", value: details.original_title });
  return rows;
}
