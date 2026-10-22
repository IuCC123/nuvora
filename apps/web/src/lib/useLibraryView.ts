import { useCallback, useEffect } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { DEFAULT_VIEW, hasViewParams, librarySearch, libraryViewStorageKey, parseLibraryView, sameView, type LibraryView } from "@nuvora/core";

function stored(libraryId: number): LibraryView | null {
  try {
    const raw = localStorage.getItem(libraryViewStorageKey(libraryId));
    return raw ? parseLibraryView(JSON.parse(raw) as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

/**
 * A library page's view, kept in the URL (so Back, reload and a shared link
 * keep it) and on this device per library (so a plain `/library/3` opens the
 * way it was left). Params in the URL win; without any, the stored view is
 * put into the URL.
 */
export function useLibraryView(libraryId: number): [LibraryView, (next: LibraryView) => void] {
  const raw = useSearch({ from: "/shell/library/$id" });
  const navigate = useNavigate();
  const fromUrl = hasViewParams(raw);
  const remembered = fromUrl ? null : stored(libraryId);
  const view = fromUrl ? parseLibraryView(raw) : (remembered ?? DEFAULT_VIEW);

  useEffect(() => {
    if (remembered && !sameView(remembered, DEFAULT_VIEW)) {
      void navigate({ to: "/library/$id", params: { id: libraryId }, search: librarySearch(remembered), replace: true, resetScroll: false });
    }
  }, [libraryId, fromUrl]);

  useEffect(() => {
    localStorage.setItem(libraryViewStorageKey(libraryId), JSON.stringify(view));
  }, [libraryId, view.sort, view.order, view.filter, view.res, view.hdr, view.group]);

  const setView = useCallback(
    (next: LibraryView) => {
      // Stored first: a return to the default view leaves no params, and the next render reads storage.
      localStorage.setItem(libraryViewStorageKey(libraryId), JSON.stringify(next));
      void navigate({ to: "/library/$id", params: { id: libraryId }, search: librarySearch(next), replace: true, resetScroll: false });
    },
    [navigate, libraryId],
  );
  return [view, setView];
}
