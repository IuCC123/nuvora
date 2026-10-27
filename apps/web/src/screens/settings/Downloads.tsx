import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { describeError, queryKeys, type DownloadSettings } from "@nuvora/core";
import { FormError, SelectField, TextField } from "../../components/Field";
import { AlertIcon, TrashIcon } from "../../components/Icon";
import { client, queries } from "../../nuvora";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { LoadError, Saved, SettingsPage, SkeletonRows } from "./SettingsPage";
import "../../styles/downloads.css";

/** Where the Downloads page searches (Torznab indexers) and which library downloads land in. */
export function SettingsDownloads() {
  const settings = useQuery(queries.downloadSettings());
  return (
    <SettingsPage
      title="Downloads"
      lede="Connect the indexers the Downloads page searches, and choose the library new movies go into."
      action={
        <Link to="/downloads" className="btn btn-secondary">
          Open Downloads
        </Link>
      }
    >
      {settings.data?.unavailable && (
        <div className="downloads-alert" role="alert">
          <AlertIcon />
          <p>This server can't download right now: {settings.data.unavailable}. Restart it to try again.</p>
        </div>
      )}
      {settings.isPending ? (
        <SkeletonRows count={2} label="Loading indexers" />
      ) : settings.isError && !settings.data ? (
        <LoadError error={settings.error} retry={() => void settings.refetch()} />
      ) : (
        <>
          <Indexers settings={settings.data} />
          <Destination settings={settings.data} />
        </>
      )}
    </SettingsPage>
  );
}

function Indexers({ settings }: { settings: DownloadSettings }) {
  const qc = useQueryClient();
  const saved = (next: DownloadSettings) => {
    qc.setQueryData(queryKeys.downloadSettings, next);
    void qc.invalidateQueries({ queryKey: ["admin", "releases"] });
  };
  const add = useMutation({ mutationFn: client.admin.addIndexer, onSuccess: saved });
  const remove = useMutation({ mutationFn: client.admin.removeIndexer, onSuccess: saved });
  const form = useForm(
    { name: "", url: "", api_key: "" },
    (v) => ({
      name: v.name.trim() ? null : "Name it, like “Prowlarr”",
      url: /^https?:\/\/\S+$/.test(v.url.trim()) ? null : "Paste the feed's full address, starting with http",
    }),
    add.reset,
  );
  const failed = serverError(add.error, ["name", "url", "api_key"]);
  return (
    <section className="settings-section" aria-labelledby="indexers">
      <div>
        <h2 id="indexers">Indexers</h2>
        <p className="settings-section-note">Searched together; results are merged.</p>
      </div>
      <div className="indexers">
        {settings.indexers.length > 0 && (
          <ul role="list" className="settings-list">
            {settings.indexers.map((indexer) => (
              <li key={indexer.id} className="settings-row indexer-row">
                <div className="library-main">
                  <p className="library-name">{indexer.name}</p>
                  <p className="library-paths">{indexer.url}</p>
                </div>
                <button
                  type="button"
                  className="icon-btn indexer-remove"
                  aria-label={`Remove ${indexer.name}`}
                  onClick={() => remove.mutate(indexer.id)}
                  disabled={remove.isPending}
                >
                  <TrashIcon size={18} />
                </button>
              </li>
            ))}
          </ul>
        )}
        {remove.isError && <FormError message={describeError(remove.error)} />}
        <form
          className="form settings-form"
          noValidate
          onSubmit={form.submit((v) =>
            add.mutate(
              { name: v.name.trim(), url: v.url.trim(), api_key: v.api_key.trim() },
              { onSuccess: () => form.reset({ name: "", url: "", api_key: "" }) },
            ),
          )}
        >
          <h3 className="indexer-form-title">{settings.indexers.length === 0 ? "Add your first indexer" : "Add another indexer"}</h3>
          <p className="settings-note">
            In Jackett, use “Copy Torznab Feed” on an indexer, or the “all” feed to search every indexer at once. In Prowlarr, open an indexer and
            copy its Torznab URL, which ends in <code>/api</code>. Both show the API key at the top of their page.
          </p>
          <TextField label="Name" autoComplete="off" {...form.field("name")} error={form.field("name").error ?? failed.field.name} />
          <TextField
            label="Torznab feed"
            placeholder="http://localhost:9117/api/v2.0/indexers/all/results/torznab/"
            autoComplete="off"
            spellCheck={false}
            inputMode="url"
            {...form.field("url")}
            error={form.field("url").error ?? failed.field.url}
          />
          <TextField
            label="API key"
            autoComplete="off"
            spellCheck={false}
            hint="Kept on the server. It never comes back to this page."
            {...form.field("api_key")}
            error={failed.field.api_key}
          />
          <FormError message={failed.form} />
          <div className="form-actions">
            <button type="submit" className="btn btn-primary" disabled={add.isPending} aria-busy={add.isPending}>
              {add.isPending ? "Checking it answers" : "Add indexer"}
            </button>
            <Saved show={add.isSuccess && !form.dirty}>Added</Saved>
          </div>
        </form>
      </div>
    </section>
  );
}

function Destination({ settings }: { settings: DownloadSettings }) {
  const qc = useQueryClient();
  const libraries = useQuery(queries.libraries());
  const save = useMutation({
    mutationFn: client.admin.setDownloadLibrary,
    onSuccess: (next) => qc.setQueryData(queryKeys.downloadSettings, next),
  });
  const movies = (libraries.data ?? []).filter((l) => l.kind === "movies");
  return (
    <section className="settings-section" aria-labelledby="destination">
      <h2 id="destination">Save to</h2>
      <div className="settings-form">
        {libraries.data && movies.length === 0 ? (
          <p className="settings-note">
            Downloads go into a Movies library. <Link to="/settings/libraries">Add one</Link> first.
          </p>
        ) : (
          <SelectField
            label="Library"
            hint="Each movie gets its own folder in the library's first folder, named like “Heat (1995)”."
            value={settings.library_id === null ? "" : String(settings.library_id)}
            disabled={save.isPending}
            onChange={(e) => save.mutate(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))}
          >
            <option value="">{movies[0] ? `The first Movies library (${movies[0].name})` : "The first Movies library"}</option>
            {movies.map((l) => (
              <option key={l.id} value={l.id}>
                {l.name}
              </option>
            ))}
          </SelectField>
        )}
        <FormError message={save.isError ? describeError(save.error) : null} />
        <Saved show={save.isSuccess} />
      </div>
    </section>
  );
}
