import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { HomeBackground, HomeRow } from "@nuvora/core";
import { queries } from "../nuvora";
import { Artwork } from "../components/Artwork";
import { HomeCustomizer } from "../components/HomeCustomizer";
import { SlidersIcon } from "../components/Icon";
import { PosterRow } from "../components/PosterRow";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useUser } from "../lib/session";
import { useStartScan } from "../lib/useStartScan";
import "../styles/home.css";

/** Home: server-ordered rows of posters. Renders every row kind the same way, so new kinds need no client change. */
export function Home() {
  useDocumentTitle(undefined);
  const { data } = useQuery(queries.home());
  const [customizing, setCustomizing] = useState(false);
  if (!data) return null; // the route loader awaited it; errors render the route's errorComponent
  const rows = data.rows.filter((row) => row.items.length > 0);
  return (
    <div className="home" data-poster-size={data.poster_size}>
      <HomeBackdrop background={data.background} rows={rows} />
      <h1 className="sr-only">Home</h1>
      <div className="home-toolbar">
        <button type="button" className="btn btn-quiet btn-small" onClick={() => setCustomizing(true)}>
          <SlidersIcon size={16} />
          Customize
        </button>
      </div>
      {rows.length ? rows.map((row) => <PosterRow key={row.id ?? `${row.kind}-${row.library_id ?? ""}`} row={row} />) : <EmptyLibrary />}
      {customizing && <HomeCustomizer onClose={() => setCustomizing(false)} />}
    </div>
  );
}

/** The chosen background behind the rows. "From what you watch" blurs the first poster on screen into a wash. */
function HomeBackdrop({ background, rows }: { background: HomeBackground; rows: HomeRow[] }) {
  if (background === "none") return null;
  const art = background === "ambient" ? rows.flatMap((r) => r.items).find((c) => c.poster)?.poster : undefined;
  return (
    <div className="home-bg" data-bg={background} aria-hidden>
      {art && <Artwork image={art} size="small" alt="" fill />}
    </div>
  );
}

/** Only admins can scan, so a member is told who can fill the library instead of being offered a button that would fail. */
function EmptyLibrary() {
  const admin = useUser()?.role === "admin";
  return (
    <div className="empty">
      <h2>Nothing to watch yet</h2>
      {admin ? <ScanPrompt /> : <p>Movies and shows appear here once an admin adds them to this server.</p>}
    </div>
  );
}

function ScanPrompt() {
  const scan = useStartScan();
  return (
    <>
      <p>Add movie or episode files to a library folder, then scan to bring them in.</p>
      <button type="button" className="btn btn-secondary" onClick={() => scan.mutate()} disabled={scan.isPending}>
        Scan library
      </button>
    </>
  );
}
