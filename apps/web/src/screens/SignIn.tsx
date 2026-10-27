import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { safeRedirect } from "@nuvora/core";
import { client, queries } from "../nuvora";
import { AuthScreen } from "../components/AuthScreen";
import { FormError, TextField } from "../components/Field";
import { forgetResponses, setSessionUser } from "../lib/session";
import { serverError } from "../lib/serverError";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useForm } from "../lib/useForm";
import { desktop } from "../platform/desktop";

/** No sign-up link: only an admin creates accounts. */
export function SignIn() {
  useDocumentTitle("Sign in");
  const { redirect } = useSearch({ from: "/signin" });
  const navigate = useNavigate();
  const serverName = useQuery(queries.authState()).data?.server_name;
  const login = useMutation({
    mutationFn: client.login,
    onSuccess: async (user) => {
      forgetResponses();
      setSessionUser(user);
      await navigate({ href: safeRedirect(redirect), replace: true });
    },
  });
  const form = useForm(
    { username: "", password: "" },
    (v) => ({ username: v.username.trim() ? null : "Enter your username.", password: v.password ? null : "Enter your password." }),
    login.reset,
  );
  const failed = serverError(login.error, []);

  return (
    <AuthScreen>
      <div className="auth-card">
        <h1>Sign in</h1>
        <p className="auth-lede">{serverName && serverName !== "Nuvora" ? `to ${serverName}` : "to your Nuvora server"}</p>
        <form className="form" noValidate onSubmit={form.submit((v) => login.mutate({ username: v.username.trim(), password: v.password }))}>
          <TextField label="Username" floating autoComplete="username" autoCapitalize="none" spellCheck={false} autoFocus {...form.field("username")} />
          <TextField label="Password" floating type="password" autoComplete="current-password" {...form.field("password")} />
          <FormError message={failed.form} />
          <button type="submit" className="btn btn-primary btn-block" disabled={login.isPending || login.isSuccess} aria-busy={login.isPending}>
            {login.isPending ? "Signing in…" : "Sign in"}
          </button>
        </form>
        <p className="auth-foot">
          No account yet? Ask the person who runs this server to add you.
          {desktop && (
            <>
              {" "}
              Wrong server?{" "}
              <button type="button" className="auth-foot-link" onClick={() => void desktop?.switchServer()}>
                Switch server
              </button>
            </>
          )}
        </p>
      </div>
    </AuthScreen>
  );
}
