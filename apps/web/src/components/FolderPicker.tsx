import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { describeError, folderCrumbs } from "@nuvora/core";
import { queries } from "../nuvora";
import { ChevronRightIcon, FolderIcon, ParentFolderIcon } from "./Icon";
import "../styles/picker.css";

/**
 * Browse the server's folders and pick one. Inline, under the field it fills,
 * rather than a dialog: it is part of the form, and the form stays visible.
 * Up and Down move between folders, Right or Enter opens one, Left goes up.
 * Escape cancels the picker alone, never the form around it.
 */
export function FolderPicker({
  start,
  onChoose,
  onCancel,
}: {
  /** The folder to open at; `null` opens the filesystem roots. */
  start: string | null;
  onChoose: (path: string) => void;
  onCancel: () => void;
}) {
  const [path, setPath] = useState(start);
  const listing = useQuery({ ...queries.adminFs(path), placeholderData: keepPreviousData });
  const data = listing.data;
  const settled = data && !listing.isPlaceholderData ? data : undefined;
  const pickerRoot = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  /** After a move, which row takes focus once the new folder arrives: the folder we came up from, or the first. */
  const focusNext = useRef<string | "first" | null>(null);

  useEffect(() => {
    if (!settled || focusNext.current === null) return;
    const rows = [...(list.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const target = rows.find((r) => r.dataset.path === focusNext.current) ?? rows[0];
    focusNext.current = null;
    (target ?? pickerRoot.current?.querySelector<HTMLElement>(".picker-up"))?.focus();
  }, [settled]);

  const open = (next: string | null, focus: string | "first" = "first") => {
    focusNext.current = focus;
    setPath(next);
  };
  const up = () => {
    if (!data || (data.parent == null && path === null)) return;
    open(data.parent ?? null, data.path);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLUListElement>) => {
    const rows = [...e.currentTarget.querySelectorAll<HTMLButtonElement>("button")];
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const move = (i: number) => rows[Math.max(0, Math.min(rows.length - 1, i))]?.focus();
    const handlers: Record<string, () => void> = {
      ArrowDown: () => move(at + 1),
      ArrowUp: () => move(at - 1),
      Home: () => move(0),
      End: () => move(rows.length - 1),
      ArrowRight: () => rows[at]?.click(),
      ArrowLeft: up,
      Backspace: up,
    };
    const handler = handlers[e.key];
    if (!handler) return;
    e.preventDefault();
    handler();
  };

  const crumbs = folderCrumbs(data?.path ?? "");
  const canGoUp = data != null && !(data.parent == null && path === null);

  return (
    <div
      className="picker"
      role="group"
      aria-label="Choose a folder"
      ref={pickerRoot}
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        e.stopPropagation();
        onCancel();
      }}
    >
      <div className="picker-bar">
        <button type="button" className="icon-btn picker-up" onClick={up} disabled={!canGoUp} aria-label="Up one folder" title="Up one folder">
          <ParentFolderIcon />
        </button>
        <nav aria-label="Folder path" className="picker-path">
          <ol role="list">
            {crumbs.length === 0 && <li className="picker-crumb-here">This computer</li>}
            {crumbs.map((c, i) =>
              i === crumbs.length - 1 ? (
                <li key={c.path} className="picker-crumb-here" aria-current="location">
                  {c.name}
                </li>
              ) : (
                <li key={c.path}>
                  <button type="button" onClick={() => open(c.path)}>
                    {c.name}
                  </button>
                </li>
              ),
            )}
          </ol>
        </nav>
      </div>

      <div className="picker-body" aria-busy={listing.isFetching}>
        {listing.isError && !data ? (
          <div className="picker-message" role="alert">
            <p>{describeError(listing.error)}</p>
            <button type="button" className="btn btn-secondary btn-small" onClick={() => void listing.refetch()}>
              Try again
            </button>
          </div>
        ) : !data ? (
          <ul role="list" className="picker-list" aria-label="Loading folders">
            {Array.from({ length: 5 }, (_, i) => (
              <li key={i} className="picker-skeleton" />
            ))}
          </ul>
        ) : data.dirs.length === 0 ? (
          <p className="picker-message">No folders inside {crumbs.at(-1)?.name ?? "this folder"}.</p>
        ) : (
          <ul role="list" className="picker-list" ref={list} onKeyDown={onKeyDown} data-stale={listing.isPlaceholderData}>
            {data.dirs.map((dir) => (
              <li key={dir.path}>
                <button type="button" className="picker-row" data-path={dir.path} onClick={() => open(dir.path)}>
                  <FolderIcon size={18} />
                  <span className="picker-name">{dir.name}</span>
                  <ChevronRightIcon size={16} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="picker-foot">
        <p className="picker-chosen">
          {data?.path ? (
            <>
              <span className="sr-only">Current folder: </span>
              {data.path}
            </>
          ) : (
            "Open a drive to choose a folder in it."
          )}
        </p>
        <button type="button" className="btn btn-quiet btn-small" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="btn btn-primary btn-small" disabled={!settled?.path} onClick={() => settled && onChoose(settled.path)}>
          Use this folder
        </button>
      </div>
    </div>
  );
}
