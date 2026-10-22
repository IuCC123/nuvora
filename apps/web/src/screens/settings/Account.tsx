import { useMutation } from "@tanstack/react-query";
import { checkDisplayName, checkPassword, checkPasswordsMatch, PASSWORD_MIN_LENGTH, type User } from "@nuvora/core";
import { client } from "../../nuvora";
import { FormError, TextField } from "../../components/Field";
import { setSessionUser, useUser } from "../../lib/session";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { Saved, SettingsPage } from "./SettingsPage";

/** Every user's own name and password. */
export function SettingsAccount() {
  const user = useUser();
  if (!user) return null;
  return (
    <SettingsPage title="Account" lede={`You're signed in as ${user.username}.`}>
      <Profile key={user.display_name} user={user} />
      <Password />
    </SettingsPage>
  );
}

function Profile({ user }: { user: User }) {
  const save = useMutation({ mutationFn: client.updateMe, onSuccess: setSessionUser });
  const form = useForm({ display_name: user.display_name }, (v) => ({ display_name: checkDisplayName(v.display_name, "your name") }), save.reset);
  const failed = serverError(save.error, ["display_name"]);
  return (
    <section className="settings-section" aria-labelledby="profile">
      <h2 id="profile">Profile</h2>
      <form className="form settings-form" noValidate onSubmit={form.submit((v) => save.mutate({ display_name: v.display_name.trim() }))}>
        <TextField
          label="Display name"
          autoComplete="name"
          hint="Shown in the account menu and to admins."
          {...form.field("display_name")}
          error={form.error("display_name") ?? failed.field.display_name}
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

function Password() {
  const username = useUser()?.username ?? "";
  const save = useMutation({ mutationFn: client.updateMe });
  const form = useForm(
    { current_password: "", new_password: "", confirm: "" },
    (v) => ({
      current_password: v.current_password ? null : "Enter your current password.",
      new_password: checkPassword(v.new_password),
      confirm: checkPasswordsMatch(v.new_password, v.confirm),
    }),
    save.reset,
  );
  const failed = serverError(save.error, ["current_password", "new_password"], { bad_credentials: "Your current password is wrong." });
  return (
    <section className="settings-section" aria-labelledby="password">
      <h2 id="password">Password</h2>
      <form
        className="form settings-form"
        noValidate
        onSubmit={form.submit((v) =>
          save.mutate({ current_password: v.current_password, new_password: v.new_password }, { onSuccess: () => form.reset() }),
        )}
      >
        <input type="text" name="username" autoComplete="username" value={username} readOnly hidden />
        <TextField
          label="Current password"
          type="password"
          autoComplete="current-password"
          {...form.field("current_password")}
          error={form.error("current_password") ?? failed.field.current_password}
        />
        <div className="form-row">
          <TextField
            label="New password"
            type="password"
            autoComplete="new-password"
            hint={`At least ${PASSWORD_MIN_LENGTH} characters.`}
            {...form.field("new_password")}
            error={form.error("new_password") ?? failed.field.new_password}
          />
          <TextField label="Confirm new password" type="password" autoComplete="new-password" {...form.field("confirm")} />
        </div>
        <FormError message={failed.form} />
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={save.isPending} aria-busy={save.isPending}>
            Change password
          </button>
          <Saved show={save.isSuccess && !form.dirty}>Password changed. Your other devices are signed out.</Saved>
        </div>
      </form>
    </section>
  );
}
