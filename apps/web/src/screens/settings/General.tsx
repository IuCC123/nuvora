import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { checkRequired, formatUptime, queryKeys, type AuthState, type ServerInfo } from "@nuvora/core";
import { client, queries } from "../../nuvora";
import { FormError, TextField } from "../../components/Field";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { LoadError, Saved, SettingsPage } from "./SettingsPage";

export function SettingsGeneral() {
  const server = useQuery(queries.adminServer());
  return (
    <SettingsPage title="General" lede="This server's name, and what it runs on.">
      {server.isPending ? (
        <div className="server-facts-skeleton" aria-busy="true" aria-label="Loading server details" />
      ) : server.isError ? (
        <LoadError error={server.error} retry={() => void server.refetch()} />
      ) : (
        <>
          <ServerName name={server.data.server_name} />
          <Metadata />
          <About server={server.data} />
        </>
      )}
    </SettingsPage>
  );
}

function ServerName({ name }: { name: string }) {
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: client.admin.updateServer,
    onSuccess: (server) => {
      qc.setQueryData(queryKeys.adminServer, server);
      qc.setQueryData<AuthState>(queryKeys.auth, (auth) => auth && { ...auth, server_name: server.server_name });
    },
  });
  const form = useForm({ server_name: name }, (v) => ({ server_name: checkRequired(v.server_name, "a name for this server") }), save.reset);
  const failed = serverError(save.error, ["server_name"]);
  return (
    <section className="settings-section" aria-labelledby="server-name">
      <h2 id="server-name">Server name</h2>
      <form
        className="form settings-form"
        noValidate
        onSubmit={form.submit((v) =>
          save.mutate({ server_name: v.server_name.trim() }, { onSuccess: (saved) => form.reset({ server_name: saved.server_name }) }),
        )}
      >
        <TextField
          label="Name"
          hint="People see it on the sign-in screen."
          {...form.field("server_name")}
          error={form.error("server_name") ?? failed.field.server_name}
        />
        <FormError message={failed.form} />
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={!form.dirty || save.isPending} aria-busy={save.isPending}>
            Save
          </button>
          <Saved show={save.isSuccess && !form.dirty} />
        </div>
      </form>
    </section>
  );
}

function Metadata() {
  const qc = useQueryClient();
  const settings = useQuery(queries.adminMetadata());
  const save = useMutation({
    mutationFn: client.admin.updateMetadata,
    onSuccess: (saved) => qc.setQueryData(queryKeys.adminMetadata, saved),
  });
  const refresh = useMutation({ mutationFn: client.admin.refreshMetadata });
  const form = useForm({ tmdb_api_key: "" }, () => ({}), save.reset);
  const failed = serverError(save.error, ["tmdb_api_key"]);
  const keySet = settings.data?.tmdb_key_set ?? false;
  return (
    <section className="settings-section" aria-labelledby="metadata">
      <h2 id="metadata">Descriptions and artwork</h2>
      <p className="settings-note">
        Nuvora looks up synopses, posters, cast and voice actors online, with no setup: anime from AniList, TV shows from TVmaze, and movies
        from Wikipedia. A free TMDB key adds ratings and better posters for movies: create an account at themoviedb.org, then copy the API key
        from Settings › API.
      </p>
      <form
        className="form settings-form"
        noValidate
        onSubmit={form.submit((v) => save.mutate({ tmdb_api_key: v.tmdb_api_key.trim() || null }, { onSuccess: () => form.reset({ tmdb_api_key: "" }) }))}
      >
        <TextField
          label="TMDB API key"
          hint={keySet ? "A key is saved. Enter a new one to replace it, or save empty to remove it." : "Movies stay without descriptions until a key is saved."}
          autoComplete="off"
          spellCheck={false}
          {...form.field("tmdb_api_key")}
          error={failed.field.tmdb_api_key}
        />
        <FormError message={failed.form} />
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={(!form.dirty && !keySet) || save.isPending} aria-busy={save.isPending}>
            {form.dirty || !keySet ? "Save key" : "Remove key"}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => refresh.mutate()} disabled={refresh.isPending} aria-busy={refresh.isPending}>
            Refresh all metadata
          </button>
          <Saved show={save.isSuccess && !form.dirty} />
          {refresh.isSuccess && <span className="settings-note-inline">Looking everything up again. This runs in the background.</span>}
        </div>
      </form>
    </section>
  );
}

function About({ server }: { server: ServerInfo }) {
  const facts: [string, string, string?][] = [
    ["Version", server.version],
    ["Address", server.bind],
    ["Data folder", server.data_dir],
    ["FFmpeg", server.ffmpeg.version, server.ffmpeg.path],
    ["FFprobe", server.ffprobe.version, server.ffprobe.path],
    ["Converts video with", server.video_encoder],
    ["Running for", formatUptime(server.uptime_s)],
  ];
  return (
    <section className="settings-section" aria-labelledby="about-server">
      <h2 id="about-server">About this server</h2>
      <dl className="server-facts">
        {facts.map(([term, value, detail]) => (
          <div key={term}>
            <dt>{term}</dt>
            <dd>
              {value}
              {detail && <span className="server-facts-detail">{detail}</span>}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
