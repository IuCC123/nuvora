import { useQuery } from "@tanstack/react-query";
import { queries } from "../nuvora";
import { PosterRow } from "../components/PosterRow";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useUser } from "../lib/session";
import { useStartScan } from "../lib/useStartScan";
import "../styles/home.css";

/** Home: server-ordered rows of posters. Renders every row kind the same way, so new kinds need no client change. */
export function Home() {
  useDocumentTitle(undefined);
  const { data } = useQuery(queries.home());
  if (!data) return null; // the route loader awaited it; errors render the route's errorComponent
  const rows = data.rows.filter((row) => row.items.length > 0);
  return (
    <div className="home">
      <h1 className="sr-only">Home</h1>
      {rows.length ? rows.map((row) => <PosterRow key={row.kind} row={row} />) : <EmptyLibrary />}
    </div>
  );
}

/** Only admins can scan, so a member is told who can fill the library instead of being offered a button that would fail. */
function EmptyLibrary() {
  const admin = useUser()?.role === "admin";
  return (
    <div className="empty">
      <h2>No movies yet</h2>
      {admin ? <ScanPrompt /> : <p>Movies show up here once an admin adds them to this server.</p>}
    </div>
  );
}

function ScanPrompt() {
  const scan = useStartScan();
  return (
    <>
      <p>Add movie files to a library folder, then scan to bring them in.</p>
      <button type="button" className="btn btn-secondary" onClick={() => scan.mutate()} disabled={scan.isPending}>
        Scan library
      </button>
    </>
  );
}
