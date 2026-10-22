import { useLayoutEffect, useRef, useState } from "react";
import type { CastMember, Details } from "@nuvora/core";
import { Artwork } from "./Artwork";
import "../styles/details.css";

/** Genres, rating and studio on one quiet line under the title's meta. */
export function DetailFacts({ details }: { details: Details | undefined }) {
  if (!details) return null;
  const parts = [details.genres.slice(0, 4).join(", "), details.studio].filter(Boolean);
  if (parts.length === 0 && details.rating == null) return null;
  return (
    <p className="detail-facts">
      {details.rating != null && (
        <span className="detail-rating" aria-label={`Rated ${details.rating.toFixed(1)} out of 10`}>
          <svg width="13" height="13" viewBox="0 0 24 24" aria-hidden>
            <path fill="currentColor" d="M12 2.8l2.85 5.78 6.38.93-4.62 4.5 1.09 6.35L12 17.36l-5.7 3 1.09-6.35-4.62-4.5 6.38-.93z" />
          </svg>
          {details.rating.toFixed(1)}
        </span>
      )}
      {parts.map((p) => (
        <span key={p}>{p}</span>
      ))}
    </p>
  );
}

/** The synopsis, four lines until the viewer asks for more. */
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

/** Cast, or voices for anime with a Japanese/English switch, plus where the details came from. */
export function CastSection({ details }: { details: Details | undefined }) {
  const voices = details?.cast.filter((c) => c.kind === "voice") ?? [];
  const languages = [...new Set(voices.map((v) => v.language).filter((l): l is string => !!l))];
  const [language, setLanguage] = useState<string | null>(null);
  if (!details || (details.cast.length === 0 && !details.source)) return null;
  const dub = language && languages.includes(language) ? language : (languages[0] ?? null);
  const people = voices.length > 0 ? voices.filter((v) => v.language === dub) : details.cast;
  return (
    <section className="cast" aria-labelledby="cast-heading">
      <div className="cast-head">
        <h2 id="cast-heading">{voices.length > 0 ? "Characters & voices" : "Cast"}</h2>
        {languages.length > 1 && (
          <div className="segmented" role="group" aria-label="Voice language">
            {languages.map((l) => (
              <button key={l} type="button" aria-pressed={l === dub} onClick={() => setLanguage(l)}>
                {l}
              </button>
            ))}
          </div>
        )}
        {details.source && details.external_url && (
          <a className="cast-source" href={details.external_url} target="_blank" rel="noreferrer">
            Details from {details.source}
          </a>
        )}
      </div>
      {people.length > 0 && (
        <ul className="cast-list" role="list">
          {people.map((person, i) => (
            <CastCard key={`${person.name}-${person.character}-${i}`} person={person} />
          ))}
        </ul>
      )}
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
        {main ? <Artwork image={main} size="small" alt="" fill /> : <span className="cast-initials">{initials(voice ? (person.character ?? person.name) : person.name)}</span>}
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
