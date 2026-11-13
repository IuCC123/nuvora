import { useRef, type CSSProperties } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Link, useSearch } from "@tanstack/react-router";
import type { SearchEpisode, SearchPerson } from "@nuvora/core";
import { Artwork } from "../components/Artwork";
import { PosterCardView } from "../components/PosterCard";
import { SearchBox } from "../components/SearchBox";
import { titleCardVars } from "../components/TitleCard";
import { ambientColor } from "../lib/ambient";
import { useSpatialNav } from "../lib/spatialNav";
import "../styles/rows.css";
import { queries } from "../nuvora";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { ErrorState } from "./ErrorState";
import "../styles/search.css";

/**
 * Search: a large glass field that stays put while results change under it
 * as you type. Before a query, genres to browse; then movies and shows as
 * posters, episodes by their own titles, and the people in them. The
 * previous results stay up while the next ones load, so typing never
 * flashes empty.
 */
export function Search() {
  const { q = "" } = useSearch({ from: "/shell/search" });
  const query = q.trim();
  useDocumentTitle(query ? `“${query}”` : "Search");
  const root = useRef<HTMLDivElement>(null);
  useSpatialNav(root);
  return (
    <div ref={root} className="search-page">
      <h1 className="search-title">Search</h1>
      <SearchBox />
      {query ? <Results query={query} /> : <Browse />}
    </div>
  );
}

/** Before anything is typed: the library's genres as tiles. */
function Browse() {
  const { data } = useQuery(queries.home());
  const genres = data?.rows.find((r) => r.kind === "genres")?.genres ?? [];
  if (!genres.length) {
    return (
      <div className="search-empty">
        <p>Find movies and shows by any of their titles, episodes by name, and the actors and voice actors in them.</p>
      </div>
    );
  }
  return (
    <section className="search-section" aria-labelledby="search-browse">
      <h2 id="search-browse">Browse by Genre</h2>
      <ul className="search-genres" role="list" data-nav-group>
        {genres.map((genre) => {
          const tint = ambientColor(genre.art);
          const style = (tint ? { "--tile": tint } : titleCardVars(genre.name)) as CSSProperties;
          return (
            <li key={genre.key}>
              <Link to="/genre/$key" params={{ key: genre.key }} className="genre-tile" data-nav-item style={style}>
                {genre.art && <Artwork image={genre.art} size="small" alt="" fill />}
                <span className="genre-text">
                  <span className="genre-name">{genre.name}</span>
                  <span className="genre-count">{genre.count === 1 ? "1 title" : `${genre.count} titles`}</span>
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function Results({ query }: { query: string }) {
  const { data, isError, refetch, isPlaceholderData } = useQuery({
    ...queries.search(query),
    placeholderData: keepPreviousData,
  });
  if (isError) return <ErrorState title="Search didn't work" detail="The server couldn't be reached." retry={() => void refetch()} />;
  if (!data) return <ResultsSkeleton />;
  const nothing = data.titles.length === 0 && data.episodes.length === 0 && data.people.length === 0;
  return (
    <div className="search-results" aria-busy={isPlaceholderData} aria-live="polite">
      {nothing && (
        <div className="search-empty">
          <h2>
            No results for <q>{query}</q>
          </h2>
          <p>Try fewer words, or the start of a word.</p>
        </div>
      )}
      {data.titles.length > 0 && (
        <section className="search-section" aria-labelledby="search-titles">
          <h2 id="search-titles">
            Movies & Shows <span className="search-count">{data.titles.length}</span>
          </h2>
          <ul className="search-grid" role="list" data-nav-group>
            {data.titles.map((card, i) => (
              <li key={card.id} style={{ "--i": Math.min(i, 12) } as CSSProperties}>
                <PosterCardView card={card} />
              </li>
            ))}
          </ul>
        </section>
      )}
      {data.episodes.length > 0 && (
        <section className="search-section" aria-labelledby="search-episodes">
          <h2 id="search-episodes">
            Episodes <span className="search-count">{data.episodes.length}</span>
          </h2>
          <ul className="search-list" role="list">
            {data.episodes.map((episode) => (
              <li key={episode.id}>
                <EpisodeResult episode={episode} />
              </li>
            ))}
          </ul>
        </section>
      )}
      {data.people.length > 0 && (
        <section className="search-section" aria-labelledby="search-people">
          <h2 id="search-people">
            People <span className="search-count">{data.people.length}</span>
          </h2>
          <ul className="search-people" role="list">
            {data.people.map((person) => (
              <li key={person.name}>
                <PersonResult person={person} />
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

/** The first answer's shape while it loads: a row of shimmering posters. */
function ResultsSkeleton() {
  return (
    <div className="search-results" aria-busy="true" aria-label="Searching">
      <section className="search-section">
        <span className="skeleton skeleton-text" style={{ width: 160, height: 18 }} />
        <ul className="search-grid" role="list">
          {Array.from({ length: 6 }, (_, i) => (
            <li key={i}>
              <span className="poster-frame skeleton" />
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function EpisodeResult({ episode }: { episode: SearchEpisode }) {
  const label = episode.season === 0 ? `Special ${episode.episode}` : `S${episode.season} · E${episode.episode}`;
  return (
    <Link to="/show/$id" params={{ id: episode.show_id }} search={{ season: episode.season }} className="search-row">
      <span className="search-thumb">{episode.poster && <Artwork image={episode.poster} size="small" alt="" fill />}</span>
      <span className="search-row-text">
        <span className="search-row-title">{episode.title}</span>
        <span className="search-row-detail">
          {episode.show_title} · {label}
        </span>
      </span>
    </Link>
  );
}

function PersonResult({ person }: { person: SearchPerson }) {
  return (
    <div className="search-person">
      <span className="search-face">
        {person.image ? <Artwork image={person.image} size="small" alt="" fill /> : <span aria-hidden>{initials(person.name)}</span>}
      </span>
      <div className="search-person-text">
        <h3>{person.name}</h3>
        <ul>
          {person.roles.map(({ card, character }) => (
            <li key={card.id}>
              <Link to={card.kind === "show" ? "/show/$id" : "/movie/$id"} params={{ id: card.id }}>
                {card.title}
              </Link>
              {character && <span className="search-character"> as {character}</span>}
            </li>
          ))}
        </ul>
      </div>
    </div>
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
