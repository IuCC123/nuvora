import { useEffect, useId, useRef, useState } from "react";
import { folderCrumbs } from "@nuvora/core";
import { FolderPicker } from "./FolderPicker";
import { AlertIcon, FolderIcon, PlusIcon } from "./Icon";

/**
 * A library's folders: the chosen ones, each removable, and the picker to add
 * another. The add button carries `name`, so a form can focus it when the list
 * is invalid.
 */
export function FolderList({
  label,
  paths,
  onChange,
  missing = [],
  error,
  name = "paths",
  openPicker = false,
}: {
  label: string;
  paths: readonly string[];
  onChange: (paths: string[]) => void;
  /** Folders the server can't read right now. */
  missing?: readonly string[];
  error?: string | null | undefined;
  name?: string;
  /** Start with the picker open when no folder is chosen yet. */
  openPicker?: boolean;
}) {
  // A picker open from the start leaves focus where the form put it; one opened with the button takes it.
  const [picking, setPicking] = useState<{ takeFocus: boolean } | null>(openPicker && paths.length === 0 ? { takeFocus: false } : null);
  const addButton = useRef<HTMLButtonElement>(null);
  const wasPicking = useRef(!!picking);
  // Closing the picker removes the focused control with it; the add button takes focus back.
  useEffect(() => {
    if (wasPicking.current && !picking) addButton.current?.focus();
    wasPicking.current = !!picking;
  }, [picking]);
  const labelId = useId();
  const errorId = useId();
  const last = paths.at(-1);
  const start = last ? (folderCrumbs(last).at(-2)?.path ?? null) : null;

  return (
    <div className="field" role="group" aria-labelledby={labelId} aria-describedby={error ? errorId : undefined}>
      <span className="field-label" id={labelId}>
        {label}
      </span>
      {paths.length > 0 && (
        <ul role="list" className="folders">
          {paths.map((path) => (
            <li key={path} className="folder">
              <FolderIcon size={18} />
              <span className="folder-path" title={path}>
                {path}
              </span>
              {missing.includes(path) && (
                <span className="folder-missing">
                  <AlertIcon size={14} />
                  Can't read
                </span>
              )}
              <button type="button" className="btn btn-quiet btn-small" onClick={() => onChange(paths.filter((p) => p !== path))} aria-label={`Remove ${path}`}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      {picking ? (
        <FolderPicker
          start={start}
          takeFocus={picking.takeFocus}
          onChoose={(path) => {
            if (!paths.includes(path)) onChange([...paths, path]);
            setPicking(null);
          }}
          onCancel={() => setPicking(null)}
        />
      ) : (
        <button ref={addButton} type="button" name={name} className="btn btn-secondary btn-small folder-add" onClick={() => setPicking({ takeFocus: true })}>
          <PlusIcon size={16} />
          {paths.length ? "Add another folder" : "Choose a folder"}
        </button>
      )}
      {error && (
        <p className="field-error" id={errorId}>
          <AlertIcon size={14} />
          {error}
        </p>
      )}
    </div>
  );
}
