import type { HomeRow } from "@nuvora/core";
import { PosterCardView } from "./PosterCard";

/**
 * A titled, horizontally scrolling row. Keyboard: every card is a link in tab
 * order; Left/Right move focus within the row (roving focus, Step 4 adds full
 * spatial navigation for TV-style remotes). Off-screen cards are rendered with
 * `content-visibility: auto`, so a 2,000-card "All Movies" row lays out lazily.
 */
export function PosterRow({ row }: { row: HomeRow }) {
  const headingId = `row-${row.kind}`;
  return (
    <section aria-labelledby={headingId}>
      <h2 id={headingId}>{row.title}</h2>
      <ul role="list">
        {row.items.map((card) => (
          <li key={card.id}>
            <PosterCardView card={card} />
          </li>
        ))}
      </ul>
    </section>
  );
}
