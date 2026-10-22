import { useQuery } from "@tanstack/react-query";
import { queries } from "../nuvora";
import { PosterRow } from "../components/PosterRow";
import { ScanBanner } from "../components/ScanBanner";

/** Home: server-ordered rows of posters. Renders every row kind the same way, so new kinds need no client change. */
export function Home() {
  const { data } = useQuery(queries.home());
  if (!data) return null; // the route loader awaited it; only reachable on hard error
  return (
    <main aria-label="Home">
      <ScanBanner />
      {data.rows.map((row) => (
        <PosterRow key={row.kind} row={row} />
      ))}
      {/* TODO: empty library state: "Add a movie folder" with the configured paths and a Scan button. */}
    </main>
  );
}
