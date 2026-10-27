import { invoke, isTauri } from "@tauri-apps/api/core";

/** A server the Mac or Windows app remembers or found on the network. */
export interface DesktopServer {
  url: string;
  name: string;
  id?: string | null;
}

export interface DesktopApp {
  /** "Nuvora for Mac", "Nuvora for Windows". */
  name: string;
  version: string;
}

export interface DesktopUpdate {
  version: string;
  notes: string | null;
}

/**
 * The native side of the Mac and Windows apps (`apps/desktop`), which show
 * this web app in the system's web view; `null` in a browser. Server pages may
 * only call the last four (see `apps/desktop/src-tauri/capabilities`).
 */
export const desktop = isTauri()
  ? {
      savedServer: () => invoke<DesktopServer | null>("saved_server"),
      discoveredServers: () => invoke<DesktopServer[]>("discovered_servers"),
      connect: (address: string) => invoke<DesktopServer>("connect", { address }),
      reconnect: () => invoke<DesktopServer>("reconnect"),
      switchServer: () => invoke<void>("switch_server"),
      app: () => invoke<DesktopApp>("app_info"),
      checkUpdate: () => invoke<DesktopUpdate | null>("check_app_update"),
      /** Downloads, installs and restarts the app; resolves only if that fails. */
      installUpdate: () => invoke<void>("install_app_update"),
    }
  : null;

/** Commands reject with the Rust side's sentence. */
export function desktopError(error: unknown): string {
  return typeof error === "string" ? error : "Something went wrong. Try again.";
}
