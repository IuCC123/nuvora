import { useQuery } from "@tanstack/react-query";
import type { HomeBackground, HomeLayout, HomeRow } from "@nuvora/core";
import { queries } from "../nuvora";
import { Artwork } from "../components/Artwork";
import { PosterRow } from "../components/PosterRow";
import { StreamCardView } from "../components/StreamCard";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useUser } from "../lib/session";
import { useStartScan } from "../lib/useStartScan";
import "../styles/home.css";

/**
 * Home: server-ordered rows of posters. Renders every row kind the same way, so new kinds need no client change.
 * An admin's torrent streams stopped partway lead Continue Watching, which shows for them alone too.
 */
export function Home() {
  useDocumentTitle(undefined);
  const { data } = useQuery(queries.home());
  const admin = useUser()?.role === "admin";
  const { data: streams = [] } = useQuery({ ...queries.watchedStreams(), enabled: admin });
  // The server leaves out an empty Continue Watching; with streams to offer, it goes back where the layout has it.
  const { data: layout } = useQuery({ ...queries.homeLayout(), enabled: streams.length > 0 });
  if (!data) return null; // the route loader awaited it; errors render the route's errorComponent
  const streamCards = streams.map((stream) => ({ key: `stream:${stream.link}`, card: <StreamCardView stream={stream} /> }));
  const lead = (row: HomeRow) => (row.kind === "continue_watching" ? streamCards : []);
  const rows = withContinueRow(data.rows, layout, streamCards.length > 0).filter((row) => row.items.length + lead(row).length > 0);
  return (
    <div className="home" data-poster-size={data.poster_size}>
      <HomeBackdrop background={data.background} rows={rows} />
      <h1 className="sr-only">Home</h1>
      {rows.length ? rows.map((row) => <PosterRow key={row.id ?? `${row.kind}-${row.library_id ?? ""}`} row={row} lead={lead(row)} />) : <EmptyLibrary />}
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
  const row: HomeRow = { kind: "continue_watching", id: config.id, title: config.title ?? "Continue Watching", items: [], total: 0 };
  return [...rows.slice(0, index), row, ...rows.slice(index)];
}

/** The chosen background behind the rows. "Posters" blurs the first poster on screen into a wash. */
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
