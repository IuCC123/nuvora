import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { ApiError, queryKeys, type AuthState, type User } from "@nuvora/core";
import { client, queries, queryClient } from "../nuvora";

/** The signed-in user, or `null` in the moment between a sign-out and the move to Sign in. */
export function useUser(): User | null {
  return useQuery(queries.authState()).data?.user ?? null;
}

/** Record who is signed in from a login, setup or logout answer, without asking the server again. */
export function setSessionUser(user: User | null) {
  queryClient.setQueryData<AuthState>(queryKeys.auth, (previous) => ({
    server_name: previous?.server_name ?? "Nuvora",
    server_id: previous?.server_id ?? "",
    setup_required: false,
    user,
  }));
}

/**
 * Drop every cached response except the auth state, because what the server
 * returns depends on who asks. Call it once no screen shows them any more.
 */
export function forgetResponses() {
  queryClient.removeQueries({ predicate: (q) => q.queryKey[0] !== queryKeys.auth[0] });
}

/**
 * Sign out, then go to Sign in with nothing of the old session left in memory.
 * A 401 means the session was already gone, which counts as signed out; any
 * other failure leaves the session alive on the server, so the person stays signed in.
 */
export function useSignOut() {
  const navigate = useNavigate();
  return useMutation({
    mutationFn: () => client.logout().catch((e: unknown) => {
      if (!(e instanceof ApiError && e.status === 401)) throw e;
    }),
    onSuccess: async () => {
      setSessionUser(null);
      await navigate({ to: "/signin" });
      forgetResponses();
    },
  });
}
