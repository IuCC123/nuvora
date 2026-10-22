import { QueryClient } from "@tanstack/react-query";
import { createClient, createQueries } from "@nuvora/core";
import { probeDeviceProfile } from "./platform/deviceProfile";

/** App-wide singletons. The only place the web app wires platform APIs into @nuvora/core. */
export const client = createClient({
  baseUrl: "",
  fetch: (url, init) => fetch(url, init as RequestInit),
  device: probeDeviceProfile(),
});

export const queries = createQueries(client);

export const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: true, retry: 1 } },
});
