import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Link, useSearch } from "@tanstack/react-router";
import type { SearchEpisode, SearchPerson } from "@nuvora/core";
import { Artwork } from "../components/Artwork";
import { SearchIcon } from "../components/Icon";
import { PosterCardView } from "../components/PosterCard";
import { queries } from "../nuvora";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { ErrorState } from "./ErrorState";
import "../styles/search.css";

/**
 * Results for the top bar's search: movies and shows as posters, then
 * episodes by their own titles, then the people in them. The previous
 * results stay up while the next ones load, so typing never flashes empty.
 */
export function Search() {
  const { q = "" } = useSearch({ from: "/shell/search" });
  const query = q.trim();
  useDocumentTitle(query ? `“${query}”` : "Search");
  const { data, isError, refetch, isPlaceholderData } = useQuery({
    ...queries.search(query),
    enabled: query !== "",
    placeholderData: keepPreviousData,
  });

  if (!query) {
    return (
      <div className="search-page">
        <div className="search-empty">
          <SearchIcon size={28} />
          <h1>Search your libraries</h1>
          <p>Find movies and shows by any of their titles, episodes by name, and the actors and voice actors in them.</p>
        </div>
      </div>
    );
  }
  if (isError) return <ErrorState title="Search didn't work" detail="The server couldn't be reached." retry={() => void refetch()} />;
  if (!data) return <div className="search-page" aria-busy />;

  const nothing = data.titles.length === 0 && data.episodes.length === 0 && data.people.length === 0;
  return (
    <div className="search-page" aria-busy={isPlaceholderData}>
      <h1 className="search-heading">
        Results for <q>{query}</q>
      </h1>
      {nothing && (
        <div className="search-empty">
          <p>Nothing matches. Try fewer words, or the start of a word.</p>
        </div>
      )}
      {data.titles.length > 0 && (
        <section className="search-section" aria-labelledby="search-titles">
          <h2 id="search-titles">
            Movies & shows <span className="search-count">{data.titles.length}</span>
          </h2>
          <ul className="search-grid">
            {data.titles.map((card) => (
              <li key={card.id}>
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
          <ul className="search-list">
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
          <ul className="search-people">
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
