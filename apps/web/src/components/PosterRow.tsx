import { useCallback, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { homeRowQueryParams, librarySearch, parseLibraryView, rowCards, type HomeRow, type RowPage } from "@nuvora/core";
import { queries } from "../nuvora";
import { ChevronRightIcon } from "./Icon";
import { PosterCardView } from "./PosterCard";
import { Row } from "./Row";

/** Cards still to load are drawn as shimmering posters, so the strip never ends early or shows a spinner. */
const MAX_PENDING_CARDS = 6;

/**
 * A row of posters. Its first page comes with home; the rest loads through
 * the row cursor (or a custom row's library query) as the strip nears its
 * end. `lead` cards come first (Continue Watching's torrent streams).
 */
export function PosterRow({ row, lead = [] }: { row: HomeRow; lead?: readonly { key: string; card: ReactNode }[] }) {
  const library = row.kind === "library" ? (row.library_id ?? undefined) : undefined;
  const id = `row-${row.id ?? (library === undefined ? row.kind : `library-${library}`)}`;
  const strip = useRef<HTMLUListElement>(null);
  const [armed, setArmed] = useState(false);
  const { data: libraries } = useQuery(queries.libraries());
  const named = libraries?.find((l) => l.id === library);
  // A custom row's later pages come from the library page with its query; built-in rows page through the row cursor.
  const params = row.query && library !== undefined ? homeRowQueryParams(row.query) : null;
  const rowPages = useInfiniteQuery({ ...queries.rowPages(row.kind, row.next ?? "", library), enabled: armed && row.next != null && !params });
  const queryPages = useInfiniteQuery({ ...queries.queryRowPages(library ?? 0, params ?? {}, row.next ?? ""), enabled: armed && row.next != null && !!params });
  const pages = params ? queryPages : rowPages;
  const seeAll = row.query ? librarySearch({ ...parseLibraryView({ ...row.query, res: row.query.resolution }), group: true }) : {};
  const { cards, hasMore } = rowCards(row, pages.data?.pages as RowPage[] | undefined);
  const failed = pages.isError && !pages.isFetching;
  const pending = hasMore && !failed ? Math.min(MAX_PENDING_CARDS, Math.max(1, row.total - cards.length)) : 0;

  const loadMore = useCallback(() => {
    if (!armed) setArmed(true);
    else if (pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }, [armed, pages]);
  // A failed page waits for Try again; re-arming on the error would refetch in a loop.
  const sentinel = useNearEnd(strip, hasMore && !pages.isFetching && !pages.isError, loadMore);
  const retry = () => void (pages.data ? pages.fetchNextPage() : pages.refetch());

  return (
    <Row
      id={id}
      variant="poster"
      stripRef={strip}
      itemCount={lead.length + cards.length + pending}
      title={row.title}
      action={
        library !== undefined && named ? (
          <Link to="/library/$id" params={{ id: library }} search={seeAll} className="row-see-all" aria-label={`See all of ${row.title}`}>
            See all <span className="row-count">{row.total.toLocaleString()}</span>
            <ChevronRightIcon size={16} />
          </Link>
        ) : undefined
      }
    >
      {lead.map(({ key, card }) => (
        <li key={key} className="row-item">
          {card}
        </li>
      ))}
      {cards.map((card) => (
        <li key={card.id} className="row-item">
          <PosterCardView card={card} />
        </li>
      ))}
      {failed && (
        <li className="row-item row-failed">
          <div className="poster-frame">
            <p>Couldn't load more.</p>
            <button type="button" className="btn btn-secondary btn-small" onClick={retry}>
              Try again
            </button>
          </div>
        </li>
      )}
      {Array.from({ length: pending }, (_, i) => (
        <li key={`pending-${i}`} className="row-item row-pending" aria-hidden ref={i === 0 ? sentinel : undefined}>
          <span className="poster-frame skeleton" />
        </li>
      ))}
    </Row>
  );
}

/** Calls `onNear` when the element it is attached to comes within a screen of the strip's visible edge. */
function useNearEnd(strip: RefObject<HTMLElement | null>, active: boolean, onNear: () => void) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  const callback = useRef(onNear);
  callback.current = onNear;
  useEffect(() => {
    if (!active || !target || !strip.current) return;
    const observer = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && callback.current(), {
      root: strip.current,
      rootMargin: "0px 100% 0px 0px",
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [active, target, strip]);
  return setTarget;
}
