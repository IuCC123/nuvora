import { createContext, useContext } from 'react';
import type { DeviceProfile, NuvoraClient, User } from '@nuvora/core';
import type { LiveLink } from './live';
import type { Session } from './storage';

export interface AppState {
  device: DeviceProfile;
  /** Null until signed in. */
  session: Session | null;
  client: NuvoraClient | null;
  /** Watch Together's live connection; null until signed in. */
  live: LiveLink | null;
  signedIn(session: Session): void;
  /** Back to Sign in for the same server, or to Connect with `changeServer`. */
  signOut(options?: { changeServer?: boolean }): void;
  /** The account as the server now has it (after changing a setting). */
  updateUser(user: User): void;
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

/** For screens behind sign-in: the session, its client and live link, never null there. */
export function useSignedIn() {
  const app = useApp();
  if (!app.session || !app.client || !app.live) throw new Error('this screen needs a session');
  return { ...app, session: app.session, client: app.client, live: app.live };
}
