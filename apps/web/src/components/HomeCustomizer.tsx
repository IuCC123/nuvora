import { useEffect, useRef, useState, type DragEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  DEFAULT_ORDER,
  FILTER_LABELS,
  LIBRARY_SORTS,
  ORDER_LABELS,
  queryKeys,
  SORT_LABELS,
  WATCH_FILTERS,
  type HomeBackground,
  type HomeLayout,
  type HomeRowConfig,
  type LibrarySummary,
  type PosterSize,
} from "@nuvora/core";
import { client, queries } from "../nuvora";
import { ArrowDownIcon, ArrowUpIcon, ClockIcon, CloseIcon, EyeIcon, EyeOffIcon, GripIcon, PencilIcon, PlusIcon, ResumeIcon, TrashIcon } from "./Icon";
import { LibraryIcon } from "./LibraryIcon";
import "../styles/customize.css";

export const BACKGROUNDS: { value: HomeBackground; label: string }[] = [
  { value: "none", label: "None" },
  { value: "ambient", label: "Posters" },
  { value: "dusk", label: "Dusk" },
  { value: "ocean", label: "Ocean" },
  { value: "forest", label: "Forest" },
  { value: "ember", label: "Ember" },
  { value: "aurora", label: "Aurora" },
  { value: "graphite", label: "Graphite" },
];

const SIZES: { value: PosterSize; label: string }[] = [
  { value: "small", label: "Small" },
  { value: "medium", label: "Medium" },
  { value: "large", label: "Large" },
];

/** Rows the user added themselves; built-in rows are hidden rather than removed. */
const isCustom = (row: HomeRowConfig) => row.id.startsWith("custom-");

/**
 * The home screen editor: background, poster size, and the rows with their
 * order, visibility, title, sort and filter. Every change saves to the
 * account at once, so the home behind the panel updates as you go.
 */
export function HomeCustomizer({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const saved = useQuery(queries.homeLayout());
  const { data: libraries = [] } = useQuery(queries.libraries());
  const [draft, setDraft] = useState<HomeLayout | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const layout = draft ?? saved.data ?? null;

  const save = useMutation({
    mutationFn: client.saveHomeLayout,
    onSuccess: (next) => {
      qc.setQueryData(queryKeys.homeLayout, next);
      void qc.invalidateQueries({ queryKey: queryKeys.home });
    },
  });
  const reset = useMutation({
    mutationFn: client.resetHomeLayout,
    onSuccess: async () => {
      setDraft(null);
      setEditing(null);
      await qc.invalidateQueries({ queryKey: queryKeys.homeLayout });
      await qc.invalidateQueries({ queryKey: queryKeys.home });
    },
  });

  // Typing in a row title fires many changes; send the last one.
  const pending = useRef<number | undefined>(undefined);
  const update = (next: HomeLayout) => {
    setDraft(next);
    window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => save.mutate(next), 350);
  };
  useEffect(() => () => window.clearTimeout(pending.current), []);
  useEffect(() => panel.current?.focus(), []);

  if (!layout) {
    return (
      <Shell onClose={onClose} panel={panel}>
        <div className="customize-loading" aria-busy="true" />
      </Shell>
    );
  }

  const rows = layout.rows;
  const setRows = (next: HomeRowConfig[]) => update({ ...layout, rows: next });
  const patchRow = (id: string, patch: Partial<HomeRowConfig>) => setRows(rows.map((r) => (r.id === id ? { ...r, ...patch } : r)));
  const move = (from: number, to: number) => {
    if (to < 0 || to >= rows.length || from === to) return;
    const next = [...rows];
    const [row] = next.splice(from, 1);
    next.splice(to, 0, row!);
    setRows(next);
  };
  const addRow = () => {
    const library = libraries[0];
    if (!library) return;
    const row: HomeRowConfig = {
      id: `custom-${Date.now().toString(36)}`,
      type: "library",
      hidden: false,
      library_id: library.id,
      sort: "added",
      order: "desc",
      filter: "unwatched",
    };
    setRows([...rows, row]);
    setEditing(row.id);
  };
  const onDrop = (e: DragEvent, target: number) => {
    e.preventDefault();
    const from = rows.findIndex((r) => r.id === dragging);
    if (from >= 0) move(from, target);
    setDragging(null);
  };

  const footer = (
    <div className="customize-footer">
      <button type="button" className="btn btn-quiet btn-small" onClick={() => reset.mutate()} disabled={reset.isPending}>
        Reset to default
      </button>
      {/* Changes save as they happen; only a failure needs saying. */}
      <span className="customize-status" role="status">
        {save.isError ? "Couldn't save. Your next change will try again." : ""}
      </span>
      <button type="button" className="btn btn-primary btn-small" onClick={onClose}>
        Done
      </button>
    </div>
  );

  return (
    <Shell onClose={onClose} panel={panel} footer={footer}>
      <section className="customize-section" aria-labelledby="customize-background">
        <h3 id="customize-background">Background</h3>
        <div className="swatches" role="radiogroup" aria-labelledby="customize-background">
          {BACKGROUNDS.map((b) => (
            <button
              key={b.value}
              type="button"
              role="radio"
              aria-checked={layout.background === b.value}
              className="swatch"
              data-bg={b.value}
              onClick={() => update({ ...layout, background: b.value })}
            >
              <span className="swatch-fill" aria-hidden />
              <span className="swatch-label">{b.label}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="customize-section" aria-labelledby="customize-size">
        <h3 id="customize-size">Poster size</h3>
        <div className="segmented" role="radiogroup" aria-labelledby="customize-size">
          {SIZES.map((s) => (
            <button key={s.value} type="button" role="radio" aria-checked={layout.poster_size === s.value} aria-pressed={layout.poster_size === s.value} onClick={() => update({ ...layout, poster_size: s.value })}>
              {s.label}
            </button>
          ))}
        </div>
      </section>

      <section className="customize-section" aria-labelledby="customize-rows">
        <div className="customize-rows-head">
          <h3 id="customize-rows">Rows</h3>
          <button type="button" className="btn btn-quiet btn-small" onClick={addRow} disabled={!libraries.length}>
            <PlusIcon size={16} />
            Add row
          </button>
        </div>
        <ol className="row-list" role="list">
          {rows.map((row, i) => {
            const library = libraries.find((l) => l.id === row.library_id);
            const name = rowName(row, library);
            const detail = rowDetail(row);
            return (
              <li
                key={row.id}
                className="row-config"
                data-hidden={row.hidden}
                data-dragging={dragging === row.id}
                draggable
                onDragStart={(e) => {
                  setDragging(row.id);
                  e.dataTransfer.effectAllowed = "move";
                }}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => onDrop(e, i)}
                onDragEnd={() => setDragging(null)}
              >
                <div className="row-config-line">
                  <span className="row-config-grip" aria-hidden>
                    <GripIcon size={16} />
                  </span>
                  <RowIcon row={row} library={library} />
                  <span className="row-config-text">
                    <span className="row-config-name">{name}</span>
                    {detail && <span className="row-config-detail">{detail}</span>}
                  </span>
                  <div className="row-config-actions">
                    <span className="row-config-move">
                      <button type="button" className="icon-btn" onClick={() => move(i, i - 1)} disabled={i === 0} aria-label={`Move ${name} up`}>
                        <ArrowUpIcon size={16} />
                      </button>
                      <button type="button" className="icon-btn" onClick={() => move(i, i + 1)} disabled={i === rows.length - 1} aria-label={`Move ${name} down`}>
                        <ArrowDownIcon size={16} />
                      </button>
                    </span>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-pressed={!row.hidden}
                      onClick={() => patchRow(row.id, { hidden: !row.hidden })}
                      aria-label={row.hidden ? `Show ${name}` : `Hide ${name}`}
                    >
                      {row.hidden ? <EyeOffIcon size={16} /> : <EyeIcon size={16} />}
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-expanded={editing === row.id}
                      onClick={() => setEditing(editing === row.id ? null : row.id)}
                      aria-label={`Edit ${name}`}
                    >
                      <PencilIcon size={16} />
                    </button>
                    {isCustom(row) && (
                      <button type="button" className="icon-btn icon-btn-danger" onClick={() => setRows(rows.filter((r) => r.id !== row.id))} aria-label={`Remove ${name}`}>
                        <TrashIcon size={16} />
                      </button>
                    )}
                  </div>
                </div>
                {editing === row.id && <RowEditor row={row} libraries={libraries} onChange={(patch) => patchRow(row.id, patch)} />}
              </li>
            );
          })}
        </ol>
      </section>
    </Shell>
  );
}

/** The panel: a fixed head, the scrolling body, and a footer pinned to the bottom. */
function Shell({
  onClose,
  panel,
  footer,
  children,
}: {
  onClose: () => void;
  panel: React.RefObject<HTMLDivElement | null>;
  footer?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="customize-layer" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="customize-scrim" onClick={onClose} aria-hidden />
      <div className="customize-panel" role="dialog" aria-modal="true" aria-labelledby="customize-title" tabIndex={-1} ref={panel}>
        <div className="customize-head">
          <h2 id="customize-title">Customize Home</h2>
          <button type="button" className="icon-btn" onClick={onClose} aria-label="Close">
            <CloseIcon size={18} />
          </button>
        </div>
        <div className="customize-body">{children}</div>
        {footer}
      </div>
    </div>
  );
}

function RowEditor({ row, libraries, onChange }: { row: HomeRowConfig; libraries: LibrarySummary[]; onChange: (patch: Partial<HomeRowConfig>) => void }) {
  const library = libraries.find((l) => l.id === row.library_id);
  const sort = row.sort ?? "title";
  return (
    <div className="row-editor">
      <label className="mini-field">
        <span>Title</span>
        <input
          type="text"
          value={row.title ?? ""}
          placeholder={rowName({ ...row, title: null }, library)}
          maxLength={64}
          onChange={(e) => onChange({ title: e.target.value.trim() ? e.target.value : null })}
        />
      </label>
      {row.type === "library" && (
        <>
          <label className="mini-field">
            <span>Library</span>
            <select value={row.library_id ?? ""} onChange={(e) => onChange({ library_id: Number(e.target.value) })}>
              {libraries.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>
          </label>
          <div className="mini-pair">
            <label className="mini-field">
              <span>Sort by</span>
              <select value={sort} onChange={(e) => onChange({ sort: e.target.value as typeof sort, order: DEFAULT_ORDER[e.target.value as typeof sort] })}>
                {LIBRARY_SORTS.map((s) => (
                  <option key={s} value={s}>
                    {SORT_LABELS[s]}
                  </option>
                ))}
              </select>
            </label>
            <label className="mini-field">
              <span>Order</span>
              <select value={row.order ?? DEFAULT_ORDER[sort]} onChange={(e) => onChange({ order: e.target.value as "asc" | "desc" })}>
                <option value="asc">{ORDER_LABELS[sort].asc}</option>
                <option value="desc">{ORDER_LABELS[sort].desc}</option>
              </select>
            </label>
          </div>
          <label className="mini-field">
            <span>Show</span>
            <select value={row.filter ?? "all"} onChange={(e) => onChange({ filter: e.target.value as (typeof WATCH_FILTERS)[number] })}>
              {WATCH_FILTERS.map((f) => (
                <option key={f} value={f}>
                  {FILTER_LABELS[f]}
                </option>
              ))}
            </select>
          </label>
        </>
      )}
    </div>
  );
}

function rowName(row: HomeRowConfig, library: LibrarySummary | undefined): string {
  if (row.title) return row.title;
  if (row.type === "continue_watching") return "Continue Watching";
  if (row.type === "recently_added") return "Recently Added";
  return library?.name ?? "Library";
}

/**
 * How a library row is sorted and filtered, when it isn't the library's own
 * title order: "Date added · Unwatched". The direction is named only when it
 * isn't the sort's usual one.
 */
function rowDetail(row: HomeRowConfig): string | null {
  if (row.type !== "library") return null;
  const sort = row.sort ?? "title";
  const order = row.order ?? DEFAULT_ORDER[sort];
  const filtered = row.filter && row.filter !== "all";
  if (sort === "title" && order === DEFAULT_ORDER.title && !filtered) return null;
  const parts = [order === DEFAULT_ORDER[sort] ? SORT_LABELS[sort] : `${SORT_LABELS[sort]}, ${ORDER_LABELS[sort][order].toLowerCase()}`];
  if (filtered) parts.push(FILTER_LABELS[row.filter!]);
  return parts.join(" · ");
}

function RowIcon({ row, library }: { row: HomeRowConfig; library: LibrarySummary | undefined }) {
  if (row.type === "continue_watching") return <ResumeIcon size={18} className="row-config-icon" />;
  if (row.type === "recently_added") return <ClockIcon size={18} className="row-config-icon" />;
  return <LibraryIcon icon={library?.icon ?? null} kind={library?.kind ?? "movies"} size={18} className="row-config-icon" />;
}
