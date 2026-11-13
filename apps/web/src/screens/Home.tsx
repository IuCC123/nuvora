import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import type { HomeBackground, HomeLayout, HomeRow } from "@nuvora/core";
import { queries } from "../nuvora";
import { Artwork } from "../components/Artwork";
import { Hero } from "../components/home/Hero";
import { ContinueRow, GenreRow, TopRow } from "../components/home/HomeRows";
import { PosterRow } from "../components/PosterRow";
import { StreamCardView } from "../components/StreamCard";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useSpatialNav } from "../lib/spatialNav";
import { useUser } from "../lib/session";
import { useStartScan } from "../lib/useStartScan";
import "../styles/home.css";

/**
 * Home: the featured hero, then the server's rows in the user's order. Each
 * row kind has its own shape (wide Continue Watching, ranked Top 10, genre
 * tiles); any other kind is a poster row, so new kinds need no client change.
 * An admin's torrent streams stopped partway lead Continue Watching.
 */
export function Home() {
  useDocumentTitle(undefined);
  const root = useRef<HTMLDivElement>(null);
  useSpatialNav(root);
  const { data } = useQuery(queries.home());
  const admin = useUser()?.role === "admin";
  const { data: streams = [] } = useQuery({ ...queries.watchedStreams(), enabled: admin });
  // The server leaves out an empty Continue Watching; with streams to offer, it goes back where the layout has it.
  const { data: layout } = useQuery({ ...queries.homeLayout(), enabled: streams.length > 0 });
  if (!data) return <HomeSkeleton />; // the route loader awaited it; errors render the route's errorComponent
  const streamCards = streams.map((stream) => ({ key: `stream:${stream.link}`, card: <StreamCardView stream={stream} wide /> }));
  const lead = (row: HomeRow) => (row.kind === "continue_watching" ? streamCards : []);
  const rows = withContinueRow(data.rows, layout, streamCards.length > 0).filter((row) => row.items.length + row.genres.length + lead(row).length > 0);
  const hero = data.featured.length > 0;
  return (
    <div ref={root} className="home" data-poster-size={data.poster_size} data-hero={hero}>
      <h1 className="sr-only">Home</h1>
      {hero ? <Hero items={data.featured} /> : <HomeBackdrop background={data.background} rows={rows} />}
      {hero && <div className="home-ambient" aria-hidden />}
      <div className="home-rows">
        {rows.length ? rows.map((row) => <HomeRowView key={row.id ?? `${row.kind}-${row.library_id ?? ""}`} row={row} lead={lead(row)} />) : <EmptyLibrary />}
      </div>
    </div>
  );
}

function HomeRowView({ row, lead }: { row: HomeRow; lead: { key: string; card: React.ReactNode }[] }) {
  switch (row.kind) {
    case "continue_watching":
      return <ContinueRow row={row} lead={lead} />;
    case "top":
      return <TopRow row={row} />;
    case "genres":
      return <GenreRow row={row} />;
    default:
      return <PosterRow row={row} />;
  }
}

/** While home loads: the hero's shape and two rows of shimmering cards, so nothing jumps when it arrives. */
export function HomeSkeleton() {
  return (
    <div className="home" aria-busy="true" aria-label="Loading Home">
      <div className="hero hero-skeleton">
        <div className="hero-info">
          <span className="skeleton skeleton-text" style={{ width: 140 }} />
          <span className="skeleton" style={{ width: "min(420px, 80%)", height: 64, borderRadius: 14 }} />
          <span className="skeleton skeleton-text" style={{ width: "min(520px, 90%)" }} />
          <span className="skeleton" style={{ width: 300, height: 48, borderRadius: 999 }} />
        </div>
      </div>
      <div className="home-rows">
        {(["landscape", "poster"] as const).map((variant) => (
          <section key={variant} className="row" data-variant={variant}>
            <div className="row-head">
              <span className="skeleton skeleton-text" style={{ width: 180, height: 18 }} />
            </div>
            <div className="row-body">
              <ul className="row-strip" role="list">
                {Array.from({ length: 8 }, (_, i) => (
                  <li key={i} className="row-item">
                    <span className={`${variant === "poster" ? "poster-frame" : "wide-frame"} skeleton`} />
                  </li>
                ))}
              </ul>
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

/** `rows` with an empty Continue Watching put back in its layout position, when `wanted` and the layout shows one. */
function withContinueRow(rows: HomeRow[], layout: HomeLayout | undefined, wanted: boolean): HomeRow[] {
  if (!wanted || !layout || rows.some((row) => row.kind === "continue_watching")) return rows;
  const visible = layout.rows.filter((row) => !row.hidden);
  const at = visible.findIndex((row) => row.type === "continue_watching");
  const config = visible[at];
  if (!config) return rows;
  const earlier = new Set(visible.slice(0, at).map((row) => row.id));
  const index = rows.filter((row) => row.id != null && earlier.has(row.id)).length;
  const row: HomeRow = { kind: "continue_watching", id: config.id, title: config.title ?? "Continue Watching", items: [], total: 0, genres: [] };
  return [...rows.slice(0, index), row, ...rows.slice(index)];
}

/** Without a hero: the chosen background behind the rows. "Posters" blurs the first poster on screen into a wash. */
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
      <button type="button" className="btn btn-primary" onClick={() => scan.mutate()} disabled={scan.isPending}>
        Scan library
      </button>
    </>
  );
}
