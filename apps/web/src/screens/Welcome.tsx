import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { checkPassword, checkPasswordsMatch, checkRequired, checkUsername, PASSWORD_MIN_LENGTH, queryKeys } from "@nuvora/core";
import { client, queries } from "../nuvora";
import { AuthScreen } from "../components/AuthScreen";
import { FormError, TextField } from "../components/Field";
import { FolderList } from "../components/FolderList";
import { setSessionUser } from "../lib/session";
import { serverError } from "../lib/serverError";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useForm } from "../lib/useForm";

function Steps({ step }: { step: 1 | 2 }) {
  return (
    <div className="steps">
      <span data-done="true" />
      <span data-done={step === 2} />
      <p className="sr-only">Step {step} of 2</p>
    </div>
  );
}

/** First run, step 1: the admin account. The server signs its creator in. */
export function WelcomeAccount() {
  useDocumentTitle("Welcome");
  const navigate = useNavigate();
  const setup = useMutation({
    mutationFn: client.setup,
    onSuccess: async (user) => {
      setSessionUser(user);
      await navigate({ to: "/welcome/library" });
    },
  });
  const form = useForm(
    { display_name: "", username: "", password: "", confirm: "" },
    (v) => ({
      display_name: checkRequired(v.display_name, "your name"),
      username: checkUsername(v.username),
      password: checkPassword(v.password),
      confirm: checkPasswordsMatch(v.password, v.confirm),
    }),
    setup.reset,
  );
  const failed = serverError(setup.error, ["display_name", "username", "password"]);

  return (
    <AuthScreen>
      <div className="auth-card auth-card-wide">
        <Steps step={1} />
        <h1>Welcome to Nuvora</h1>
        <p className="auth-lede">Create the admin account. It adds libraries and decides who can watch.</p>
        <form
          className="form"
          noValidate
          onSubmit={form.submit((v) => setup.mutate({ display_name: v.display_name.trim(), username: v.username, password: v.password }))}
        >
          <TextField label="Your name" autoComplete="name" autoFocus {...form.field("display_name")} />
          <TextField
            label="Username"
            autoComplete="username"
            autoCapitalize="none"
            spellCheck={false}
            hint="You sign in with this. Letters, digits, dots, dashes and underscores."
            {...form.field("username")}
            error={form.error("username") ?? failed.field.username}
          />
          <div className="form-row">
            <TextField
              label="Password"
              type="password"
              autoComplete="new-password"
              hint={`At least ${PASSWORD_MIN_LENGTH} characters.`}
              {...form.field("password")}
              error={form.error("password") ?? failed.field.password}
            />
            <TextField label="Confirm password" type="password" autoComplete="new-password" {...form.field("confirm")} />
          </div>
          <FormError message={failed.form} />
          <button type="submit" className="btn btn-primary btn-block" disabled={setup.isPending || setup.isSuccess} aria-busy={setup.isPending}>
            {setup.isPending ? "Creating account…" : "Create account"}
          </button>
        </form>
      </div>
    </AuthScreen>
  );
}

/** First run, step 2: the first library. Adding it starts a scan, and Home fills in while it runs. */
export function WelcomeLibrary() {
  useDocumentTitle("Add a library");
  const navigate = useNavigate();
  const qc = useQueryClient();
  const existing = useQuery(queries.adminLibraries()).data ?? [];
  const create = useMutation({
    mutationFn: client.admin.createLibrary,
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.scan });
      await navigate({ to: "/" });
    },
  });
  const form = useForm(
    { name: "Movies", paths: [] as readonly string[] },
    (v) => ({ name: checkRequired(v.name, "a name for the library"), paths: v.paths.length ? null : "Choose the folder your movies are in." }),
    create.reset,
  );
  const failed = serverError(create.error, ["name", "paths"]);

  return (
    <AuthScreen>
      <div className="auth-card auth-card-wide">
        <Steps step={2} />
        <h1>Add your movies</h1>
        <p className="auth-lede">
          Choose the folder that holds your movie files, and Nuvora brings them in. When you add more later, scan again from Settings.
        </p>
        {existing.length > 0 && (
          <p className="welcome-existing">
            {existing.map((l) => l.name).join(", ")} {existing.length === 1 ? "is" : "are"} already set up from the server's
            settings. Add another library here, or go straight to Home.
          </p>
        )}
        <form
          className="form"
          noValidate
          onSubmit={form.submit((v) => create.mutate({ name: v.name.trim(), kind: "movies", paths: [...v.paths] }))}
        >
          <TextField label="Library name" {...form.field("name")} error={form.error("name") ?? failed.field.name} />
          <FolderList
            label="Folder"
            paths={form.values.paths}
            onChange={(paths) => form.set("paths", paths)}
            error={form.error("paths") ?? failed.field.paths}
            openPicker
          />
          <FormError message={failed.form} />
          <div className="form-actions">
            <button type="submit" className="btn btn-primary" disabled={create.isPending || create.isSuccess} aria-busy={create.isPending}>
              {create.isPending ? "Adding…" : "Add library and scan"}
            </button>
            <Link to="/" className="btn btn-quiet">
              {existing.length ? "Go to Home" : "Skip for now"}
            </Link>
          </div>
        </form>
      </div>
    </AuthScreen>
  );
}
