import { useEffect, useRef } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { queries } from "../nuvora";
import { PosterCardView } from "../components/PosterCard";
import { ambientColor } from "../lib/ambient";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import "../styles/genre.css";

/** One genre's movies and shows, newest first, in a grid that loads more as it nears the end. */
export function GenreScreen() {
  const { key } = useParams({ from: "/shell/genre/$key" });
  const pages = useInfiniteQuery(queries.genrePages(key));
  const first = pages.data?.pages[0];
  useDocumentTitle(first?.name);
  const cards = pages.data?.pages.flatMap((p) => p.items) ?? [];
  const glow = ambientColor(cards.find((c) => c.backdrop)?.backdrop);
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = end.current;
    if (!el || !pages.hasNextPage) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting) && !pages.isFetchingNextPage) void pages.fetchNextPage();
    }, { rootMargin: "600px 0px" });
    observer.observe(el);
    return () => observer.disconnect();
  }, [pages]);

  return (
    <div className="genre-page" style={glow ? ({ "--ambient": glow } as React.CSSProperties) : undefined}>
      <header className="genre-head">
        {first ? (
          <>
            <h1>{first.name}</h1>
            <p>{first.total === 1 ? "1 title" : `${first.total.toLocaleString()} titles`}</p>
          </>
        ) : (
          <>
            <span className="skeleton" style={{ width: 280, height: 48, borderRadius: 12 }} />
            <span className="skeleton skeleton-text" style={{ width: 90 }} />
          </>
        )}
      </header>
      <ul className="genre-grid" role="list">
        {cards.map((card) => (
          <li key={card.id}>
            <PosterCardView card={card} />
          </li>
        ))}
        {(pages.isPending || pages.isFetchingNextPage) &&
          Array.from({ length: 12 }, (_, i) => (
            <li key={`pending-${i}`} aria-hidden>
              <span className="poster-frame skeleton" />
            </li>
          ))}
      </ul>
      <div ref={end} />
    </div>
  );
}
