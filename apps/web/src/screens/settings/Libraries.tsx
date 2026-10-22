import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { checkRequired, describeError, isScanRunning, plural, queryKeys, type AdminLibrary, type LibraryKind } from "@nuvora/core";
import { client, queries } from "../../nuvora";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { FormError, TextField } from "../../components/Field";
import { FolderList } from "../../components/FolderList";
import { AlertIcon, PlusIcon, RefreshIcon } from "../../components/Icon";
import { LIBRARY_TYPES, LibraryTypeIcon, TypePicker, TypeSuggestion } from "../../components/LibraryType";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { LoadError, SettingsPage, SkeletonRows } from "./SettingsPage";


/** Which library is open for editing: none, a new one, or one by id. */
type Editing = null | "new" | number;

export function SettingsLibraries() {
  const libraries = useQuery(queries.adminLibraries());
  const [editing, setEditing] = useState<Editing>(null);
  const [removing, setRemoving] = useState<AdminLibrary | null>(null);

  return (
    <SettingsPage
      title="Libraries"
      lede="The folders Nuvora scans for movies and shows. Everyone who can sign in sees every library."
      action={
        editing !== "new" &&
        libraries.data?.length !== 0 && (
          <button type="button" className="btn btn-primary btn-small" onClick={() => setEditing("new")}>
            <PlusIcon size={16} />
            Add library
          </button>
        )
      }
    >
      {editing === "new" && <LibraryEditor library={null} onDone={() => setEditing(null)} />}
      {libraries.isPending ? (
        <SkeletonRows count={2} label="Loading libraries" />
      ) : libraries.isError ? (
        <LoadError error={libraries.error} retry={() => void libraries.refetch()} />
      ) : libraries.data.length === 0 ? (
        editing !== "new" && (
          <div className="settings-empty">
            <h2>No libraries yet</h2>
            <p>Add a folder of movies or TV shows, and Nuvora brings them in.</p>
            <button type="button" className="btn btn-primary" onClick={() => setEditing("new")}>
              <PlusIcon size={16} />
              Add library
            </button>
          </div>
        )
      ) : (
        <ul role="list" className="settings-list">
          {libraries.data.map((library) =>
            editing === library.id ? (
              <li key={library.id} className="settings-row-editing">
                <LibraryEditor library={library} onDone={() => setEditing(null)} />
              </li>
            ) : (
              <LibraryRow key={library.id} library={library} onEdit={() => setEditing(library.id)} onRemove={() => setRemoving(library)} />
            ),
          )}
        </ul>
      )}
      {removing && <RemoveLibrary library={removing} onClose={() => setRemoving(null)} />}
    </SettingsPage>
  );
}

function LibraryRow({ library, onEdit, onRemove }: { library: AdminLibrary; onEdit: () => void; onRemove: () => void }) {
  const qc = useQueryClient();
  const scanning = isScanRunning(useQuery(queries.scanStatus()).data);
  const scan = useMutation({
    mutationFn: () => client.admin.scanLibrary(library.id),
    onSuccess: (status) => qc.setQueryData(queryKeys.scan, status),
  });
  return (
    <li className="settings-row library-row">
      <LibraryTypeIcon kind={library.kind} />
      <div className="library-main">
        <p className="library-name">
          {library.name}
          <span className="library-count">{itemCount(library)}</span>
        </p>
        <ul role="list" className="library-paths">
          {library.paths.map((path) => (
            <li key={path} data-missing={library.missing_paths.includes(path)}>
              {library.missing_paths.includes(path) && <AlertIcon size={14} aria-label="Can't read" />}
              {path}
            </li>
          ))}
        </ul>
        {library.missing_paths.length > 0 && (
          <p className="library-warning">
            Nuvora can't read {library.missing_paths.length === 1 ? "this folder" : "these folders"} right now. Check that the drive or
            share is connected. Its {LIBRARY_TYPES[library.kind].noun}s stay listed until it's back.
          </p>
        )}
        {scan.isError && <p className="library-warning">{describeError(scan.error)}</p>}
      </div>
      <div className="row-actions">
        <button
          type="button"
          className="btn btn-secondary btn-small"
          onClick={() => scan.mutate()}
          disabled={scanning || scan.isPending}
          aria-busy={scan.isPending}
        >
          <RefreshIcon size={15} />
          {scanning ? "Scanning…" : "Scan now"}
        </button>
        <button type="button" className="btn btn-quiet btn-small" onClick={onEdit} aria-label={`Edit ${library.name}`}>
          Edit
        </button>
        <button type="button" className="btn btn-quiet btn-small btn-quiet-danger" onClick={onRemove} aria-label={`Remove ${library.name}`}>
          Remove
        </button>
      </div>
    </li>
  );
}

function itemCount(library: Pick<AdminLibrary, "kind" | "item_count">): string {
  return plural(library.item_count, LIBRARY_TYPES[library.kind].noun);
}

interface LibraryChanges {
  name: string;
  paths: string[];
  kind: LibraryKind;
}

function LibraryEditor({ library, onDone }: { library: AdminLibrary | null; onDone: () => void }) {
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: ({ kind, ...v }: LibraryChanges) =>
      library ? client.admin.updateLibrary(library.id, { ...v, ...(kind !== library.kind && { kind }) }) : client.admin.createLibrary({ ...v, kind }),
    onSuccess: async () => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: queryKeys.adminLibraries }),
        qc.invalidateQueries({ queryKey: queryKeys.scan }),
        qc.invalidateQueries({ queryKey: queryKeys.libraries }),
        qc.invalidateQueries({ queryKey: queryKeys.library }),
        qc.invalidateQueries({ queryKey: queryKeys.home }),
      ]);
      onDone();
    },
  });
  const form = useForm(
    { name: library?.name ?? LIBRARY_TYPES.movies.label, paths: (library?.paths ?? []) as readonly string[], kind: library?.kind ?? ("movies" as LibraryKind) },
    (v) => ({ name: checkRequired(v.name, "a name for the library"), paths: v.paths.length ? null : "Choose at least one folder.", kind: null }),
    save.reset,
  );
  // A type change rebuilds the library, so it waits for a confirmation.
  const [confirming, setConfirming] = useState<LibraryChanges | null>(null);
  const failed = serverError(save.error, ["name", "paths", "kind"]);
  const title = library ? `Edit ${library.name}` : "Add a library";
  const chooseKind = (kind: LibraryKind) => {
    // A new library still named after its type follows the type.
    if (!library && form.values.name.trim() === LIBRARY_TYPES[form.values.kind].label) form.set("name", LIBRARY_TYPES[kind].label);
    form.set("kind", kind);
  };
  const submit = (v: LibraryChanges) => (library && v.kind !== library.kind ? setConfirming(v) : save.mutate(v));
  return (
    <form
      className="form editor"
      noValidate
      aria-label={title}
      onSubmit={form.submit((v) => submit({ name: v.name.trim(), paths: [...v.paths], kind: v.kind }))}
      onKeyDown={(e) => e.key === "Escape" && !save.isPending && !confirming && onDone()}
    >
      <h2 className="editor-title">{title}</h2>
      <TypePicker value={form.values.kind} onChange={chooseKind} />
      <TextField label="Name" placeholder={LIBRARY_TYPES[form.values.kind].label} autoFocus {...form.field("name")} error={form.error("name") ?? failed.field.name} />
      <FolderList
        label="Folders"
        paths={form.values.paths}
        missing={library?.missing_paths}
        onChange={(paths) => form.set("paths", paths)}
        error={form.error("paths") ?? failed.field.paths}
        openPicker={!library}
      />
      {!library && <TypeSuggestion paths={form.values.paths} kind={form.values.kind} onUse={chooseKind} />}
      {library && form.values.kind !== library.kind && (
        <p className="field-hint">Changing the type rebuilds this library's items and resets watch progress in it.</p>
      )}
      {library && form.values.kind === library.kind && form.values.paths.some((p) => !library.paths.includes(p)) && (
        <p className="field-hint">Saving scans the new folders.</p>
      )}
      <FormError message={failed.form} />
      <div className="form-actions">
        <button type="submit" className="btn btn-primary" disabled={save.isPending} aria-busy={save.isPending}>
          {library ? "Save" : "Add library"}
        </button>
        <button type="button" className="btn btn-quiet" onClick={onDone} disabled={save.isPending}>
          Cancel
        </button>
      </div>
      {library && confirming && (
        <ConfirmDialog
          title={`Change ${library.name} to ${LIBRARY_TYPES[confirming.kind].label}?`}
          confirmLabel="Change type"
          pending={save.isPending}
          error={save.isError ? describeError(save.error) : null}
          onConfirm={() => save.mutate(confirming, { onSettled: () => setConfirming(null) })}
          onCancel={() => setConfirming(null)}
        >
          <p>Items in this library are rebuilt; watch progress in it is reset.</p>
          <p>
            The next scan sorts its files as {confirming.kind === "shows" ? "episodes, grouped into shows and seasons" : "movies, one per file"}.
          </p>
        </ConfirmDialog>
      )}
    </form>
  );
}

function RemoveLibrary({ library, onClose }: { library: AdminLibrary; onClose: () => void }) {
  const qc = useQueryClient();
  const remove = useMutation({
    mutationFn: () => client.admin.deleteLibrary(library.id),
    onSuccess: async () => {
      qc.setQueryData<AdminLibrary[]>(queryKeys.adminLibraries, (all) => all?.filter((l) => l.id !== library.id));
      await Promise.all([qc.invalidateQueries({ queryKey: queryKeys.home }), qc.invalidateQueries({ queryKey: queryKeys.rows })]);
      onClose();
    },
  });
  return (
    <ConfirmDialog
      title={`Remove ${library.name}?`}
      confirmLabel="Remove library"
      pending={remove.isPending}
      error={remove.isError ? describeError(remove.error) : null}
      onConfirm={() => remove.mutate()}
      onCancel={onClose}
    >
      <p>
        <strong>{itemCount(library)}</strong> leave Nuvora for everyone, along with their artwork.
      </p>
      <p>The files stay on disk. Adding the folder again brings them back.</p>
    </ConfirmDialog>
  );
}
