import { createContext, useContext } from 'react';
import type { Server } from './server';

/** Every screen, with what it needs to open. */
export type Route =
  | { name: 'connect' }
  | { name: 'signIn'; server: Server }
  | { name: 'home' }
  | { name: 'movie'; id: number }
  | { name: 'show'; id: number }
  /** `startMs`: where to start; leave it out to resume. */
  | { name: 'player'; itemId: number; startMs?: number };

export interface Nav {
  push(route: Route): void;
  /** Back one screen, if there is one. */
  pop(): void;
  /** A fresh start at `route`: after signing in or out. */
  reset(route: Route): void;
}

export const NavContext = createContext<Nav | null>(null);

export function useNav(): Nav {
  const nav = useContext(NavContext);
  if (!nav) throw new Error('useNav outside the app');
  return nav;
}
