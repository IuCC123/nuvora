import { QueryClient } from "@tanstack/react-query";
import { ApiError, createClient, createQueries, type FetchLike } from "@nuvora/core";
import { probeDeviceProfile } from "./platform/deviceProfile";

const browserFetch: FetchLike = (url, init) => fetch(url, init as RequestInit);

// The condition is a build-time constant, so production bundles drop the fixtures module.
const transport: FetchLike =
  import.meta.env.VITE_FIXTURES === "1" ? (await import("./dev/fixtures")).fixtureFetch : browserFetch;

let unauthorized = () => {};

/** The router registers what a 401 does (go to Sign in); it is created after the client. */
export function onUnauthorized(handler: () => void) {
  unauthorized = handler;
}

/** App-wide singletons. The only place the web app wires platform APIs into @nuvora/core. */
export const client = createClient({
  baseUrl: "",
  fetch: transport,
  device: probeDeviceProfile(),
  onUnauthorized: () => unauthorized(),
});

export const queries = createQueries(client);

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: true,
      // A 4xx (a deleted movie's 404) will not change on retry; a dropped connection might.
      retry: (failures, error) => failures < 1 && !(error instanceof ApiError && error.status < 500),
    },
  },
});
