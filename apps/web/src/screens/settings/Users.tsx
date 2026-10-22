import { Fragment, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  checkDisplayName,
  checkPassword,
  checkUsername,
  describeError,
  formatLastSeen,
  initials,
  PASSWORD_MIN_LENGTH,
  queryKeys,
  type AdminUser,
  type Role,
} from "@nuvora/core";
import { client, queries } from "../../nuvora";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { FormError, SelectField, TextField } from "../../components/Field";
import { PlusIcon } from "../../components/Icon";
import { setSessionUser, useUser } from "../../lib/session";
import { serverError } from "../../lib/serverError";
import { useForm } from "../../lib/useForm";
import { LoadError, SettingsPage, SkeletonRows } from "./SettingsPage";

type Editing = null | "new" | number;

const ROLE_LABEL: Record<Role, string> = { admin: "Admin", member: "Member" };
const ROLE_HINT = "Admins manage libraries, users and the server. Members browse and play.";

export function SettingsUsers() {
  const users = useQuery(queries.adminUsers());
  const me = useUser();
  const [editing, setEditing] = useState<Editing>(null);
  const [deleting, setDeleting] = useState<AdminUser | null>(null);
  const now = Date.now();

  return (
    <SettingsPage
      title="Users"
      lede="Everyone who can sign in to this server. Each person gets their own account."
      action={
        editing !== "new" && (
          <button type="button" className="btn btn-primary btn-small" onClick={() => setEditing("new")}>
            <PlusIcon size={16} />
            Add user
          </button>
        )
      }
    >
      {editing === "new" && <AddUser onDone={() => setEditing(null)} />}
      {users.isPending ? (
        <SkeletonRows count={3} label="Loading users" />
      ) : users.isError ? (
        <LoadError error={users.error} retry={() => void users.refetch()} />
      ) : (
        <div className="settings-list">
          <table className="users">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Role</th>
                <th scope="col">Status</th>
                <th scope="col">Last seen</th>
                <th scope="col">
                  <span className="sr-only">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {users.data.map((user) => (
                <Fragment key={user.id}>
                  <tr data-disabled={user.disabled} data-editing={editing === user.id}>
                    <td>
                      <span className="user-cell">
                        <span className="avatar" aria-hidden>
                          {initials(user.display_name)}
                        </span>
                        <span className="user-names">
                          <span className="user-name">
                            {user.display_name}
                            {user.id === me?.id && <span className="you">You</span>}
                          </span>
                          <span className="user-handle">{user.username}</span>
                        </span>
                      </span>
                    </td>
                    <td>{ROLE_LABEL[user.role]}</td>
                    <td>
                      <span className="status" data-disabled={user.disabled}>
                        {user.disabled ? "Disabled" : "Active"}
                      </span>
                    </td>
                    <td className="tabular">{formatLastSeen(user.last_seen_at ?? null, now)}</td>
                    <td className="users-actions">
                      {editing !== user.id && (
                        <button type="button" className="btn btn-quiet btn-small" onClick={() => setEditing(user.id)} aria-label={`Edit ${user.display_name}`}>
                          Edit
                        </button>
                      )}
                    </td>
                  </tr>
                  {editing === user.id && (
                    <tr className="users-editor-row">
                      <td colSpan={5}>
                        <EditUser user={user} self={user.id === me?.id} onDone={() => setEditing(null)} onDelete={() => setDeleting(user)} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {deleting && (
        <DeleteUser
          user={deleting}
          onClose={(deleted) => {
            setDeleting(null);
            if (deleted) setEditing(null);
          }}
        />
      )}
    </SettingsPage>
  );
}

function AddUser({ onDone }: { onDone: () => void }) {
  const qc = useQueryClient();
  const add = useMutation({
    mutationFn: client.admin.createUser,
    onSuccess: (user) => {
      qc.setQueryData<AdminUser[]>(queryKeys.adminUsers, (all) => all && [...all, user]);
      onDone();
    },
  });
  const form = useForm(
    { display_name: "", username: "", password: "", role: "member" },
    (v) => ({ display_name: checkDisplayName(v.display_name, "their name"), username: checkUsername(v.username), password: checkPassword(v.password) }),
    add.reset,
  );
  const failed = serverError(add.error, ["display_name", "username", "password"]);
  return (
    <form
      className="form editor"
      noValidate
      aria-label="Add a user"
      onSubmit={form.submit((v) =>
        add.mutate({ display_name: v.display_name.trim(), username: v.username, password: v.password, role: v.role as Role }),
      )}
      onKeyDown={(e) => e.key === "Escape" && !add.isPending && onDone()}
    >
      <h2 className="editor-title">Add a user</h2>
      <div className="form-row">
        <TextField label="Name" autoComplete="off" autoFocus {...form.field("display_name")} error={form.error("display_name") ?? failed.field.display_name} />
        <TextField
          label="Username"
          autoComplete="off"
          autoCapitalize="none"
          spellCheck={false}
          {...form.field("username")}
          error={form.error("username") ?? failed.field.username}
        />
      </div>
      <div className="form-row">
        <TextField
          label="Password"
          type="password"
          autoComplete="new-password"
          hint={`At least ${PASSWORD_MIN_LENGTH} characters. Share it privately; they can change it in Account settings.`}
          {...form.field("password")}
          error={form.error("password") ?? failed.field.password}
        />
        <SelectField label="Role" hint={ROLE_HINT} {...form.field("role")}>
          <option value="member">Member</option>
          <option value="admin">Admin</option>
        </SelectField>
      </div>
      <FormError message={failed.form} />
      <div className="form-actions">
        <button type="submit" className="btn btn-primary" disabled={add.isPending} aria-busy={add.isPending}>
          Add user
        </button>
        <button type="button" className="btn btn-quiet" onClick={onDone} disabled={add.isPending}>
          Cancel
        </button>
      </div>
    </form>
  );
}

/**
 * Name, role, a new password, and disabling. Nobody can disable or delete
 * themselves; the server also refuses to leave the server without an admin.
 */
function EditUser({ user, self, onDone, onDelete }: { user: AdminUser; self: boolean; onDone: () => void; onDelete: () => void }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const me = useUser();
  const save = useMutation({
    mutationFn: (changes: Parameters<typeof client.admin.updateUser>[1]) => client.admin.updateUser(user.id, changes),
    onSuccess: async (updated) => {
      qc.setQueryData<AdminUser[]>(queryKeys.adminUsers, (all) => all?.map((u) => (u.id === updated.id ? updated : u)));
      if (!self || !me) return;
      setSessionUser({ ...me, username: updated.username, display_name: updated.display_name, role: updated.role });
      // An admin who made themselves a member can no longer see this screen.
      if (updated.role !== "admin") await navigate({ to: "/settings/account" });
    },
  });
  const form = useForm(
    { display_name: user.display_name, role: user.role as string, password: "" },
    (v) => ({ display_name: checkDisplayName(v.display_name, "their name"), password: v.password ? checkPassword(v.password) : null }),
    save.reset,
  );
  const failed = serverError(save.error, ["display_name", "password"]);
  const toggle = useMutation({
    mutationFn: () => client.admin.updateUser(user.id, { disabled: !user.disabled }),
    onSuccess: (updated) => qc.setQueryData<AdminUser[]>(queryKeys.adminUsers, (all) => all?.map((u) => (u.id === updated.id ? updated : u))),
  });

  return (
    <form
      className="form editor editor-inline"
      noValidate
      aria-label={`Edit ${user.display_name}`}
      onSubmit={form.submit((v) =>
        save.mutate(
          {
            ...(v.display_name.trim() !== user.display_name && { display_name: v.display_name.trim() }),
            ...(v.role !== user.role && { role: v.role as Role }),
            ...(v.password && { password: v.password }),
          },
          { onSuccess: onDone },
        ),
      )}
      onKeyDown={(e) => e.key === "Escape" && !save.isPending && onDone()}
    >
      <div className="form-row">
        <TextField label="Name" autoFocus {...form.field("display_name")} error={form.error("display_name") ?? failed.field.display_name} />
        <SelectField label="Role" hint={self ? "Make someone else an admin before you step down." : ROLE_HINT} {...form.field("role")}>
          <option value="member">Member</option>
          <option value="admin">Admin</option>
        </SelectField>
      </div>
      <div className="form-row">
        <TextField
          label="New password"
          type="password"
          autoComplete="new-password"
          placeholder="Leave empty to keep it"
          hint={self ? undefined : "Setting one signs them out everywhere."}
          {...form.field("password")}
          error={form.error("password") ?? failed.field.password}
        />
      </div>
      <FormError message={failed.form ?? (toggle.isError ? describeError(toggle.error) : null)} />
      <div className="form-actions">
        <button type="submit" className="btn btn-primary" disabled={save.isPending || !form.dirty} aria-busy={save.isPending}>
          Save
        </button>
        <button type="button" className="btn btn-quiet" onClick={onDone} disabled={save.isPending}>
          Cancel
        </button>
        <span className="form-actions-spacer" />
        {self ? (
          <p className="field-hint">You can't disable or delete your own account.</p>
        ) : (
          <>
            <button type="button" className="btn btn-secondary btn-small" onClick={() => toggle.mutate()} disabled={toggle.isPending} aria-busy={toggle.isPending}>
              {user.disabled ? "Enable" : "Disable"}
            </button>
            <button type="button" className="btn btn-quiet btn-small btn-quiet-danger" onClick={onDelete}>
              Delete user
            </button>
          </>
        )}
      </div>
    </form>
  );
}

function DeleteUser({ user, onClose }: { user: AdminUser; onClose: (deleted: boolean) => void }) {
  const qc = useQueryClient();
  const remove = useMutation({
    mutationFn: () => client.admin.deleteUser(user.id),
    onSuccess: () => {
      qc.setQueryData<AdminUser[]>(queryKeys.adminUsers, (all) => all?.filter((u) => u.id !== user.id));
      onClose(true);
    },
  });
  return (
    <ConfirmDialog
      title={`Delete ${user.display_name}?`}
      confirmLabel="Delete user"
      pending={remove.isPending}
      error={remove.isError ? describeError(remove.error) : null}
      onConfirm={() => remove.mutate()}
      onCancel={() => onClose(false)}
    >
      <p>
        <strong>{user.username}</strong> can no longer sign in, and is signed out everywhere.
      </p>
      <p>To keep the account but stop them signing in for now, disable it instead.</p>
    </ConfirmDialog>
  );
}
