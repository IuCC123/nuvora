import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { SubtitleSettings } from "@nuvora/core";
import { client } from "../../nuvora";
import { FormError, TextField } from "../../components/Field";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { LoadError, Saved, SettingsPage, SkeletonRows } from "./SettingsPage";

const KEY = ["admin", "subtitle-settings"] as const;

/** The key for finding subtitles online, for films and episodes whose files have none in the viewer's language. */
export function SettingsSubtitles() {
  const settings = useQuery({ queryKey: KEY, queryFn: ({ signal }) => client.admin.subtitleSettings(signal) });
  return (
    <SettingsPage
      title="Subtitles"
      lede="Find subtitles online when a file has none in someone's language, or none their device can show. The player picks the one timed for the file when there is one."
    >
      {settings.isPending ? (
        <SkeletonRows count={1} label="Loading subtitle settings" />
      ) : settings.isError ? (
        <LoadError error={settings.error} retry={() => void settings.refetch()} />
      ) : (
        <OnlineSearch settings={settings.data} />
      )}
    </SettingsPage>
  );
}

function OnlineSearch({ settings }: { settings: SubtitleSettings }) {
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: client.admin.setSubtitleSettings,
    onSuccess: (next) => {
      qc.setQueryData(KEY, next);
      // Searches made without a key said so; ask again.
      void qc.invalidateQueries({ queryKey: ["subtitles-online"] });
      form.reset({ key: "" });
    },
  });
  const form = useForm({ key: "" }, (v) => ({ key: v.key.trim() ? null : "Paste the key" }), save.reset);
  const failed = serverError(save.error, ["online_key"]);
  return (
    <section className="settings-section" aria-labelledby="online-search">
      <div>
        <h2 id="online-search">Online search</h2>
        <p className="settings-section-note">{settings.online_search ? "On." : "Off until a key is added."}</p>
      </div>
      <form
        className="form settings-form"
        noValidate
        onSubmit={form.submit((v) => save.mutate({ online_key: v.key.trim() }))}
      >
        <p className="settings-note">
          Nuvora searches with <a href="https://store.wyzie.io/redeem" target="_blank" rel="noreferrer">Wyzie Subs</a>, which looks through
          OpenSubtitles, SubDL, Podnapisi and more at once. Its key is free and needs no account, and allows 1,000 searches a day.
        </p>
        <TextField
          label={settings.online_search ? "Replace the key" : "Key"}
          autoComplete="off"
          spellCheck={false}
          hint="Kept on the server. It never comes back to this page."
          {...form.field("key")}
          error={form.field("key").error ?? failed.field.online_key}
        />
        <FormError message={failed.form} />
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={save.isPending} aria-busy={save.isPending}>
            {save.isPending ? "Checking the key" : "Save key"}
          </button>
          {settings.online_search && (
            <button type="button" className="btn btn-secondary" disabled={save.isPending} onClick={() => save.mutate({ online_key: null })}>
              Remove key
            </button>
          )}
          <Saved show={save.isSuccess && !form.dirty} />
        </div>
      </form>
    </section>
  );
}
