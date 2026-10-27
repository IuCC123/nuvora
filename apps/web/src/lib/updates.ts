import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { describeError, queryKeys } from "@nuvora/core";
import { client } from "../nuvora";
import { desktop, desktopError } from "../platform/desktop";
import { useUser } from "./session";

/** The server version this page was loaded from: the first one it hears. */
let loadedFrom: string | undefined;

/**
 * The server's new version once it was updated under this open page, which
 * still runs the old web app. Checked every ten minutes and when the window
 * regains focus.
 */
export function useServerUpdated(): string | null {
  const { data } = useQuery({
    queryKey: queryKeys.health,
    queryFn: ({ signal }) => client.health(signal),
    refetchInterval: 10 * 60_000,
    staleTime: 60_000,
  });
  if (data && loadedFrom === undefined) loadedFrom = data.version;
  return data && data.version !== loadedFrom ? data.version : null;
}

/** What GitHub's newest release is, for admins. `refresh` asks GitHub now. */
export function useServerUpdate() {
  const admin = useUser()?.role === "admin";
  return useQuery({
    queryKey: queryKeys.adminUpdate,
    queryFn: ({ signal }) => client.admin.update(false, signal),
    enabled: admin,
    staleTime: 5 * 60_000,
    retry: false,
  });
}

type Install = { step: "idle" } | { step: "downloading" | "restarting"; version: string } | { step: "failed"; message: string };

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Installs the newest server version, then follows it: the server downloads
 * it, restarts as it, and this page reloads into the new web app once the
 * server answers with the new version.
 */
export function useInstallServerUpdate() {
  const [install, setInstall] = useState<Install>({ step: "idle" });
  const start = async (version: string) => {
    setInstall({ step: "downloading", version });
    try {
      await client.admin.installUpdate();
    } catch (e) {
      return setInstall({ step: "failed", message: describeError(e) });
    }
    const deadline = Date.now() + 5 * 60_000;
    while (Date.now() < deadline) {
      await sleep(1500);
      const health = await client.health().catch(() => null);
      if (health?.version === version) return location.reload();
      // Unreachable means it is restarting; reachable on the old version, still downloading or failed.
      const status = health ? await client.admin.update().catch(() => null) : null;
      if (status?.phase === "failed") return setInstall({ step: "failed", message: status.error ?? "The update failed." });
      setInstall({ step: !health || status?.phase === "restarting" ? "restarting" : "downloading", version });
    }
    setInstall({ step: "failed", message: "Nuvora hasn't come back after the update. Check that the server is running, then reload this page." });
  };
  return { install, start };
}

/** The Mac or Windows app's name and version; never fetched in a browser. */
export function useDesktopApp() {
  return useQuery({ queryKey: ["desktop", "app"], queryFn: () => desktop!.app(), enabled: desktop !== null, staleTime: Infinity });
}

/** A newer Mac or Windows app, checked once per launch (and on "Check again"). */
export function useDesktopUpdate() {
  return useQuery({
    queryKey: ["desktop", "update"],
    queryFn: () => desktop!.checkUpdate(),
    enabled: desktop !== null,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
}

/** Downloads and installs the app update, then the app restarts; only a failure comes back. */
export function useInstallDesktopUpdate() {
  const install = useMutation({ mutationFn: () => desktop!.installUpdate() });
  return { ...install, message: install.error ? desktopError(install.error) : null };
}

const DISMISSED = "nuvora.updates.dismissed";

/** A version someone chose "Later" for stays quiet until a newer one comes. */
export function useDismissed(kind: "server" | "app") {
  const [dismissed, setDismissed] = useState<Record<string, string>>(() => {
    try {
      return JSON.parse(localStorage.getItem(DISMISSED) ?? "{}") as Record<string, string>;
    } catch {
      return {};
    }
  });
  const dismiss = (version: string) => {
    const next = { ...dismissed, [kind]: version };
    setDismissed(next);
    localStorage.setItem(DISMISSED, JSON.stringify(next));
  };
  return { isDismissed: (version: string) => dismissed[kind] === version, dismiss };
}
