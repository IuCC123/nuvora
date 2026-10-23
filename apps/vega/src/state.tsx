import { createContext, useContext } from 'react';
import type { DeviceProfile, NuvoraClient } from '@nuvora/core';
import type { Session } from './session';

export interface AppState {
  device: DeviceProfile;
  /** Null until signed in. */
  session: Session | null;
  client: NuvoraClient | null;
  signedIn(session: Session): void;
  signOut(): void;
  /**
   * Stops offering any video codec but H.264, for a TV whose decoder claims
   * more than it plays (Vega's virtual device claims HEVC). Lasts until the app
   * restarts. False when there was nothing left to drop.
   */
  onlyH264(): boolean;
}

export const AppContext = createContext<AppState | null>(null);

export function useApp(): AppState {
  const app = useContext(AppContext);
  if (!app) throw new Error('useApp outside the app');
  return app;
}

/** For screens behind sign-in: the session and its client, never null there. */
export function useSignedIn() {
  const app = useApp();
  if (!app.session || !app.client) throw new Error('this screen needs a session');
  return { ...app, session: app.session, client: app.client };
}
