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
          <ServerName key={server.data.server_name} name={server.data.server_name} />
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
      <form className="form settings-form" noValidate onSubmit={form.submit((v) => save.mutate({ server_name: v.server_name.trim() }))}>
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

function About({ server }: { server: ServerInfo }) {
  const facts: [string, string, string?][] = [
    ["Version", server.version],
    ["Address", server.bind],
    ["Data folder", server.data_dir],
    ["FFmpeg", server.ffmpeg.version, server.ffmpeg.path],
    ["FFprobe", server.ffprobe.version, server.ffprobe.path],
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
